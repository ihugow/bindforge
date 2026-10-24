#![cfg(all(target_os = "linux", feature = "dbus", feature = "native-input"))]
use std::{collections::HashMap, io::{BufRead, BufReader, Read}, os::{fd::OwnedFd, unix::net::UnixStream}, process::{Child, Command, Stdio}, sync::{Arc, Mutex}, time::{Duration, Instant}};
use platform_linux::native::{session::SessionGuard, InputSession};
use remap_core::InputRemapper;
use zbus::{blocking::{connection::Builder, Connection}, zvariant::{OwnedObjectPath, Value}};
const SERVICE: &str = "org.freedesktop.login1";
const MANAGER_PATH: &str = "/org/freedesktop/login1";
const SESSION_PATH: &str = "/org/freedesktop/login1/session/test";
const SESSION: &str = "org.freedesktop.login1.Session";
struct Bus { child: Child, address: String }
impl Bus {
    fn start() -> Self {
        let mut child = Command::new("dbus-daemon").args(["--session", "--nofork", "--print-address=1"]).stdout(Stdio::piped()).spawn().unwrap();
        let mut address = String::new(); BufReader::new(child.stdout.take().unwrap()).read_line(&mut address).unwrap();
        Self { child, address: address.trim().into() }
    }
}
impl Drop for Bus { fn drop(&mut self) { let _ = self.child.kill(); let _ = self.child.wait(); } }
#[derive(Default)]
struct State { locked: bool, inactive: bool, sleeping: bool, deny_inhibit: bool, readers: Vec<UnixStream> }
#[derive(Clone)]
struct Manager(Arc<Mutex<State>>);
#[zbus::interface(name = "org.freedesktop.login1.Manager")]
impl Manager {
    fn get_session_by_pid(&self, _pid: u32) -> zbus::fdo::Result<OwnedObjectPath> {
        Err(zbus::fdo::Error::Failed("not in a session cgroup; use graphical session".into()))
    }
    fn get_session(&self, id: &str) -> OwnedObjectPath { assert_eq!(id, "auto"); SESSION_PATH.try_into().unwrap() }
    fn inhibit(&self, what: &str, who: &str, why: &str, mode: &str) -> zbus::fdo::Result<zbus::zvariant::OwnedFd> {
        assert_eq!((what, who, mode), ("sleep", "BindForge", "delay")); assert!(!why.is_empty());
        let mut state = self.0.lock().unwrap();
        if state.deny_inhibit { return Err(zbus::fdo::Error::AccessDenied("fixture denied inhibitor".into())); }
        let (reader, writer) = UnixStream::pair().unwrap(); reader.set_nonblocking(true).unwrap();
        state.readers.push(reader);
        Ok(OwnedFd::from(writer).into())
    }
    #[zbus(property)] fn preparing_for_sleep(&self) -> bool { self.0.lock().unwrap().sleeping }
}
struct Session(Arc<Mutex<State>>);
#[zbus::interface(name = "org.freedesktop.login1.Session")]
impl Session {
    #[zbus(property)] fn locked_hint(&self) -> bool { self.0.lock().unwrap().locked }
    #[zbus(property)] fn active(&self) -> bool { !self.0.lock().unwrap().inactive }
}
fn service(address: &str, state: &Arc<Mutex<State>>) -> Connection {
    Builder::address(address).unwrap().serve_at(MANAGER_PATH, Manager(state.clone())).unwrap()
        .serve_at(SESSION_PATH, Session(state.clone())).unwrap().name(SERVICE).unwrap().build().unwrap()
}
fn wait_for(mut predicate: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while !predicate() { assert!(Instant::now() < deadline, "session state did not converge"); std::thread::sleep(Duration::from_millis(10)); }
}
fn allowed(input: &InputSession) -> bool {
    input.focus(true);
    input.start("missing-device").unwrap_err().contains("cadastrado")
        && input.start_keyboard("missing-device").unwrap_err().contains("cadastrado")
}
fn changed(connection: &Connection, name: &str, value: bool) {
    connection.emit_signal(None::<&str>, SESSION_PATH, "org.freedesktop.DBus.Properties", "PropertiesChanged",
        &(SESSION, HashMap::from([(name, Value::from(value))]), Vec::<String>::new())).unwrap();
}
fn sleep(connection: &Connection, state: &Arc<Mutex<State>>, preparing: bool) {
    state.lock().unwrap().sleeping = preparing;
    connection.emit_signal(None::<&str>, MANAGER_PATH, "org.freedesktop.login1.Manager", "PrepareForSleep", &(preparing,)).unwrap();
}
fn closed(reader: &mut UnixStream) -> bool { matches!(reader.read(&mut [0u8; 1]), Ok(0)) }

