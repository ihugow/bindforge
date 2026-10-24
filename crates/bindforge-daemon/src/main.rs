use std::io::{self, BufRead, Read};
use remap_core::{demo_profiles, DryRunRemapper, ProfileSwitcher, WindowEvent, WindowMonitor};
use platform_kde::KwinFocus;
use remap_ipc::{FocusState, StatusStore};

const MAX_LINE: usize = 4096;
struct StdinMonitor<R: BufRead>(R);
impl<R: BufRead> WindowMonitor for StdinMonitor<R> {
    fn next_event(&mut self) -> Result<Option<WindowEvent>, String> {
        let mut bytes = Vec::new();
        self.0.by_ref().take((MAX_LINE + 1) as u64).read_until(b'\n', &mut bytes)
            .map_err(|e| e.to_string())?;
        if bytes.is_empty() { return Ok(None); }
        if bytes.len() > MAX_LINE { return Err("focus record exceeds 4096 bytes".into()); }
        let line = std::str::from_utf8(&bytes).map_err(|_| "focus record must be UTF-8")?
            .trim_end_matches(['\n', '\r']);
        if line.chars().any(char::is_control) { return Err("control characters are not allowed".into()); }
        Ok(Some(KwinFocus { desktop_file_name: line.into(), resource_class: String::new() }.into_event()))
    }
}

fn drive(monitor: &mut impl WindowMonitor) -> Result<(), String> {
    drive_until(monitor, false)
}

fn drive_until(monitor: &mut impl WindowMonitor, once: bool) -> Result<(), String> {
    drive_with_status(monitor, once, None, None)
}

type StatusNotifier<'a> = Option<&'a dyn Fn(&remap_ipc::DaemonStatus) -> Result<(), String>>;

fn publish_status(status: Option<&StatusStore>, notify: StatusNotifier<'_>, profile: Option<&str>, focus: FocusState) -> Result<(), String> {
    if let Some(status) = status {
        status.publish(profile, focus);
        if let Some(notify) = notify { notify(&status.snapshot())?; }
    }
    Ok(())
}

