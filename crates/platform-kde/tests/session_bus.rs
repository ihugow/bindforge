#![cfg(all(feature = "dbus", target_os = "linux"))]
use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::time::Duration;

use platform_kde::dbus::{KdeWindowMonitor, INTERFACE, PATH, SERVICE};
use remap_core::{ActiveWindow, WindowEvent, WindowMonitor};
use zbus::blocking::{connection::Builder, Connection, Proxy};

struct PrivateBus { process: Child, address: String, runtime_dir: std::path::PathBuf }
impl PrivateBus {
    fn start() -> Self {
        // Never own org.kde.KWin on the user's real session bus.
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let runtime_dir = std::env::temp_dir().join(format!("bindforge-dbus-test-{}-{}", std::process::id(), NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)));
        std::fs::create_dir(&runtime_dir).unwrap();
        let mut process = Command::new("dbus-daemon")
            .env("XDG_RUNTIME_DIR", &runtime_dir)
            .args(["--session", "--nofork", "--print-address=1"])
            .stdout(Stdio::piped()).spawn().expect("dbus-daemon is required for this integration test");
        let mut address = String::new();
        BufReader::new(process.stdout.take().unwrap()).read_line(&mut address).unwrap();
        assert!(!address.is_empty(), "private bus did not start");
        Self { process, address: address.trim().into(), runtime_dir }
    }
    fn client(&self, kwin: bool) -> Connection {
        let builder = Builder::address(self.address.as_str()).unwrap().method_timeout(Duration::from_secs(2));
        if kwin { builder.name("org.kde.KWin").unwrap().build().unwrap() }
        else { builder.build().unwrap() }
    }
}
impl Drop for PrivateBus {
    fn drop(&mut self) { let _ = self.process.kill(); let _ = self.process.wait(); let _ = std::fs::remove_dir_all(&self.runtime_dir); }
}
fn send(connection: &Connection, app_id: &str, resource_class: &str) -> zbus::Result<()> {
    Proxy::new(connection, SERVICE, PATH, INTERFACE)?.call("FocusChanged", &(app_id, resource_class))
}
struct GameProcess(Child);
impl Drop for GameProcess {
    fn drop(&mut self) { let _ = self.0.kill(); let _ = self.0.wait(); }
}
fn send_process(connection: &Connection, app: &str, pid: &str) -> zbus::Result<()> {
    Proxy::new(connection, SERVICE, PATH, INTERFACE)?.call("FocusChangedWithProcess", &(app, "", pid))
}
fn focused(id: &str) -> Option<WindowEvent> {
    Some(WindowEvent::Focused(ActiveWindow { app_id: Some(id.into()), ..Default::default() }))
}

#[test]
fn refuses_to_start_beside_a_daemon_from_the_previous_brand() {
    let bus = PrivateBus::start();
    let previous = Builder::address(bus.address.as_str()).unwrap()
        .name(remap_ipc::dbus::LEGACY_SERVICE).unwrap().build().unwrap();
    let error = match KdeWindowMonitor::at_address(&bus.address) {
        Ok(_) => panic!("must not start beside the previous daemon"), Err(error) => error,
    };
    assert!(error.contains("versão anterior"));
    drop(previous);
    assert!(KdeWindowMonitor::at_address(&bus.address).is_ok());
}

