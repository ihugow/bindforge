//! Daemon-scoped KWin script. No installed package is needed for normal operation.
use std::{fs, path::{Path, PathBuf}, process::{Command, Stdio}, time::{Duration, Instant}};
use std::os::unix::fs::{MetadataExt, PermissionsExt, DirBuilderExt, OpenOptionsExt};
use std::io::Write;
use zbus::blocking::{Connection, Proxy};
use remap_ipc::dbus::BridgeStore;
const LEGACY: &str = "app-buttons-remap-focus";
const KEY: &str = "app-buttons-remap-focusEnabled";
pub const RUNTIME_PLUGIN: &str = "bindforge-runtime";
const SCRIPT: &str = include_str!("../../../../integrations/kwin/contents/code/main.js");
fn user_dir(variable: &str, fallback: &str) -> Result<PathBuf, String> {
    if let Some(p) = std::env::var_os(variable).map(PathBuf::from).filter(|p| p.is_absolute()) { return Ok(p); }
    std::env::var_os("HOME").map(PathBuf::from).filter(|p| p.is_absolute()).map(|p| p.join(fallback))
        .ok_or_else(|| "Não foi possível localizar as pastas do usuário.".into())
}
fn tool(name: &str) -> Result<PathBuf, String> {
    ["/usr/bin", "/bin"].iter().map(|base| Path::new(base).join(name)).find(|p| p.is_file())
        .ok_or_else(|| format!("Utilitário {name} não encontrado. Instale as ferramentas do Plasma 6 e tente novamente."))
}
fn run(tool_name: &str, args: &[&std::ffi::OsStr], output: Option<&Path>) -> Result<(), String> {
    // Outputs are either discarded or redirected to a private file; no full pipes can block wait.
    let stdout = if let Some(path) = output { Stdio::from(fs::File::create(path).map_err(|e| e.to_string())?) } else { Stdio::null() };
    let mut child = Command::new(tool(tool_name)?).args(args).stdin(Stdio::null()).stdout(stdout).stderr(Stdio::null()).spawn()
        .map_err(|e| format!("Não foi possível executar {tool_name}: {e}"))?;
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        match child.try_wait() {
            Ok(Some(status)) if status.success() => return Ok(()),
            Ok(Some(_)) => return Err(format!("{tool_name} não concluiu a operação. Verifique as permissões das pastas do usuário e tente novamente.")),
            Err(e) => { let _ = child.kill(); let _ = child.wait(); return Err(e.to_string()); }
            Ok(None) => {}
        }
        if Instant::now() >= deadline { let _ = child.kill(); let _ = child.wait(); return Err(format!("Tempo esgotado ao executar {tool_name}.")); }
        std::thread::sleep(Duration::from_millis(25));
    }
}
struct Scratch(PathBuf);
impl Scratch {
    fn create() -> Result<Self, String> {
        // create_dir is exclusive; mode 0700 protects both the package and command output.
        use std::os::unix::fs::DirBuilderExt;
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        for _ in 0..100 {
            let p = std::env::temp_dir().join(format!("bindforge-kwin-{}-{}", std::process::id(), NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)));
            match fs::DirBuilder::new().mode(0o700).create(&p) {
                Ok(()) => return Ok(Self(p)), Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(e.to_string()),
            }
        }
        Err("Não foi possível criar a pasta temporária da integração.".into())
    }
}
impl Drop for Scratch { fn drop(&mut self) { let _ = fs::remove_dir_all(&self.0); } }