fn drive_with_status(monitor: &mut impl WindowMonitor, once: bool, status: Option<&StatusStore>, notify: StatusNotifier<'_>) -> Result<(), String> {
    let mut switcher = ProfileSwitcher::new(demo_profiles(), DryRunRemapper::default());
    eprintln!("SIMULATION ONLY: no input devices opened; no keys/buttons remapped.");
    let result = (|| {
        // Establish fallback before accepting events.
        switcher.handle(&WindowEvent::NoActiveWindow)?;
        publish_status(status, notify, switcher.active_id(), FocusState::Waiting)?;
        println!("simulation profile=default");
        while let Some(event) = monitor.next_event()? {
            if switcher.handle(&event)? {
                println!("simulation profile={}", switcher.active_id().unwrap_or("default"));
            }
            let focus = match event { WindowEvent::Focused(_) => FocusState::Focused, WindowEvent::NoActiveWindow => FocusState::NoActiveWindow };
            publish_status(status, notify, switcher.active_id(), focus)?;
            if once {
                eprintln!("focus event received; one-shot simulation complete");
                break;
            }
        }
        Ok(())
    })();
    let cleanup = switcher.reset();
    let notification = if let Some(status) = status {
        status.stopped();
        if let Some(notify) = notify { notify(&status.snapshot()) } else { Ok(()) }
    } else { Ok(()) };
    result.and(cleanup).and(notification)
}
#[cfg(all(feature = "kde-dbus", target_os = "linux"))]
fn run_kde(once: bool) -> Result<(), String> {
    use futures_lite::StreamExt;
    let mut signals = async_signal::Signals::new([async_signal::Signal::Int, async_signal::Signal::Term])
        .map_err(|e| format!("Cannot register shutdown signals: {e}"))?;
    let path = remap_config::config_path()?;
    let mut config = remap_config::load(&path)?;
    let input = platform_linux::native::InputSession::with_access_policy(std::sync::Arc::new(|device| {
        let kind = match device.kind { platform_linux::native::InputKind::Mouse => bindforge_access::Kind::Mouse, platform_linux::native::InputKind::Keyboard => bindforge_access::Kind::Keyboard };
        let target = bindforge_access::inspect(&device.interface_id, kind)?;
        bindforge_access::ensure_not_revoked(&target)?;
        Ok(std::sync::Arc::new(move || bindforge_access::ensure_not_revoked(&target)))
    }));
    let _session_guard = platform_linux::native::session::SessionGuard::system(input.clone())?;
    let registrations = |config: &remap_config::Config| config.devices.iter().map(|d| platform_linux::native::DeviceRegistration {
        id: d.id.clone(), name: d.name.clone(), interface_id: d.interface_id.clone(),
        kind: if d.kind == "keyboard" { platform_linux::native::InputKind::Keyboard } else { platform_linux::native::InputKind::Mouse },
    }).collect();
    input.devices(registrations(&config));
    let mut switcher = ProfileSwitcher::new(config.validate()?, input.clone());
    let status = StatusStore::default();
    let mut monitor = platform_kde::dbus::KdeWindowMonitor::session_with_input(status.clone(), input.clone())?;
    let notify = monitor.status_notifier();
    let mut last_event = WindowEvent::NoActiveWindow;
    let mut focus = FocusState::Waiting;
    let mut last_error = None;
    let mut last_input_state = input.statuses();
    eprintln!("Input backend ready but PAUSED: activate a registered mouse or keyboard explicitly in the app.");
    let result = (|| {
        switcher.handle(&last_event)?;
        println!("simulation profile={}", switcher.active_id().unwrap_or("default"));
        publish_status(Some(&status), Some(&notify), switcher.active_id(), focus)?;
        loop {
            if let Some(Some(signal)) = futures_lite::future::block_on(futures_lite::future::poll_once(signals.next())) {
                signal.map_err(|e| e.to_string())?;
                break;
            }
            // Authenticate focus/owner changes before re-evaluating saved rules.
            let event = monitor.poll_event()?;
            if let Some(event) = &event {
                input.focus(matches!(event, WindowEvent::Focused(_)));
                last_event = event.clone();
                focus = match event { WindowEvent::Focused(_) => FocusState::Focused, WindowEvent::NoActiveWindow => FocusState::NoActiveWindow };
            }
            let mut reloaded = false;
            match remap_config::load(&path) {
                Ok(next) => {
                    if next != config {
                        if next.devices != config.devices { input.devices(registrations(&next)); }
                        match switcher.replace_profiles(next.validate()?, &last_event) {
                            Ok(()) => { config = next; reloaded = true; last_error = None; eprintln!("Saved profiles loaded: revision {}.", config.revision); },
                            Err(e) => { if last_error.as_ref() != Some(&e) { eprintln!("Native mapping rejected: {e}"); } last_error = Some(e); },
                        }
                    }
                }
                Err(error) => {
                    // Keep the last valid policy. Do not flood logs on every idle tick.
                    if last_error.as_ref() != Some(&error) { eprintln!("Configuration reload rejected: {error}"); }
                    last_error = Some(error);
                }
            }
            let changed = if event.is_some() {
                match switcher.handle(&last_event) {
                    Ok(changed) => changed,
                    Err(e) => { input.pause_for_error(format!("Remapeamento pausado ao trocar de perfil: {e}")); switcher.handle(&last_event)? }
                }
            } else { false };
            let input_state = input.statuses();
            let input_changed = input_state != last_input_state;
            last_input_state = input_state.clone();
            let simulation = !input_state.iter().any(|s| s.active);
            let mode_changed = status.snapshot().simulation != simulation;
            status.set_simulation(simulation);
            if changed || reloaded { println!("{} profile={}", if simulation { "simulation" } else { "native" }, switcher.active_id().unwrap_or("default")); }
            if event.is_some() || reloaded || mode_changed || input_changed {
                publish_status(Some(&status), Some(&notify), switcher.active_id(), focus)?;
            }
            if once && event.is_some() { break; }
        }
        Ok(())
    })();
    let cleanup = switcher.reset();
    status.set_simulation(true);
    status.stopped();
    let notification = notify(&status.snapshot());
    result.and(cleanup).and(notification)
}

