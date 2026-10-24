//! Explicitly selected physical mouse and keyboard, both paused by default.
mod engine;
mod transport;
#[cfg(feature = "dbus")]
pub mod session;
pub use engine::{Engine, Plan};
pub use transport::{isolated_self_test, isolated_keyboard_self_test};
use std::sync::{Arc, Mutex};
use remap_core::{InputRemapper, Profile, RemapMode};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputKind { Mouse, Keyboard }
impl InputKind {
    fn index(self) -> usize { match self { Self::Mouse => 0, Self::Keyboard => 1 } }
    pub(super) fn name(self) -> &'static str { match self { Self::Mouse => "mouse", Self::Keyboard => "teclado" } }
    fn compile(self, profile: &Profile, interface: &str) -> Result<Plan, String> {
        match self { Self::Mouse => Plan::compile(profile, interface), Self::Keyboard => Plan::compile_keyboard(profile, interface) }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeviceRegistration { pub id: String, pub name: String, pub interface_id: String, pub kind: InputKind }
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct NativeStatus { pub active: bool, pub device_id: String, pub message: String }
#[derive(Default)]
struct Slot { worker: Option<transport::Worker>, message: String }
impl Slot {
    fn stop(&mut self, message: &str) {
        if let Some(worker) = self.worker.take() { worker.stop(); }
        self.message = message.into();
    }
    fn status(&self) -> NativeStatus {
        if let Some(worker) = &self.worker {
            let mut status = worker.status();
            if status.message.is_empty() { status.message = self.message.clone(); }
            status
        } else { NativeStatus { message: self.message.clone(), ..Default::default() } }
    }
}
/// Application policy checked before opening the physical device and while capturing.
pub type CaptureGuard = Arc<dyn Fn() -> Result<(), String> + Send + Sync>;
pub type AccessPolicy = Arc<dyn Fn(&DeviceRegistration) -> Result<CaptureGuard, String> + Send + Sync>;
#[derive(Default)]
struct State { policy: Option<AccessPolicy>, restriction: Option<SessionRestriction>, focus_valid: bool, devices: Vec<DeviceRegistration>, profile: Option<Profile>, slots: [Slot; 2] }
impl State {
    fn stop(&mut self, message: &str) { for slot in &mut self.slots { slot.stop(message); } }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SessionRestriction { Checking, Locked, Sleeping, Inactive, Unavailable }
impl SessionRestriction {
    fn message(self) -> &'static str {
        match self {
            Self::Checking => "Aguardando verificação da sessão e suspensão.",
            Self::Locked => "Remapeamento pausado por bloqueio da sessão.",
            Self::Sleeping => "Remapeamento pausado para suspensão do computador.",
            Self::Inactive => "Remapeamento pausado porque a sessão não está ativa.",
            Self::Unavailable => "Remapeamento pausado: monitor de sessão e suspensão indisponível.",
        }
    }
    fn resumed_message(self) -> &'static str {
        match self {
            Self::Checking => "Sessão verificada. Ative um dispositivo para aplicar seus mapeamentos.",
            Self::Locked => "Remapeamento pausado após bloqueio da sessão. Sessão disponível; ative novamente para retomar.",
            Self::Sleeping => "Remapeamento pausado após suspensão. Sessão disponível; ative novamente para retomar.",
            Self::Inactive => "Remapeamento pausado após troca de sessão. Sessão disponível; ative novamente para retomar.",
            Self::Unavailable => "Monitor de sessão reconectado. O remapeamento permanece pausado; ative novamente para retomar.",
        }
    }
}
#[derive(Clone, Default)]
pub struct InputSession(Arc<Mutex<State>>);
impl InputSession {
    pub fn with_access_policy(policy: AccessPolicy) -> Self {
        Self(Arc::new(Mutex::new(State { policy: Some(policy), ..Default::default() })))
    }
    /// The same lock serializes both activations, profile updates and session restrictions.
    pub fn restrict_session(&self, restriction: Option<SessionRestriction>) {
        let mut state = self.0.lock().unwrap_or_else(|e| e.into_inner());
        if state.restriction == restriction { return; }
        let previous = state.restriction;
        state.restriction = restriction;
        if let Some(reason) = restriction { state.stop(reason.message()); state.focus_valid = false; }
        else if let Some(previous) = previous { for slot in &mut state.slots { slot.message = previous.resumed_message().into(); } }
    }
    pub fn focus(&self, valid: bool) {
        let mut state = self.0.lock().unwrap_or_else(|e| e.into_inner());
        state.focus_valid = valid && state.restriction.is_none();
        if !valid { state.stop("Remapeamento pausado por perda de foco/janela. Ative novamente quando a ponte estiver conectada."); }
    }
    pub fn devices(&self, devices: Vec<DeviceRegistration>) {
        let mut state = self.0.lock().unwrap_or_else(|e| e.into_inner());
        for kind in [InputKind::Mouse, InputKind::Keyboard] {
            if state.devices.iter().filter(|d| d.kind == kind).ne(devices.iter().filter(|d| d.kind == kind)) {
                state.slots[kind.index()].stop("Cadastro alterado. Ative novamente o dispositivo desejado.");
            }
        }
        state.devices = devices;
    }
    pub fn statuses(&self) -> [NativeStatus; 2] {
        let state = self.0.lock().unwrap_or_else(|e| e.into_inner());
        [state.slots[0].status(), state.slots[1].status()]
    }
    pub fn status(&self) -> NativeStatus { self.statuses()[0].clone() }
    pub fn keyboard_status(&self) -> NativeStatus { self.statuses()[1].clone() }
    pub fn start(&self, id: &str) -> Result<(), String> { self.start_kind(InputKind::Mouse, id) }
    pub fn start_keyboard(&self, id: &str) -> Result<(), String> { self.start_kind(InputKind::Keyboard, id) }
    fn start_kind(&self, kind: InputKind, id: &str) -> Result<(), String> {
        let mut state = self.0.lock().map_err(|e| e.to_string())?;
        if let Some(reason) = state.restriction { return Err(reason.message().into()); }
        if !state.focus_valid { return Err("Aguarde a ponte KWin identificar uma janela ativa antes de ativar o dispositivo.".into()); }
        let index = kind.index();
        if state.slots[index].status().active { return Err(format!("Pause o {} atual antes de ativar outro.", kind.name())); }
        let device = state.devices.iter().find(|d| d.id == id && d.kind == kind)
            .ok_or_else(|| format!("Selecione um {} cadastrado e salvo no aplicativo.", kind.name()))?.clone();
        if state.slots.iter().filter_map(|slot| slot.worker.as_ref()).any(|worker| worker.status().active && worker.interface_id() == device.interface_id) {
            return Err("Esta interface composta já está sendo capturada. Pause o outro dispositivo antes de ativar este.".into());
        }
        let profile = state.profile.as_ref().ok_or("Aguarde a seleção inicial de perfil pelo daemon.")?;
        let plan = kind.compile(profile, &device.interface_id)?;
        if let Some(worker) = state.slots[index].worker.take() { worker.stop(); }
        let guard = state.policy.as_ref().ok_or("Reinicie o daemon para habilitar a verificação de autorizações.")?(&device)?;
        guard()?;
        match transport::Worker::start(device, plan, guard) {
            Ok(worker) => { state.slots[index].worker = Some(worker); state.slots[index].message.clear(); Ok(()) },
            Err(error) => { state.slots[index].message = error.clone(); Err(error) },
        }
    }
    pub fn pause_for_error(&self, message: String) {
        self.0.lock().unwrap_or_else(|e| e.into_inner()).stop(&message);
    }
    fn pause_kind(&self, kind: InputKind) {
        let mut state = self.0.lock().unwrap_or_else(|e| e.into_inner());
        let message = state.restriction.map(|r| r.message().to_string()).unwrap_or_else(|| format!("Remapeamento pausado. O {} está com seu funcionamento original.", kind.name()));
        state.slots[kind.index()].stop(&message);
    }
    pub fn pause_mouse(&self) { self.pause_kind(InputKind::Mouse); }
    pub fn pause_keyboard(&self) { self.pause_kind(InputKind::Keyboard); }
    pub fn pause(&self) {
        let mut state = self.0.lock().unwrap_or_else(|e| e.into_inner());
        let message = state.restriction.map(|r| r.message()).unwrap_or("Remapeamento pausado. Os dispositivos estão com seu funcionamento original.");
        state.stop(message);
    }
}
impl InputRemapper for InputSession {
    fn mode(&self) -> RemapMode { if self.statuses().iter().any(|s| s.active) { RemapMode::Native } else { RemapMode::Simulation } }
    fn apply_profile(&mut self, profile: &Profile) -> Result<(), String> {
        let mut state = self.0.lock().map_err(|e| e.to_string())?;
        // Compile every active map before changing either worker.
        let plans = [InputKind::Mouse, InputKind::Keyboard].into_iter().map(|kind| {
            state.slots[kind.index()].worker.as_ref().filter(|worker| worker.status().active)
                .map(|worker| kind.compile(profile, worker.interface_id())).transpose()
        }).collect::<Result<Vec<_>, _>>();
        let plans = match plans {
            Ok(plans) => plans,
            Err(error) => {
                for slot in &mut state.slots { if slot.status().active { slot.message = format!("Mapa rejeitado; plano anterior mantido: {error}"); } }
                return Err(error);
            }
        };
        for (index, plan) in plans.into_iter().enumerate() {
            if let Some(plan) = plan {
                if let Err(error) = state.slots[index].worker.as_ref().expect("compiled worker").apply(plan) {
                    state.stop(&format!("Remapeamento pausado ao aplicar perfil: {error}")); return Err(error);
                }
            }
        }
        state.profile = Some(profile.clone());
        for slot in &mut state.slots { if slot.status().active { slot.message.clear(); } }
        Ok(())
    }
    fn reset(&mut self) -> Result<(), String> { self.pause(); Ok(()) }
}
#[cfg(feature = "dbus")]
pub struct InputInterface(pub InputSession);
#[cfg(feature = "dbus")]
#[zbus::interface(name = "org.bindforge.Daemon.Input1")]
impl InputInterface {
    fn get_input_status(&self) -> (u32, bool, String, String) {
        let s = self.0.status(); (1, s.active, s.device_id, s.message)
    }
    fn get_keyboard_status(&self) -> (u32, bool, String, String) {
        let s = self.0.keyboard_status(); (1, s.active, s.device_id, s.message)
    }
    async fn start_mouse(&self, device_id: String) -> zbus::fdo::Result<()> { self.start(InputKind::Mouse, device_id).await }
    async fn start_keyboard(&self, device_id: String) -> zbus::fdo::Result<()> { self.start(InputKind::Keyboard, device_id).await }
    async fn pause_mouse(&self) {
        let session = self.0.clone(); blocking::unblock(move || session.pause_mouse()).await;
    }
    async fn pause_keyboard(&self) {
        let session = self.0.clone(); blocking::unblock(move || session.pause_keyboard()).await;
    }
}
#[cfg(feature = "dbus")]
impl InputInterface {
    async fn start(&self, kind: InputKind, id: String) -> zbus::fdo::Result<()> {
        if id.is_empty() || id.len() > 128 || id.chars().any(char::is_control) { return Err(zbus::fdo::Error::InvalidArgs("Invalid device ID".into())); }
        let session = self.0.clone();
        blocking::unblock(move || session.start_kind(kind, &id)).await.map_err(zbus::fdo::Error::Failed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn session_starts_paused_and_never_opens_unregistered_paths_or_devices() {
        let mut session = InputSession::default();
        assert!(session.statuses().iter().all(|s| !s.active)); assert_eq!(session.mode(), RemapMode::Simulation);
        assert!(session.start("/dev/input/event0").is_err());
        assert!(session.start_keyboard("/dev/input/event0").is_err());
        session.apply_profile(&remap_core::demo_profiles().profiles()[0]).unwrap();
        session.focus(true);
        assert!(session.start("../../dev/input/event0").is_err());
        assert!(session.start_keyboard("../../dev/input/event0").is_err());
        session.pause_for_error("fixture error".into());
        session.apply_profile(&remap_core::demo_profiles().profiles()[1]).unwrap();
        assert!(session.statuses().iter().all(|s| s.message == "fixture error"));
        session.reset().unwrap(); assert!(session.statuses().iter().all(|s| !s.active));
    }
    #[test]
    fn keyboard_activation_cannot_bypass_focus_restriction_or_registration_kind() {
        let mut session = InputSession::default();
        session.apply_profile(&remap_core::demo_profiles().profiles()[0]).unwrap();
        session.devices(vec![DeviceRegistration { id: "mouse".into(), name: "Mouse".into(), interface_id: "not-a-path".into(), kind: InputKind::Mouse }]);
        session.focus(true);
        assert!(session.start_keyboard("mouse").unwrap_err().contains("teclado cadastrado"));
        for reason in [SessionRestriction::Locked, SessionRestriction::Sleeping, SessionRestriction::Unavailable, SessionRestriction::Inactive] {
            session.restrict_session(Some(reason)); session.focus(true); session.pause_keyboard();
            assert_eq!(session.start_keyboard("mouse").unwrap_err(), reason.message());
            assert!(session.keyboard_status().message.contains("pausado"));
            session.restrict_session(None);
            assert!(session.start_keyboard("mouse").unwrap_err().contains("ponte KWin"));
        }
    }
    fn populated_session(fail_keyboard: bool) -> (InputSession, Arc<Mutex<Vec<Plan>>>, Arc<Mutex<Vec<Plan>>>) {
        let session = InputSession::default();
        let applied_mouse = Arc::new(Mutex::new(vec![])); let applied_keyboard = Arc::new(Mutex::new(vec![]));
        for (kind, applied, fail) in [(InputKind::Mouse, applied_mouse.clone(), false), (InputKind::Keyboard, applied_keyboard.clone(), fail_keyboard)] {
            let device = DeviceRegistration { id: kind.name().into(), name: kind.name().into(), interface_id: kind.name().into(), kind };
            let worker = transport::Worker::fixture(device.clone(), applied, fail);
            let mut state = session.0.lock().unwrap(); state.devices.push(device); state.slots[kind.index()].worker = Some(worker); state.focus_valid = true;
        }
        (session, applied_mouse, applied_keyboard)
    }
    #[test]
    fn dual_sessions_compile_before_mutating_either_and_io_failure_stops_both() {
        let (mut session, mouse, keyboard) = populated_session(false);
        let p = remap_core::demo_profiles().profiles()[0].clone();
        session.apply_profile(&p).unwrap();
        assert_eq!(session.mode(), RemapMode::Native);
        let mut invalid = p.clone();
        invalid.bindings.push(remap_core::Binding { device_id: None, input: remap_core::InputControl::Key("invalid".into()), action: remap_core::Action::Disabled });
        assert!(session.apply_profile(&invalid).is_err());
        assert!(session.statuses().iter().all(|s| s.active));
        assert_eq!(mouse.lock().unwrap().len(), 1); assert_eq!(keyboard.lock().unwrap().len(), 1);
        session.pause_keyboard(); assert!(session.status().active); assert!(!session.keyboard_status().active);
        session.reset().unwrap(); assert_eq!(session.mode(), RemapMode::Simulation);
        let (mut session, _, _) = populated_session(true);
        assert!(session.apply_profile(&p).is_err());
        assert!(session.statuses().iter().all(|s| !s.active && s.message.contains("I/O failure")));
    }
    #[test]
    fn shared_interface_cannot_be_grabbed_twice_and_restrictions_stop_both_slots() {
        let (session, _, _) = populated_session(false);
        session.pause_keyboard();
        session.0.lock().unwrap().devices.iter_mut().find(|d| d.kind == InputKind::Keyboard).unwrap().interface_id = "mouse".into();
        session.0.lock().unwrap().profile = Some(remap_core::demo_profiles().profiles()[0].clone());
        assert!(session.start_keyboard("teclado").unwrap_err().contains("interface composta"));
        assert!(session.status().active);
        session.restrict_session(Some(SessionRestriction::Sleeping));
        assert!(session.statuses().iter().all(|s| !s.active));
        session.restrict_session(None); session.focus(true);
        assert!(session.statuses().iter().all(|s| !s.active));
    }

    #[test]
    fn activation_obeys_explicit_denial_before_any_device_open_for_both_kinds() {
        let session = InputSession::with_access_policy(Arc::new(|_| Err("Acesso revogado".into())));
        session.devices([InputKind::Mouse, InputKind::Keyboard].into_iter().map(|kind| DeviceRegistration {
            id: kind.name().into(), name: kind.name().into(), interface_id: "/never/open/this".into(), kind,
        }).collect());
        session.0.lock().unwrap().profile = Some(remap_core::demo_profiles().profiles()[0].clone());
        session.focus(true);
        assert_eq!(session.start("mouse").unwrap_err(), "Acesso revogado");
        assert_eq!(session.start_keyboard("teclado").unwrap_err(), "Acesso revogado");
        assert!(session.statuses().iter().all(|s| !s.active));
    }

}