fn scripting<'a>(connection: &'a Connection, owner: &'a str) -> Result<Proxy<'a>, String> {
    Proxy::new(connection, owner, "/Scripting", "org.kde.kwin.Scripting").map_err(|e| e.to_string())
}
fn is_loaded(proxy: &Proxy<'_>, plugin: &str) -> Result<bool, String> {
    proxy.call("isScriptLoaded", &(plugin,)).map_err(|e| format!("Não foi possível consultar o KWin: {e}"))
}
fn unload(proxy: &Proxy<'_>, plugin: &str) -> Result<(), String> {
    if !is_loaded(proxy, plugin)? { return Ok(()); }
    let removed: bool = proxy.call("unloadScript", &(plugin,)).map_err(|e| e.to_string())?;
    if !removed { return Err("KWin recusou encerrar a ponte anterior.".into()); }
    let deadline = Instant::now() + Duration::from_secs(2);
    while is_loaded(proxy, plugin)? {
        if Instant::now() >= deadline { return Err("KWin ainda está encerrando a ponte anterior.".into()); }
        std::thread::sleep(Duration::from_millis(25));
    }
    Ok(())
}
fn disable_legacy(config: &Path) -> Result<(), String> {
    let scratch = Scratch::create()?; let output = scratch.0.join("enabled");
    run("kreadconfig6", &["--file".as_ref(), config.as_os_str(), "--group".as_ref(), "Plugins".as_ref(), "--key".as_ref(), KEY.as_ref(), "--default".as_ref(), "false".as_ref()], Some(&output))?;
    if fs::read_to_string(&output).map_err(|e| e.to_string())?.trim() == "true" {
        run("kwriteconfig6", &["--file".as_ref(), config.as_os_str(), "--group".as_ref(), "Plugins".as_ref(), "--key".as_ref(), KEY.as_ref(), "--type".as_ref(), "bool".as_ref(), "false".as_ref()], None)?;
        run("kreadconfig6", &["--file".as_ref(), config.as_os_str(), "--group".as_ref(), "Plugins".as_ref(), "--key".as_ref(), KEY.as_ref(), "--default".as_ref(), "false".as_ref()], Some(&output))?;
        if fs::read_to_string(&output).map_err(|e| e.to_string())?.trim() == "true" { return Err("Desabilite o script BindForge antigo nas configurações do KWin para concluir a migração.".into()); }
    }
    Ok(())
}
struct ScriptFile { directory: PathBuf, path: PathBuf }
impl ScriptFile {
    fn create() -> Result<Self, String> {
        let uid = fs::metadata("/proc/self").map_err(|e| e.to_string())?.uid();
        let base = std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from).filter(|p| p.is_absolute())
            .unwrap_or_else(std::env::temp_dir);
        Self::at(base.join(format!("bindforge-bridge-{uid}")), uid)
    }
    fn at(directory: PathBuf, uid: u32) -> Result<Self, String> {
        match fs::DirBuilder::new().mode(0o700).create(&directory) {
            Ok(()) => {}, Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}, Err(e) => return Err(e.to_string()),
        }
        let meta = fs::symlink_metadata(&directory).map_err(|e| e.to_string())?;
        if !meta.is_dir() || meta.uid() != uid || meta.permissions().mode() & 0o077 != 0 { return Err("A pasta temporária da ponte não pertence exclusivamente ao usuário atual.".into()); }
        let path = directory.join("main.js");
        let temp = directory.join(format!("main-{}.tmp", std::process::id()));
        let mut file = fs::OpenOptions::new().create_new(true).write(true).mode(0o600).open(&temp).map_err(|e| e.to_string())?;
        let result = file.write_all(SCRIPT.as_bytes()).and_then(|_| fs::rename(&temp, &path));
        if let Err(e) = result { let _ = fs::remove_file(&temp); return Err(e.to_string()); }
        Ok(Self { directory, path })
    }
}
impl Drop for ScriptFile { fn drop(&mut self) { let _ = fs::remove_file(&self.path); let _ = fs::remove_dir(&self.directory); } }