fn run() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.as_slice() {
        [] => drive(&mut StdinMonitor(io::Cursor::new(include_bytes!("../../../examples/focus-events.txt")))),
        #[cfg(all(feature = "linux-devices", target_os = "linux"))]
        [flag] if flag == "--list-devices" => {
            use remap_core::DeviceEnumerator;
            let devices = platform_linux::LinuxDeviceEnumerator.enumerate()?;
            println!("Read-only inventory: {} input interface(s); no input streams opened.", devices.len());
            for device in devices {
                println!("{:?}\t{}\t{:?}", device.kind, device.name, device.connection);
            }
            Ok(())
        }
        [flag] if flag == "--list-devices" => Err("Device inventory requires Linux and --features linux-devices".into()),
        #[cfg(all(feature = "kde-dbus", target_os = "linux"))]
        [flag] if flag == "--native-self-test" => {
            platform_linux::native::isolated_self_test()?;
            println!("PASS: isolated virtual input -> remapper -> virtual output, V press/release verified."); Ok(())
        }
        #[cfg(all(feature = "kde-dbus", target_os = "linux"))]
        [flag] if flag == "--keyboard-self-test" => {
            platform_linux::native::isolated_keyboard_self_test()?;
            println!("PASS: isolated keyboard mapping, repeat and held-key profile switch verified."); Ok(())
        }
        [flag] if flag == "--stdin" => drive(&mut StdinMonitor(io::stdin().lock())),
        #[cfg(all(feature = "kde-dbus", target_os = "linux"))]
        [flag] if flag == "--kde" => {
            eprintln!("KDE focus bridge: automatic session D-Bus; mouse and keyboard backends start paused.");
            run_kde(false)
        }
        #[cfg(all(feature = "kde-dbus", target_os = "linux"))]
        [flag, once] if flag == "--kde" && once == "--once" => {
            eprintln!("KDE focus bridge: waiting for one authenticated event; simulation only.");
            run_kde(true)
        }
        [flag] if flag == "--kde" => Err("KDE support requires Linux and --features kde-dbus".into()),
        [flag] if flag == "--help" || flag == "-h" => {
            println!("bindforge-daemon [--stdin | --kde [--once] | --list-devices | --native-self-test | --keyboard-self-test]\nDefault: finite demo. --stdin: one exact app ID per line; blank line clears focus.\nDefault and --stdin simulate only. --kde: live focus bridge with native mouse and keyboard initially PAUSED; activate saved devices explicitly in the app. --native-self-test / --keyboard-self-test: exclusively isolated virtual fixtures, no physical devices. EOF resets and exits. --once exits after the first authenticated focus event (no timeout)."); Ok(())
        }
        _ => Err("unknown arguments; use --help".into()),
    }
}
fn main() {
    if let Err(error) = run() { eprintln!("error: {error}"); std::process::exit(1); }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stream_distinguishes_no_focus_from_eof() {
        let mut monitor = StdinMonitor(io::Cursor::new(b"firefox\n\n"));
        assert!(matches!(monitor.next_event().unwrap(), Some(WindowEvent::Focused(_))));
        assert_eq!(monitor.next_event().unwrap(), Some(WindowEvent::NoActiveWindow));
        assert_eq!(monitor.next_event().unwrap(), None);
    }
    #[test]
    fn bounds_and_invalid_input() {
        assert!(StdinMonitor(io::Cursor::new(vec![b'x'; MAX_LINE + 1])).next_event().is_err());
        assert!(StdinMonitor(io::Cursor::new(b"a\tb\n")).next_event().is_err());
        assert!(StdinMonitor(io::Cursor::new([0xff])).next_event().is_err());
    }
    struct CountingMonitor { reads: usize, fail: bool }
    impl WindowMonitor for CountingMonitor {
        fn next_event(&mut self) -> Result<Option<WindowEvent>, String> {
            self.reads += 1;
            assert_eq!(self.reads, 1, "one-shot must not wait for a second event");
            if self.fail { Err("monitor disconnected".into()) }
            else { Ok(Some(WindowEvent::NoActiveWindow)) }
        }
    }
    #[test]
    fn one_shot_consumes_exactly_one_event() {
        let mut monitor = CountingMonitor { reads: 0, fail: false };
        drive_until(&mut monitor, true).unwrap();
        assert_eq!(monitor.reads, 1);
    }
    #[test]
    fn one_shot_propagates_monitor_failure() {
        let mut monitor = CountingMonitor { reads: 0, fail: true };
        assert_eq!(drive_until(&mut monitor, true).unwrap_err(), "monitor disconnected");
    }

    #[test]
    fn notification_failure_resets_simulation_and_marks_status_stopped() {
        let status = StatusStore::default();
        let mut monitor = CountingMonitor { reads: 0, fail: false };
        let notify = |_: &remap_ipc::DaemonStatus| Err("bus disconnected".into());
        assert_eq!(drive_with_status(&mut monitor, false, Some(&status), Some(&notify)).unwrap_err(), "bus disconnected");
        assert_eq!(monitor.reads, 0);
        assert_eq!(status.snapshot().focus_state, FocusState::Stopped);
        assert_eq!(status.snapshot().active_profile_id, None);
    }

}
