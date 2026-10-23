#![cfg(all(feature = "kde-dbus", target_os = "linux"))]
use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};
use remap_ipc::{DaemonStatus, FocusState};
use remap_ipc::dbus::{read_status_at, SERVICE, PATH, STATUS_INTERFACE};
use zbus::blocking::{connection::Builder, Proxy};

struct LegacyStatus;
#[zbus::interface(name = "org.bindforge.Daemon.Status1")]
impl LegacyStatus {
    fn get_status(&self) -> (u32, bool, String, String, u64) {
        (1, true, "default".into(), "waiting".into(), 1)
    }
}

struct ChildGuard(Child);
impl Drop for ChildGuard {
    fn drop(&mut self) { let _ = self.0.kill(); let _ = self.0.wait(); }
}
fn wait_for(address: &str, predicate: impl Fn(&DaemonStatus) -> bool) -> DaemonStatus {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Ok(state) = read_status_at(address) {
            if predicate(&state) { return state; }
        }
        assert!(Instant::now() < deadline, "daemon did not publish expected status");
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn next_status(events: &std::sync::mpsc::Receiver<remap_ipc::dbus::WatchEvent>, id: &str) -> DaemonStatus {
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        let event = events.recv_timeout(deadline.saturating_duration_since(Instant::now())).expect("missing pushed status");
        if let remap_ipc::dbus::WatchEvent::Status(state) = event {
            if state.active_profile_id.as_deref() == Some(id) { return state; }
        }
    }
}
fn next_unavailable(events: &std::sync::mpsc::Receiver<remap_ipc::dbus::WatchEvent>, restart: bool) {
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        let event = events.recv_timeout(deadline.saturating_duration_since(Instant::now())).expect("missing disconnect event");
        if matches!(event, remap_ipc::dbus::WatchEvent::Unavailable { restart_required } if restart_required == restart) { return; }
    }
}