pub(super) struct BridgeRuntime {
    connection: Connection, store: BridgeStore, owner: Option<String>, script: Option<ScriptFile>,
    migrate: bool, migration_checked: bool, next_attempt: Instant,
}
impl BridgeRuntime {
    pub fn new(connection: Connection, store: BridgeStore, migrate: bool) -> Self {
        Self { connection, store, owner: None, script: None, migrate, migration_checked: false, next_attempt: Instant::now() }
    }
    fn activate(&mut self, owner: &str) -> Result<(), String> {
        let proxy = scripting(&self.connection, owner)?;
        if !self.migration_checked {
            let legacy_loaded = is_loaded(&proxy, LEGACY)?;
            if self.migrate {
                let installed = user_dir("XDG_DATA_HOME", ".local/share")?.join("kwin/scripts").join(LEGACY).exists();
                if legacy_loaded || installed {
                    let config = user_dir("XDG_CONFIG_HOME", ".config")?.join("kwinrc");
                    disable_legacy(&config)?;
                    self.store.update(|s| s.legacy_disabled = true);
                }
            }
            unload(&proxy, LEGACY)?;
            self.migration_checked = true;
        } else {
            // A user may re-enable the old installed script while the daemon is running.
            if is_loaded(&proxy, LEGACY)? {
                if self.migrate { disable_legacy(&user_dir("XDG_CONFIG_HOME", ".config")?.join("kwinrc"))?; }
                unload(&proxy, LEGACY)?;
                self.store.update(|s| s.legacy_disabled = true);
            }
        }
        // Stable runtime name also lets the next daemon reclaim a script left by SIGKILL.
        unload(&proxy, "app-buttons-remap-runtime")?; // Clear a stale bridge from the previous brand.
        unload(&proxy, RUNTIME_PLUGIN)?;
        self.store.update(|s| { s.loaded = false; s.focus_received = false; });
        if self.script.is_none() { self.script = Some(ScriptFile::create()?); }
        let path = self.script.as_ref().unwrap().path.to_str().ok_or("Caminho da ponte inválido.")?;
        let id: i32 = proxy.call("loadScript", &(path, RUNTIME_PLUGIN)).map_err(|e| e.to_string())?;
        if id < 0 { return Err("KWin recusou carregar a ponte temporária.".into()); }
        let object = format!("/Scripting/Script{id}");
        Proxy::new(&self.connection, owner, object.as_str(), "org.kde.kwin.Script").map_err(|e| e.to_string())?
            .call::<_, _, ()>("run", &()).map_err(|e| format!("A ponte temporária não iniciou: {e}"))?;
        if !is_loaded(&proxy, RUNTIME_PLUGIN)? { return Err("A ponte temporária encerrou durante a inicialização.".into()); }
        Ok(())
    }
    /// Returns true if a previously usable bridge was lost, so the policy can reset.
    pub fn tick(&mut self, owner: Option<&str>) -> bool {
        let was_loaded = self.store.snapshot().loaded;
        let changed = self.owner.as_deref() != owner;
        if changed {
            self.owner = owner.map(str::to_string); self.next_attempt = Instant::now(); self.migration_checked = false;
            self.store.update(|s| { s.loaded = false; s.focus_received = false; });
        }
        let requested = self.store.requested();
        let forced = requested > self.store.snapshot().completed;
        self.store.update(|s| s.kwin_available = owner.is_some());
        let Some(owner) = owner else {
            self.store.update(|s| { s.loaded = false; s.focus_received = false; s.completed = requested; s.message = "Aguardando KWin nesta sessão.".into(); });
            return was_loaded;
        };
        if !forced && self.store.snapshot().loaded {
            match scripting(&self.connection, owner).and_then(|p| is_loaded(&p, RUNTIME_PLUGIN)) {
                Ok(true) => {
                    // If the persistent bridge is re-enabled, run migration again.
                    if let Ok(proxy) = scripting(&self.connection, owner) {
                        if !is_loaded(&proxy, LEGACY).unwrap_or(true) { return false; }
                    }
                },
                result => self.store.update(|s| { s.loaded = false; s.focus_received = false; s.message = result.err().unwrap_or_else(|| "A ponte foi descarregada; reconectando.".into()); }),
            }
        }
        if !forced && Instant::now() < self.next_attempt { return was_loaded && !self.store.snapshot().loaded; }
        self.next_attempt = Instant::now() + Duration::from_secs(3);
        let result = self.activate(owner);
        self.store.update(|s| { s.completed = requested; s.loaded = result.is_ok(); s.message = result.err().unwrap_or_default(); if !s.loaded { s.focus_received = false; } });
        was_loaded && !self.store.snapshot().loaded
    }
}
impl Drop for BridgeRuntime {
    fn drop(&mut self) {
        if let Some(owner) = &self.owner {
            if let Ok(proxy) = scripting(&self.connection, owner) { let _ = unload(&proxy, RUNTIME_PLUGIN); }
        }
        self.store.update(|s| { s.loaded = false; s.focus_received = false; });
        // ScriptFile is removed only after KWin has been asked to unload it.
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{collections::BTreeSet, io::{BufRead, BufReader}, sync::{Arc, Mutex}};
    use zbus::blocking::connection::Builder;
    use remap_core::{ActiveWindow, WindowEvent};
    use crate::dbus::{KdeWindowMonitor, SERVICE, PATH, INTERFACE};
    #[derive(Default)]
    struct Calls { loaded: BTreeSet<String>, events: Vec<String>, fail_load: bool }
    struct Scripting(Arc<Mutex<Calls>>);
    #[zbus::interface(name = "org.kde.kwin.Scripting")]
    impl Scripting {
        #[zbus(name = "isScriptLoaded")]
        fn loaded(&self, plugin: &str) -> bool { self.0.lock().unwrap().loaded.contains(plugin) }
        #[zbus(name = "unloadScript")]
        fn unload(&self, plugin: &str) -> bool {
            let mut s = self.0.lock().unwrap(); s.events.push(format!("unload:{plugin}")); s.loaded.remove(plugin)
        }
        #[zbus(name = "loadScript")]
        fn load(&self, file: &str, plugin: &str) -> i32 {
            assert_eq!(fs::read_to_string(file).unwrap(), SCRIPT);
            let mut s = self.0.lock().unwrap(); s.events.push(format!("load:{plugin}"));
            if s.fail_load { return -1; }
            assert!(s.loaded.insert(plugin.into())); 7
        }
    }
    struct Script(Arc<Mutex<Calls>>);
    #[zbus::interface(name = "org.kde.kwin.Script")]
    impl Script {
        #[zbus(name = "run")]
        async fn run(&self, #[zbus(connection)] c: &zbus::Connection) -> zbus::fdo::Result<()> {
            self.0.lock().unwrap().events.push("run".into());
            c.call_method(Some(SERVICE), PATH, Some(INTERFACE), "FocusChangedWithProcess", &("firefox", "", "0")).await
                .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))?;
            Ok(())
        }
    }
    struct Bus(std::process::Child);
    impl Drop for Bus { fn drop(&mut self) { let _ = self.0.kill(); let _ = self.0.wait(); } }
    fn kwin(address: &str, calls: Arc<Mutex<Calls>>) -> Connection {
        Builder::address(address).unwrap().serve_at("/Scripting", Scripting(calls.clone())).unwrap()
            .serve_at("/Scripting/Script7", Script(calls)).unwrap().name("org.kde.KWin").unwrap().build().unwrap()
    }
    #[test]
    fn automatic_bridge_recovers_reconnects_and_cleans_up_without_touching_other_scripts() {
        let scratch = Scratch::create().unwrap();
        let mut process = Command::new("dbus-daemon").args(["--session", "--nofork", "--print-address=1"]).stdout(Stdio::piped()).spawn().unwrap();
        let mut address = String::new(); BufReader::new(process.stdout.take().unwrap()).read_line(&mut address).unwrap();
        let _bus = Bus(process); let address = address.trim();
        let calls = Arc::new(Mutex::new(Calls::default()));
        calls.lock().unwrap().loaded.extend([LEGACY.into(), RUNTIME_PLUGIN.into(), "unrelated-script".into()]);
        let server = kwin(address, calls.clone());
        let mut monitor = KdeWindowMonitor::at_address(address).unwrap();
        let directory = scratch.0.join("runtime");
        let uid = fs::metadata("/proc/self").unwrap().uid();
        monitor.bridge.script = Some(ScriptFile::at(directory.clone(), uid).unwrap());
        let focused = Some(WindowEvent::Focused(ActiveWindow { app_id: Some("firefox".into()), ..Default::default() }));
        assert_eq!(monitor.poll_event().unwrap(), focused);
        assert!(monitor.bridge.store.snapshot().loaded); assert!(monitor.bridge.store.snapshot().focus_received);
        assert!(!calls.lock().unwrap().loaded.contains(LEGACY));
        assert!(calls.lock().unwrap().loaded.contains("unrelated-script"));
        let client = Builder::address(address).unwrap().build().unwrap();
        let proxy = Proxy::new(&client, SERVICE, PATH, remap_ipc::dbus::INTEGRATION_INTERFACE).unwrap();
        let target: u64 = proxy.call("RequestReconnect", &()).unwrap();
        assert_eq!(monitor.poll_event().unwrap(), focused);
        assert_eq!(monitor.bridge.store.snapshot().completed, target);
        calls.lock().unwrap().loaded.remove(RUNTIME_PLUGIN);
        monitor.bridge.next_attempt = Instant::now();
        assert_eq!(monitor.poll_event().unwrap(), focused);
        server.close().unwrap();
        assert_eq!(monitor.poll_event().unwrap(), Some(WindowEvent::NoActiveWindow));
        assert!(!monitor.bridge.store.snapshot().loaded); assert!(!monitor.bridge.store.snapshot().focus_received);
        calls.lock().unwrap().loaded.remove(RUNTIME_PLUGIN);
        let _replacement = kwin(address, calls.clone());
        assert_eq!(monitor.poll_event().unwrap(), Some(WindowEvent::NoActiveWindow));
        assert_eq!(monitor.poll_event().unwrap(), focused);
        calls.lock().unwrap().fail_load = true;
        monitor.bridge.store.request();
        assert_eq!(monitor.poll_event().unwrap(), Some(WindowEvent::NoActiveWindow));
        assert!(!monitor.bridge.store.snapshot().loaded); assert!(!monitor.bridge.store.snapshot().message.is_empty());
        calls.lock().unwrap().fail_load = false;
        monitor.bridge.store.request();
        assert_eq!(monitor.poll_event().unwrap(), focused);
        drop(monitor);
        assert!(!directory.exists());
        let calls = calls.lock().unwrap();
        assert!(!calls.loaded.contains(RUNTIME_PLUGIN)); assert!(calls.loaded.contains("unrelated-script"));
        assert!(calls.events.iter().all(|s| !s.contains("unrelated-script")));
    }
    #[test]
    fn migration_disables_only_legacy_key_and_keeps_package_and_other_preferences() {
        let scratch = Scratch::create().unwrap(); let config = scratch.0.join("kwinrc");
        fs::write(&config, "[Plugins]\napp-buttons-remap-focusEnabled=true\notherEnabled=true\n\n[Other]\nsetting=keep\n").unwrap();
        disable_legacy(&config).unwrap(); disable_legacy(&config).unwrap();
        let text = fs::read_to_string(config).unwrap();
        assert!(text.contains("app-buttons-remap-focusEnabled=false")); assert!(text.contains("otherEnabled=true")); assert!(text.contains("setting=keep"));
    }
    #[test]
    fn runtime_file_rejects_symlink_directories_and_recovers_a_stale_file() {
        let scratch = Scratch::create().unwrap(); let uid = fs::metadata("/proc/self").unwrap().uid();
        let directory = scratch.0.join("runtime"); fs::DirBuilder::new().mode(0o700).create(&directory).unwrap();
        fs::write(directory.join("main.js"), "old").unwrap();
        let file = ScriptFile::at(directory.clone(), uid).unwrap(); assert_eq!(fs::read_to_string(&file.path).unwrap(), SCRIPT);
        drop(file); assert!(!directory.exists());
        let link = scratch.0.join("link"); std::os::unix::fs::symlink(&scratch.0, &link).unwrap();
        assert!(ScriptFile::at(link, uid).is_err());
    }
}
