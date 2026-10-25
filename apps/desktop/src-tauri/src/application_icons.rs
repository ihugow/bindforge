//! Resolve installed desktop/Steam icons locally; never execute desktop entries.
use std::path::{Path, PathBuf};
use base64::Engine;
use gtk::{gio, gdk_pixbuf::Pixbuf, prelude::*};

#[derive(serde::Serialize)]
pub struct ApplicationIcon { pub app_id: String, pub name: String, pub icon: String }

fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 4096 && !id.chars().any(char::is_control)
        && !id.contains('/') && !id.contains('\\') && id != "." && id != ".."
}
fn steam_id(id: &str) -> Option<&str> {
    id.strip_prefix("steam:").filter(|v| !v.is_empty() && v.len() <= 10 && v.bytes().all(|b| b.is_ascii_digit()))
}
fn matches_app(app: &gio::DesktopAppInfo, id: &str) -> bool {
    if let Some(game) = steam_id(id) {
        let icon = format!("steam_icon_{game}");
        let uri = format!("steam://rungameid/{game}");
        return app.string("Icon").is_some_and(|v| v == icon)
            || app.string("Exec").is_some_and(|v| v.split_whitespace().any(|word| word.trim_matches('"') == uri));
    }
    app.id().is_some_and(|v| v.strip_suffix(".desktop").unwrap_or(&v).eq_ignore_ascii_case(id.strip_suffix(".desktop").unwrap_or(id)))
        || app.startup_wm_class().is_some_and(|v| v.eq_ignore_ascii_case(id))
}
fn steam_roots() -> Vec<PathBuf> {
    let Some(home) = std::env::var_os("HOME").map(PathBuf::from) else { return vec![]; };
    let data = std::env::var_os("XDG_DATA_HOME").map(PathBuf::from).filter(|p| p.is_absolute()).unwrap_or_else(|| home.join(".local/share"));
    vec![data.join("Steam"), home.join(".steam/steam"), home.join(".steam/root"), home.join(".var/app/com.valvesoftware.Steam/data/Steam")]
}
fn cached_game_images(root: &Path, game: &str) -> Vec<PathBuf> {
    let cache = root.join("appcache/librarycache");
    let mut files = vec![cache.join(format!("{game}_icon.jpg")), cache.join(format!("{game}_icon.png"))];
    // Steam uses both flat filenames and per-app/hash directories.
    let directory = cache.join(game);
    for name in ["icon.png", "icon.jpg", "logo.png", "library_600x900.jpg", "library_capsule.jpg"] { files.push(directory.join(name)); }
    if let Ok(entries) = std::fs::read_dir(&directory) {
        let mut children: Vec<_> = entries.flatten().take(128).map(|e| e.path()).filter(|p| p.is_dir()).collect();
        children.sort();
        for child in children {
            for name in ["icon.png", "icon.jpg", "logo.png", "library_600x900.jpg", "library_capsule.jpg"] { files.push(child.join(name)); }
        }
    }
    files.push(cache.join(format!("{game}_library_600x900.jpg")));
    files
}
fn encode(pixbuf: Pixbuf) -> Option<String> {
    let width = pixbuf.width(); let height = pixbuf.height();
    if width <= 0 || height <= 0 { return None; }
    let scale = (96.0 / width as f64).min(96.0 / height as f64);
    let scaled = pixbuf.scale_simple((width as f64 * scale).round().max(1.0) as i32, (height as f64 * scale).round().max(1.0) as i32, gtk::gdk_pixbuf::InterpType::Bilinear)?;
    let bytes = scaled.save_to_bufferv("png", &[]).ok()?;
    let encoded = base64::engine::general_purpose::STANDARD.encode(bytes);
    (encoded.len() <= 65536).then(|| format!("data:image/png;base64,{encoded}"))
}
fn themed(theme: &gtk::IconTheme, icon: &impl IsA<gio::Icon>) -> Option<String> {
    let info = theme.lookup_by_gicon(icon, 96, gtk::IconLookupFlags::FORCE_SIZE)?;
    encode(info.load_icon().ok()?)
}
/// GTK icon themes must be queried on the application's main thread.
pub fn find(ids: Vec<String>) -> Result<Vec<ApplicationIcon>, String> {
    if ids.len() > 256 || ids.iter().any(|id| !valid_id(id)) { return Err("IDs de aplicativos inválidos.".into()); }
    let theme = gtk::IconTheme::default().ok_or("Não foi possível consultar os ícones do sistema.")?;
    static EXTRA_PATHS: std::sync::Once = std::sync::Once::new();
    EXTRA_PATHS.call_once(|| {
        if let Some(home) = std::env::var_os("HOME") {
            theme.append_search_path(PathBuf::from(home).join(".local/share/flatpak/exports/share/icons"));
        }
        theme.append_search_path("/var/lib/flatpak/exports/share/icons");
    });
    let applications: Vec<_> = gio::AppInfo::all().into_iter().filter_map(|app| app.downcast::<gio::DesktopAppInfo>().ok()).collect();
    let roots = steam_roots();
    let mut result = vec![];
    for id in ids {
        if result.iter().any(|item: &ApplicationIcon| item.app_id == id) { continue; }
        let desktop_id = if id.ends_with(".desktop") { id.clone() } else { format!("{id}.desktop") };
        let app = if steam_id(&id).is_some() { None } else { gio::DesktopAppInfo::new(&desktop_id) }
            .or_else(|| applications.iter().find(|app| matches_app(app, &id)).cloned());
        let icon = app.as_ref().and_then(|app| app.icon()).and_then(|icon| themed(&theme, &icon))
            .or_else(|| {
                let name = steam_id(&id).map(|game| format!("steam_icon_{game}")).unwrap_or_else(|| id.trim_end_matches(".desktop").to_string());
                themed(&theme, &gio::ThemedIcon::new(&name))
            }).or_else(|| steam_id(&id).and_then(|game| {
                roots.iter().flat_map(|root| cached_game_images(root, game)).find_map(|path| {
                    if !path.is_file() || std::fs::metadata(&path).ok()?.len() > 5 * 1024 * 1024 { return None; }
                    encode(Pixbuf::from_file_at_scale(path, 96, 96, true).ok()?)
                })
            }));
        if let Some(icon) = icon {
            let name = app.map(|app| app.display_name().to_string()).unwrap_or_else(|| steam_id(&id).map(|game| format!("Jogo Steam {game}")).unwrap_or_else(|| id.clone()));
            result.push(ApplicationIcon { app_id: id, name, icon });
        }
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn entry(content: &str) -> gio::DesktopAppInfo {
        let key = gtk::glib::KeyFile::new();
        key.load_from_data(&format!("[Desktop Entry]\nType=Application\nName=Test\nExec=true\n{content}"), gtk::glib::KeyFileFlags::NONE).unwrap();
        gio::DesktopAppInfo::from_keyfile(&key).unwrap()
    }
    #[test]
    fn matches_window_class_and_exact_steam_game_without_executing_entries() {
        let app = entry("StartupWMClass=Firefox\nIcon=firefox\n");
        assert!(matches_app(&app, "firefox"));
        assert!(!matches_app(&app, "fox"));
        let game = entry("Icon=steam_icon_1808500\n");
        assert!(matches_app(&game, "steam:1808500"));
        assert!(!matches_app(&game, "steam:180850"));
        let game = entry("Exec=steam steam://rungameid/1808500\n");
        assert!(matches_app(&game, "steam:1808500"));
        assert!(!matches_app(&game, "steam:180850"));
    }
    #[test]
    #[ignore = "requires a graphical Linux session and installed apps selected with BINDFORGE_ICON_TEST_IDS"]
    fn installed_application_icons() {
        gtk::init().expect("graphical GTK session");
        let ids: Vec<String> = std::env::var("BINDFORGE_ICON_TEST_IDS").expect("provide installed application IDs separated by commas").split(',').map(str::to_string).collect();
        let icons = find(ids.clone()).unwrap();
        for id in ids { assert!(icons.iter().any(|icon| icon.app_id == id), "missing icon for {id}"); }
        if let Some(output) = std::env::var_os("BINDFORGE_ICON_TEST_OUTPUT") {
            std::fs::write(output, serde_json::to_vec(&icons).unwrap()).unwrap();
        }
        assert!(find(vec!["bindforge-app-that-does-not-exist-823915".into()]).unwrap().is_empty());
    }
    #[test]
    fn rejects_paths_and_malformed_game_ids() {
        for id in ["../secret", "/tmp/icon", "a\\b", "", "a\nb", ".."] { assert!(!valid_id(id)); }
        for id in ["steam:../123", "steam:", "steam:123oops", "steam:12345678901"] { assert!(steam_id(id).is_none()); }
        assert_eq!(steam_id("steam:1808500"), Some("1808500"));
    }
    #[test]
    fn steam_cache_supports_flat_and_hashed_layouts_for_the_exact_game() {
        let root = std::env::temp_dir().join(format!("bindforge-icon-cache-{}", std::process::id()));
        let nested = root.join("appcache/librarycache/1808500/hash");
        std::fs::create_dir_all(&nested).unwrap();
        let icon = nested.join("icon.png");
        std::fs::write(&icon, b"fixture").unwrap();
        let paths = cached_game_images(&root, "1808500");
        assert!(paths.contains(&icon));
        assert!(paths.contains(&root.join("appcache/librarycache/1808500_icon.jpg")));
        assert!(!cached_game_images(&root, "180850").contains(&icon));
        assert!(find(vec!["../secret".into()]).is_err());
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn encodes_small_png_compatible_with_saved_profile_icons() {
        let pixbuf = Pixbuf::new(gtk::gdk_pixbuf::Colorspace::Rgb, true, 8, 300, 150).unwrap();
        pixbuf.fill(0xa679f5ff);
        let icon = encode(pixbuf).unwrap();
        let bytes = base64::engine::general_purpose::STANDARD.decode(icon.strip_prefix("data:image/png;base64,").unwrap()).unwrap();
        let loader = gtk::gdk_pixbuf::PixbufLoader::new();
        loader.write(&bytes).unwrap(); loader.close().unwrap();
        assert_eq!(loader.pixbuf().unwrap().width(), 96);
        assert_eq!(loader.pixbuf().unwrap().height(), 48);
        let mut config = remap_config::Config::default(); config.profiles[0].icon = Some(icon);
        assert!(config.validate().is_ok());
    }
}
