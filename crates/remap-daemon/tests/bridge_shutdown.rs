#![cfg(all(feature = "kde-dbus", target_os = "linux"))]
use std::{io::{BufRead, BufReader}, process::{Child, Command, Stdio}, sync::{Arc, Mutex}, time::{Duration, Instant}};
use zbus::blocking::connection::Builder;
struct ChildGuard(Child);
impl Drop for ChildGuard { fn drop(&mut self) { let _ = self.0.kill(); let _ = self.0.wait(); } }
struct Scripting(Arc<Mutex<bool>>);
#[zbus::interface(name = "org.kde.kwin.Scripting")]
impl Scripting {
    #[zbus(name = "isScriptLoaded")]
    fn loaded(&self, name: &str) -> bool { name == "app-buttons-remap-runtime" && *self.0.lock().unwrap() }
    #[zbus(name = "unloadScript")]
    fn unload(&self, name: &str) -> bool { assert_eq!(name, "app-buttons-remap-runtime"); *self.0.lock().unwrap() = false; true }
    #[zbus(name = "loadScript")]
    fn load(&self, path: &str, name: &str) -> i32 {
        assert_eq!(name, "app-buttons-remap-runtime"); assert!(std::fs::read_to_string(path).unwrap().contains("workspace.windowActivated"));
        *self.0.lock().unwrap() = true; 0
    }
}
struct Script;
#[zbus::interface(name = "org.kde.kwin.Script")]
impl Script { #[zbus(name = "run")] fn run(&self) {} }
#[test]
fn sigterm_and_sigint_unload_bridge_and_remove_temporary_source() {
    let root = std::env::temp_dir().join(format!("app-buttons-shutdown-{}", std::process::id()));
    std::fs::create_dir(&root).unwrap();
    let mut bus = ChildGuard(Command::new("dbus-daemon").args(["--session", "--nofork", "--print-address=1"]).stdout(Stdio::piped()).spawn().unwrap());
    let mut address = String::new(); BufReader::new(bus.0.stdout.take().unwrap()).read_line(&mut address).unwrap();
    let loaded = Arc::new(Mutex::new(false));
    let _kwin = Builder::address(address.trim()).unwrap().serve_at("/Scripting", Scripting(loaded.clone())).unwrap()
        .serve_at("/Scripting/Script0", Script).unwrap().name("org.kde.KWin").unwrap().build().unwrap();
    for signal in ["-TERM", "-INT"] {
        let mut daemon = ChildGuard(Command::new(env!("CARGO_BIN_EXE_remap-daemon")).arg("--kde")
            .env("DBUS_SESSION_BUS_ADDRESS", address.trim()).env("DBUS_SYSTEM_BUS_ADDRESS", address.trim()).env("XDG_RUNTIME_DIR", &root)
            .env("XDG_DATA_HOME", root.join("data")).env("XDG_CONFIG_HOME", root.join("config"))
            .env("APP_BUTTONS_CONFIG", root.join("profiles.json")).stdout(Stdio::null()).stderr(Stdio::null()).spawn().unwrap());
        let deadline = Instant::now() + Duration::from_secs(5);
        while !*loaded.lock().unwrap() { assert!(Instant::now() < deadline, "bridge did not load"); std::thread::sleep(Duration::from_millis(20)); }
        assert!(Command::new("kill").args([signal, &daemon.0.id().to_string()]).status().unwrap().success());
        let deadline = Instant::now() + Duration::from_secs(5);
        let exit = loop {
            if let Some(status) = daemon.0.try_wait().unwrap() { break status; }
            assert!(Instant::now() < deadline, "daemon did not shut down"); std::thread::sleep(Duration::from_millis(20));
        };
        assert!(exit.success()); assert!(!*loaded.lock().unwrap());
        assert!(!std::fs::read_dir(&root).unwrap().any(|entry| entry.unwrap().file_name().to_string_lossy().starts_with("app-buttons-remap-bridge-")));
    }
    drop(_kwin); drop(bus); std::fs::remove_dir_all(root).unwrap();
}
