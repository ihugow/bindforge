#[cfg(target_os = "linux")]
use std::path::PathBuf;

/// Keep file selection in the native shell; the webview cannot write arbitrary paths.
pub async fn export(window: tauri::WebviewWindow) -> Result<Option<String>, String> {
    #[cfg(target_os = "linux")]
    {
        use gtk::prelude::*;
        let bytes = tauri::async_runtime::spawn_blocking(|| {
            remap_config::export_backup(&remap_config::load(&remap_config::config_path()?)?)
        }).await.map_err(|e| e.to_string())??;
        let (sender, receiver) = std::sync::mpsc::sync_channel(1);
        let owner = window.clone();
        window.run_on_main_thread(move || {
            let parent = match owner.gtk_window() { Ok(parent) => parent, Err(_) => { let _ = sender.send(None); return; } };
            gtk::glib::MainContext::default().spawn_local(async move {
                let dialog = gtk::FileChooserNative::new(Some("Exportar configurações do BindForge"), Some(&parent),
                    gtk::FileChooserAction::Save, Some("Exportar"), Some("Cancelar"));
                dialog.set_current_name("bindforge-backup.json");
                dialog.set_do_overwrite_confirmation(true);
                let filter = gtk::FileFilter::new();
                filter.set_name(Some("Backup JSON")); filter.add_pattern("*.json"); dialog.add_filter(filter);
                let response = dialog.run_future().await;
                let path = if response == gtk::ResponseType::Accept { dialog.filename() } else { None };
                let _ = sender.send(path);
                dialog.destroy();
            });
        }).map_err(|e| e.to_string())?;
        tauri::async_runtime::spawn_blocking(move || {
            let Some(path) = receiver.recv().map_err(|e| e.to_string())? else { return Ok(None); };
            write_backup(&path, &bytes)?;
            Ok(Some(path.to_string_lossy().into_owned()))
        }).await.map_err(|e| e.to_string())?
    }
    #[cfg(not(target_os = "linux"))]
    { let _ = window; Err("Exportação disponível no aplicativo Linux.".into()) }
}

#[cfg(target_os = "linux")]
fn write_backup(path: &std::path::Path, bytes: &[u8]) -> Result<(), String> {
    use std::{fs::OpenOptions, io::Write, os::unix::fs::OpenOptionsExt};
    // Publish only a complete backup, including when replacing an older export.
    let mut temporary_name = path.as_os_str().to_os_string();
    temporary_name.push(format!(".{}.tmp", std::process::id()));
    let temporary = PathBuf::from(temporary_name);
    let mut file = OpenOptions::new().write(true).create_new(true).mode(0o600)
        .open(&temporary).map_err(|e| format!("Não foi possível criar o backup: {e}"))?;
    let result = file.write_all(bytes).and_then(|_| file.sync_all()).and_then(|_| {
        drop(file);
        std::fs::rename(&temporary, path)
    });
    let _ = std::fs::remove_file(&temporary);
    result.map_err(|e| format!("Não foi possível exportar: {e}"))
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;
    #[test]
    fn export_replaces_only_after_complete_write_and_cleans_up_failure() {
        let root = std::env::temp_dir().join(format!("bindforge-backup-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("backup.json");
        let bytes = remap_config::export_backup(&remap_config::Config::default()).unwrap();
        write_backup(&path, b"old backup").unwrap();
        write_backup(&path, &bytes).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        let directory = root.join("directory.json");
        std::fs::create_dir(&directory).unwrap();
        assert!(write_backup(&directory, &bytes).is_err());
        assert!(directory.is_dir());
        assert_eq!(std::fs::read_dir(&root).unwrap().count(), 2);
        std::fs::remove_dir_all(root).unwrap();
    }
}