#[test]
fn authenticates_and_recovers_across_compositor_owner_changes() {
    let bus = PrivateBus::start();
    let mut monitor = KdeWindowMonitor::at_address(&bus.address).unwrap();
    assert!(KdeWindowMonitor::at_address(&bus.address).is_err(), "second daemon must not queue for the name");
    let impostor = bus.client(false);
    assert!(send(&impostor, "firefox", "").unwrap_err().to_string().contains("AccessDenied"));
    let kwin = bus.client(true);
    assert!(send(&impostor, "firefox", "").unwrap_err().to_string().contains("AccessDenied"));
    send(&kwin, "firefox", "").unwrap();
    // Owner acquisition clears old state before delivering new focus.
    assert_eq!(monitor.next_event().unwrap(), Some(WindowEvent::NoActiveWindow));
    assert_eq!(monitor.next_event().unwrap(), focused("firefox"));
    assert!(send(&kwin, &"x".repeat(4097), "").unwrap_err().to_string().contains("InvalidArgs"));
    assert!(send(&kwin, "bad\napp", "").unwrap_err().to_string().contains("InvalidArgs"));
    // Several compositor events before one consumer read coalesce to the latest.
    send(&kwin, "firefox", "").unwrap();
    send(&kwin, "", "org.kde.konsole").unwrap();
    assert_eq!(monitor.next_event().unwrap(), focused("org.kde.konsole"));
    send(&kwin, "", "").unwrap();
    assert_eq!(monitor.next_event().unwrap(), Some(WindowEvent::NoActiveWindow));
    send(&kwin, "firefox", "").unwrap();
    assert_eq!(monitor.next_event().unwrap(), focused("firefox"));
    // Explicit capture ignores stale/current focus and keeps only the first new app.
    send(&kwin, "bindforge-desktop", "").unwrap();
    assert_eq!(monitor.next_event().unwrap(), focused("bindforge-desktop"));
    let capture = remap_ipc::dbus::WindowCapture::at_address(&bus.address).unwrap();
    assert!(capture.poll().unwrap().is_none());
    send(&kwin, "bindforge-desktop", "").unwrap();
    assert!(send(&impostor, "spoofed", "").is_err());
    send(&kwin, "", "").unwrap();
    assert!(capture.poll().unwrap().is_none());
    send(&kwin, "firefox", "").unwrap();
    assert_eq!(capture.poll().unwrap().as_deref(), Some("firefox"));
    send(&kwin, "bindforge-desktop", "").unwrap();
    assert_eq!(capture.poll().unwrap().as_deref(), Some("firefox"));
    assert_eq!(monitor.next_event().unwrap(), focused("bindforge-desktop"));
    drop(capture);
    // Dropping the client cancels its request and permits immediate rearming.
    drop(remap_ipc::dbus::WindowCapture::at_address(&bus.address).unwrap());
    // A real /proc read, using two isolated test processes whose executable is
    // called gamescope. No Steam installation or game launch is needed.
    let executable = bus.runtime_dir.join("gamescope");
    std::fs::copy("/bin/sleep", &executable).unwrap();
    let game_a = GameProcess(Command::new(&executable).arg("30").env_clear().env("SteamAppId", "1808500").spawn().unwrap());
    let game_b = GameProcess(Command::new(&executable).arg("30").env_clear().env("SteamAppId", "570").spawn().unwrap());
    assert!(send_process(&impostor, "gamescope", &game_a.0.id().to_string()).is_err());
    assert!(send_process(&kwin, "gamescope", "../self").is_err());
    let game_capture = remap_ipc::dbus::WindowCapture::at_address(&bus.address).unwrap();
    send_process(&kwin, "gamescope", &game_a.0.id().to_string()).unwrap();
    assert_eq!(monitor.next_event().unwrap(), focused("steam:1808500"));
    assert_eq!(game_capture.poll().unwrap().as_deref(), Some("steam:1808500"));
    drop(game_capture);
    let game_capture = remap_ipc::dbus::WindowCapture::at_address(&bus.address).unwrap();
    send_process(&kwin, "gamescope", &game_b.0.id().to_string()).unwrap();
    assert_eq!(monitor.next_event().unwrap(), focused("steam:570"));
    assert_eq!(game_capture.poll().unwrap().as_deref(), Some("steam:570"));
    drop(game_capture);
    // Actual nested Wayland behavior: KWin sends the Gamescope PID but both
    // desktopFileName/resourceClass are empty. This is not an absence of window.
    let game_capture = remap_ipc::dbus::WindowCapture::at_address(&bus.address).unwrap();
    send_process(&kwin, "", &game_a.0.id().to_string()).unwrap();
    assert_eq!(monitor.next_event().unwrap(), focused("steam:1808500"));
    assert_eq!(game_capture.poll().unwrap().as_deref(), Some("steam:1808500"));
    drop(game_capture);
    // An unidentified actual Gamescope still produces an explicit generic result
    // for the capture UI to explain, instead of waiting until timeout.
    let unknown_game = GameProcess(Command::new(&executable).arg("30").env_clear().spawn().unwrap());
    let unknown_capture = remap_ipc::dbus::WindowCapture::at_address(&bus.address).unwrap();
    send_process(&kwin, "", &unknown_game.0.id().to_string()).unwrap();
    assert_eq!(monitor.next_event().unwrap(), focused("gamescope"));
    assert_eq!(unknown_capture.poll().unwrap().as_deref(), Some("gamescope"));
    drop(unknown_capture);
    // Empty labels on a different process must never identify it as Gamescope.
    send_process(&kwin, "", &std::process::id().to_string()).unwrap();
    assert_eq!(monitor.next_event().unwrap(), Some(WindowEvent::NoActiveWindow));
    // A PID with Steam metadata must not relabel unrelated application windows.
    send_process(&kwin, "firefox", &game_a.0.id().to_string()).unwrap();
    assert_eq!(monitor.next_event().unwrap(), focused("firefox"));
    send_process(&kwin, "gamescope", "0").unwrap();
    assert_eq!(monitor.next_event().unwrap(), focused("gamescope"));
    let capture = remap_ipc::dbus::WindowCapture::at_address(&bus.address).unwrap();
    kwin.close().unwrap();
    assert!(capture.poll().is_err());
    drop(capture);
    assert_eq!(monitor.next_event().unwrap(), Some(WindowEvent::NoActiveWindow));
    let replacement = bus.client(true);
    send(&replacement, "org.kde.konsole", "").unwrap();
    assert_eq!(monitor.next_event().unwrap(), Some(WindowEvent::NoActiveWindow));
    assert_eq!(monitor.next_event().unwrap(), focused("org.kde.konsole"));
    // A dead bus is an error, never a silent continuation with stale focus.
    let mut bus = bus;
    bus.process.kill().unwrap();
    bus.process.wait().unwrap();
    assert!(monitor.next_event().is_err());
}