#[test]
fn daemon_publishes_consumed_profiles_and_read_only_client_detects_disconnect() {
    let runtime_dir = std::env::temp_dir().join(format!("remap-status-test-{}", std::process::id()));
    std::fs::create_dir(&runtime_dir).unwrap();
    let mut bus = ChildGuard(Command::new("dbus-daemon")
        .env("XDG_RUNTIME_DIR", &runtime_dir)
        .args(["--session", "--nofork", "--print-address=1"])
        .stdout(Stdio::piped()).spawn().unwrap());
    let mut address = String::new();
    BufReader::new(bus.0.stdout.take().unwrap()).read_line(&mut address).unwrap();
    let address = address.trim();
    assert!(!address.is_empty());
    let kwin = Builder::address(address).unwrap().name("org.kde.KWin").unwrap().build().unwrap();
    let config_path = runtime_dir.join("profiles.json");
    let mut daemon = ChildGuard(Command::new(env!("CARGO_BIN_EXE_bindforge-daemon"))
        .arg("--kde").env("DBUS_SESSION_BUS_ADDRESS", address).env("DBUS_SYSTEM_BUS_ADDRESS", address).env("BINDFORGE_CONFIG", &config_path)
        .stdout(Stdio::null()).stderr(Stdio::null()).spawn().unwrap());
    let initial = wait_for(address, |s| s.active_profile_id.as_deref() == Some("default"));
    assert!(initial.simulation);
    assert!(initial.history_available);
    assert_eq!(initial.focus_state, FocusState::Waiting);
    let control = Proxy::new(&kwin, SERVICE, PATH, remap_ipc::dbus::INPUT_INTERFACE).unwrap();
    let paused: (u32, bool, String, String) = control.call("GetInputStatus", &()).unwrap();
    assert_eq!((paused.0, paused.1, paused.2), (1, false, String::new()));
    assert!(control.call::<_, _, ()>("StartMouse", &("/dev/input/event0",)).is_err(), "paths must never select a device");
    let inventory = remap_ipc::dbus::read_devices_at(address).unwrap();
    assert!(inventory.iter().all(|device| !device.id.is_empty() && !device.name.is_empty()));
    let battery_proxy = Proxy::new(&kwin, SERVICE, PATH, remap_ipc::dbus::DEVICES_INTERFACE).unwrap();
    let (version, batteries): (u32, Vec<(String, i32, String)>) = battery_proxy.call("GetDeviceBatteries", &()).unwrap();
    assert_eq!(version, 1);
    for (id, percentage, state) in &batteries {
        assert!(inventory.iter().any(|device| &device.id == id));
        assert!((0..=100).contains(percentage));
        assert!(["charging", "discharging", "full", "unknown"].contains(&state.as_str()));
    }
    println!("Read-only battery snapshot: {} interface(s), percentages {:?}", batteries.len(), batteries.iter().map(|(_, percentage, _)| percentage).collect::<Vec<_>>());
    assert_eq!(read_status_at(address).unwrap(), initial, "inventory and battery reads must not change profiles");
    let (cancel, cancelled) = remap_ipc::dbus::watch_cancel();
    let (sender, events) = std::sync::mpsc::channel();
    let watch_address = address.to_owned();
    let watch = std::thread::spawn(move || futures_lite::future::block_on(
        remap_ipc::dbus::watch_status_at(&watch_address, cancelled, |event| sender.send(event).is_ok())));
    let observed = next_status(&events, "default");
    assert_eq!(observed.revision, initial.revision);
    // An unrelated process cannot impersonate daemon signals.
    let mut spoof = observed.clone();
    spoof.revision += 100;
    spoof.active_profile_id = Some("spoofed".into());
    remap_ipc::dbus::emit_status_changed(&kwin, &spoof).unwrap();
    assert!(events.recv_timeout(Duration::from_millis(100)).is_err());
    let focus = Proxy::new(&kwin, SERVICE, PATH, "org.bindforge.Daemon.Focus1").unwrap();
    let capture = remap_ipc::dbus::WindowCapture::at_address(address).unwrap();
    assert!(capture.poll().unwrap().is_none());
    focus.call::<_, _, ()>("FocusChanged", &("firefox", "")).unwrap();
    assert_eq!(capture.poll().unwrap().as_deref(), Some("firefox"));
    drop(capture);
    let browser = wait_for(address, |s| s.active_profile_id.as_deref() == Some("firefox"));
    assert_eq!(browser.focus_state, FocusState::Focused);
    assert_eq!(next_status(&events, "firefox").revision, browser.revision);
    assert!(browser.revision > initial.revision);
    // Read requests must not change the selected profile or revision.
    assert_eq!(read_status_at(address).unwrap(), browser);
    focus.call::<_, _, ()>("FocusChanged", &("firefox", "")).unwrap();
    let repeat = wait_for(address, |s| s.revision > browser.revision);
    assert_eq!(repeat.active_profile_id, browser.active_profile_id);
    assert_eq!(repeat.recent_profiles, browser.recent_profiles);
    focus.call::<_, _, ()>("FocusChanged", &("org.kde.konsole", "")).unwrap();
    wait_for(address, |s| s.active_profile_id.as_deref() == Some("konsole"));
    focus.call::<_, _, ()>("FocusChanged", &("unknown.app", "")).unwrap();
    let fallback = wait_for(address, |s| s.active_profile_id.as_deref() == Some("default"));
    assert_eq!(fallback.focus_state, FocusState::Focused);
    next_status(&events, "konsole");
    next_status(&events, "default");
    // Returning to an unconfigured app must not hide the terminal visit.
    assert_eq!(fallback.recent_profiles.iter().map(|e| e.profile_id.as_str()).collect::<Vec<_>>(),
        vec!["default", "firefox", "konsole", "default"]);
    focus.call::<_, _, ()>("FocusChanged", &("", "")).unwrap();
    wait_for(address, |s| s.focus_state == FocusState::NoActiveWindow);
    // Save a new association while focus stays on the same app. No new KWin
    // event is sent: the idle reload must select and push the new profile.
    let mut config = remap_config::Config::default();
    let mut custom = config.profiles[1].clone();
    custom.id = "personal".into(); custom.priority = 100;
    config.profiles.push(custom);
    focus.call::<_, _, ()>("FocusChanged", &("firefox", "")).unwrap();
    wait_for(address, |s| s.active_profile_id.as_deref() == Some("firefox"));
    let saved = remap_config::save(&config_path, config, 0).unwrap();
    let personal = next_status(&events, "personal");
    assert_eq!(personal.focus_state, FocusState::Focused);
    // Invalid external edits retain the last valid policy and emit no new state.
    std::fs::write(&config_path, b"{invalid").unwrap();
    std::thread::sleep(Duration::from_millis(650));
    assert_eq!(read_status_at(address).unwrap(), personal);
    // Recover after a valid replacement and re-evaluate without moving focus.
    let mut disabled = saved;
    disabled.profiles.last_mut().unwrap().enabled = false;
    // Restore the file first: normal saves deliberately refuse to overwrite corruption.
    let recovery_path = runtime_dir.join("recovery.json");
    remap_config::save(&recovery_path, disabled, 0).unwrap();
    std::fs::rename(&recovery_path, &config_path).unwrap();
    let restored = wait_for(address, |s| s.active_profile_id.as_deref() == Some("firefox"));
    assert!(restored.revision > personal.revision);
    next_status(&events, "firefox");
    let status_proxy = Proxy::new(&kwin, SERVICE, PATH, STATUS_INTERFACE).unwrap();
    assert!(status_proxy.call::<_, _, ()>("SetStatus", &("firefox",)).unwrap_err().to_string().contains("UnknownMethod"));
    drop(status_proxy);
    drop(focus);
    let interrupted_capture = remap_ipc::dbus::WindowCapture::at_address(address).unwrap();
    daemon.0.kill().unwrap(); daemon.0.wait().unwrap();
    assert!(interrupted_capture.poll().is_err());
    drop(interrupted_capture);
    assert!(read_status_at(address).is_err(), "disconnected clients must not return stale status");
    next_unavailable(&events, false);
    // A daemon started before this update still supports current-state monitoring.
    let legacy = Builder::address(address).unwrap().serve_at(PATH, LegacyStatus).unwrap()
        .name(SERVICE).unwrap().build().unwrap();
    let old = read_status_at(address).unwrap();
    assert_eq!(old.active_profile_id.as_deref(), Some("default"));
    assert!(!old.history_available);
    assert!(old.recent_profiles.is_empty());
    assert!(remap_ipc::dbus::read_devices_at(address).unwrap_err().contains("Restart the daemon"));
    assert!(remap_ipc::dbus::WindowCapture::at_address(address).err().unwrap().contains("Reinicie o daemon"));
    next_unavailable(&events, true);
    legacy.close().unwrap();
    let restarted = ChildGuard(Command::new(env!("CARGO_BIN_EXE_bindforge-daemon"))
        .arg("--kde").env("DBUS_SESSION_BUS_ADDRESS", address).env("DBUS_SYSTEM_BUS_ADDRESS", address).env("BINDFORGE_CONFIG", &config_path)
        .stdout(Stdio::null()).stderr(Stdio::null()).spawn().unwrap());
    let fresh = wait_for(address, |s| s.focus_state == FocusState::Waiting && s.active_profile_id.is_some());
    assert_eq!(fresh.active_profile_id.as_deref(), Some("default"));
    let observed = next_status(&events, "default");
    assert!(observed.revision <= fresh.revision);
    assert_eq!(fresh.recent_profiles.len(), 1);
    assert_eq!(fresh.revision, 1, "restarted daemon must expose its own new state");
    drop(cancel);
    watch.join().unwrap();
    drop(restarted);
    drop(kwin); drop(daemon); drop(bus);
    std::fs::remove_dir_all(runtime_dir).unwrap();
}
