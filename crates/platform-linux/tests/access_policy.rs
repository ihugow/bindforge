#![cfg(all(target_os = "linux", feature = "dbus", feature = "native-input"))]
use std::{io::{BufRead, BufReader}, process::{Child, Command, Stdio}, sync::{Arc, atomic::{AtomicBool, Ordering}}};
use platform_linux::native::{InputSession, InputInterface, DeviceRegistration, InputKind};
use remap_core::InputRemapper;
use remap_ipc::dbus::{SERVICE, PATH, INPUT_INTERFACE};
use zbus::blocking::{connection::Builder, Proxy};
struct Bus(Child);
impl Drop for Bus { fn drop(&mut self) { let _ = self.0.kill(); let _ = self.0.wait(); } }
#[test]
fn dbus_activation_cannot_bypass_explicit_revocation() {
    let mut bus = Bus(Command::new("dbus-daemon").args(["--session", "--nofork", "--print-address=1"]).stdout(Stdio::piped()).spawn().unwrap());
    let mut address = String::new(); BufReader::new(bus.0.stdout.take().unwrap()).read_line(&mut address).unwrap();
    let denied = Arc::new(AtomicBool::new(true)); let policy = denied.clone();
    let mut input = InputSession::with_access_policy(Arc::new(move |_| {
        let denied = policy.clone();
        Ok(Arc::new(move || if denied.load(Ordering::Relaxed) { Err("Acesso revogado no BindForge".into()) } else { Ok(()) }))
    }));
    input.apply_profile(&remap_core::demo_profiles().profiles()[0]).unwrap();
    input.devices([InputKind::Mouse, InputKind::Keyboard].into_iter().map(|kind| DeviceRegistration {
        id: if kind == InputKind::Mouse { "mouse" } else { "keyboard" }.into(), name: "Fixture".into(), interface_id: "never-open-a-physical-device".into(), kind,
    }).collect());
    input.focus(true);
    let server = Builder::address(address.trim()).unwrap().serve_at(PATH, InputInterface(input.clone())).unwrap().name(SERVICE).unwrap().build().unwrap();
    let client = Builder::address(address.trim()).unwrap().build().unwrap();
    let proxy = Proxy::new(&client, SERVICE, PATH, INPUT_INTERFACE).unwrap();
    for (method, id) in [("StartMouse", "mouse"), ("StartKeyboard", "keyboard")] {
        let error = proxy.call::<_, _, ()>(method, &(id,)).unwrap_err().to_string();
        assert!(error.contains("Acesso revogado"), "{error}");
    }
    assert!(input.statuses().iter().all(|state| !state.active));
    denied.store(false, Ordering::Relaxed);
    // A new authorization clears the gate; the non-existent fixture is still refused
    // by the physical backend before any node is opened or virtual output is created.
    let error = proxy.call::<_, _, ()>("StartKeyboard", &("keyboard",)).unwrap_err().to_string();
    assert!(!error.contains("revogado"));
    assert!(!input.keyboard_status().active);
    drop(server);
}