#[test]
fn lock_sleep_authentication_owner_recovery_and_inhibitor_lifecycle() {
    let bus = Bus::start();
    let state = Arc::new(Mutex::new(State { locked: true, ..Default::default() }));
    let logind = service(&bus.address, &state);
    let mut input = InputSession::default();
    input.apply_profile(&remap_core::demo_profiles().profiles()[0]).unwrap();
    let guard = SessionGuard::at_address(input.clone(), &bus.address).unwrap();
    wait_for(|| input.status().message.contains("bloqueio"));
    assert!(input.start("missing-device").unwrap_err().contains("bloqueio"));
    assert!(input.start_keyboard("missing-device").unwrap_err().contains("bloqueio"));
    input.pause(); input.focus(true); // Neither a manual pause nor focus bypasses the gate.
    assert!(input.start("missing-device").unwrap_err().contains("bloqueio"));
    assert!(input.start_keyboard("missing-device").unwrap_err().contains("bloqueio"));
    state.lock().unwrap().locked = false;
    changed(&logind, "LockedHint", false);
    wait_for(|| allowed(&input));
    assert!(!input.status().active);
    let impostor = Builder::address(bus.address.as_str()).unwrap().build().unwrap();
    sleep(&impostor, &Arc::new(Mutex::new(State::default())), true);
    changed(&impostor, "LockedHint", true);
    std::thread::sleep(Duration::from_millis(100));
    assert!(allowed(&input), "signals from an unrelated sender must be ignored");
    assert!(!closed(&mut state.lock().unwrap().readers[0]));
    sleep(&logind, &state, true);
    wait_for(|| input.start("missing-device").unwrap_err().contains("suspensão"));
    wait_for(|| closed(&mut state.lock().unwrap().readers[0]));
    input.focus(true);
    assert!(input.start("missing-device").unwrap_err().contains("suspensão"));
    assert!(input.start_keyboard("missing-device").unwrap_err().contains("suspensão"));
    sleep(&logind, &state, false);
    wait_for(|| allowed(&input));
    assert_eq!(state.lock().unwrap().readers.len(), 2);
    assert!(!input.status().active, "resume must never start native input");
    // The snapshot is already unlocked: still consume a brief queued lock transition.
    changed(&logind, "LockedHint", true);
    wait_for(|| input.status().message.contains("bloqueio") && input.status().message.contains("Sessão disponível"));
    assert!(allowed(&input));
    state.lock().unwrap().inactive = true;
    changed(&logind, "Active", false);
    wait_for(|| input.start("missing-device").unwrap_err().contains("não está ativa"));
    state.lock().unwrap().inactive = false;
    changed(&logind, "Active", true);
    wait_for(|| allowed(&input));
    logind.release_name(SERVICE).unwrap();
    wait_for(|| input.start("missing-device").unwrap_err().contains("indisponível"));
    wait_for(|| closed(&mut state.lock().unwrap().readers[1]));
    let replacement = service(&bus.address, &state);
    wait_for(|| allowed(&input));
    assert!(!input.status().active);
    drop(guard);
    assert!(closed(state.lock().unwrap().readers.last_mut().unwrap()));
    assert!(input.start("missing-device").unwrap_err().contains("indisponível"));
    drop(replacement);
}

#[test]
fn unavailable_or_denied_monitor_blocks_activation_and_recovers() {
    let mut bus = Bus::start();
    let input = InputSession::default();
    let guard = SessionGuard::at_address(input.clone(), &bus.address).unwrap();
    wait_for(|| input.status().message.contains("indisponível"));
    let state = Arc::new(Mutex::new(State { deny_inhibit: true, ..Default::default() }));
    let _service = service(&bus.address, &state);
    std::thread::sleep(Duration::from_millis(1200));
    assert!(input.start("missing-device").unwrap_err().contains("indisponível"));
    state.lock().unwrap().deny_inhibit = false;
    wait_for(|| allowed(&input));
    sleep(&_service, &state, true);
    wait_for(|| closed(&mut state.lock().unwrap().readers[0]));
    state.lock().unwrap().deny_inhibit = true;
    sleep(&_service, &state, false);
    wait_for(|| input.start("missing-device").unwrap_err().contains("indisponível"));
    state.lock().unwrap().deny_inhibit = false;
    wait_for(|| allowed(&input));
    bus.child.kill().unwrap(); bus.child.wait().unwrap();
    wait_for(|| input.start("missing-device").unwrap_err().contains("indisponível"));
    wait_for(|| closed(state.lock().unwrap().readers.last_mut().unwrap()));
    drop(guard);
}
