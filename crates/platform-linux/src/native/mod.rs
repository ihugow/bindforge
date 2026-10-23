//! One explicitly selected physical mouse. Paused by default; no keyboard grabs.
mod engine;
mod transport;
#[cfg(feature = "dbus")]
pub mod session;
pub use engine::{Engine, Plan};
pub use transport::isolated_self_test;
use std::sync::{Arc, Mutex};
use remap_core::{InputRemapper, Profile, RemapMode};
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MouseRegistration { pub id: String, pub name: String, pub interface_id: String }
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct NativeStatus { pub active: bool, pub device_id: String, pub message: String }
#[derive(Default)]
struct State { restriction: Option<SessionRestriction>, focus_valid: bool, devices: Vec<MouseRegistration>, profile: Option<Profile>, worker: Option<transport::Worker>, message: String }
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
            Self::Checking => "Sessão verificada. Ative um mouse para aplicar seus mapeamentos.",
            Self::Locked => "Remapeamento pausado após bloqueio da sessão. Sessão disponível; ative novamente para retomar.",
            Self::Sleeping => "Remapeamento pausado após suspensão. Sessão disponível; ative novamente para retomar.",
            Self::Inactive => "Remapeamento pausado após troca de sessão. Sessão disponível; ative novamente para retomar.",
            Self::Unavailable => "Monitor de sessão reconectado. O remapeamento permanece pausado; ative novamente para retomar.",
        }
    }
}
#[derive(Clone, Default)]
pub struct MouseSession(Arc<Mutex<State>>);
impl MouseSession {
    /// Serializes session restrictions with StartMouse, including a concurrent activation.
    pub fn restrict_session(&self, restriction: Option<SessionRestriction>) {
        let mut state = self.0.lock().unwrap_or_else(|e| e.into_inner());
        if state.restriction == restriction { return; }
        let previous = state.restriction;
        state.restriction = restriction;
        if let Some(reason) = restriction {
            if let Some(worker) = state.worker.take() { worker.stop(); }
            state.focus_valid = false;
            state.message = reason.message().into();
        } else if let Some(previous) = previous {
            state.message = previous.resumed_message().into();
        }
    }
    pub fn focus(&self, valid: bool) {
        let mut state = self.0.lock().unwrap_or_else(|e| e.into_inner()); state.focus_valid = valid && state.restriction.is_none();
        if !valid {
            if let Some(worker) = state.worker.take() { worker.stop(); state.message = "Remapeamento pausado por perda de foco/janela. Ative novamente quando a ponte estiver conectada.".into(); }
        }
    }
    pub fn devices(&self, devices: Vec<MouseRegistration>) {
        let mut state = self.0.lock().unwrap_or_else(|e| e.into_inner());
        if state.devices != devices {
            if let Some(worker) = state.worker.take() { worker.stop(); state.message = "Cadastro alterado. Ative novamente o mouse desejado.".into(); }
            state.devices = devices;
        }
    }
    pub fn status(&self) -> NativeStatus {
        let state = self.0.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(worker) = &state.worker {
            let mut status = worker.status();
            if status.message.is_empty() { status.message = state.message.clone(); }
            status
        } else { NativeStatus { message: state.message.clone(), ..Default::default() } }
    }
    pub fn start(&self, id: &str) -> Result<(), String> {
        let mut state = self.0.lock().map_err(|e| e.to_string())?;
        if let Some(reason) = state.restriction { return Err(reason.message().into()); }
        if !state.focus_valid { return Err("Aguarde a ponte KWin identificar uma janela ativa antes de ativar o mouse.".into()); }
        if state.worker.as_ref().is_some_and(|w| w.status().active) { return Err("Pause o mouse atual antes de ativar outro.".into()); }
        if let Some(worker) = state.worker.take() { worker.stop(); }
        let device = state.devices.iter().find(|d| d.id == id).ok_or("Selecione um mouse cadastrado e salvo no aplicativo.")?.clone();
        let profile = state.profile.as_ref().ok_or("Aguarde a seleção inicial de perfil pelo daemon.")?;
        let plan = Plan::compile(profile, &device.interface_id)?;
        match transport::Worker::start(device, plan) {
            Ok(worker) => { state.worker = Some(worker); state.message.clear(); Ok(()) },
            Err(error) => { state.message = error.clone(); Err(error) },
        }
    }
    pub fn pause_for_error(&self, message: String) {
        self.pause(); self.0.lock().unwrap_or_else(|e| e.into_inner()).message = message;
    }
    pub fn pause(&self) {
        let mut state = self.0.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(worker) = state.worker.take() { worker.stop(); }
        state.message = state.restriction.map(|r| r.message()).unwrap_or("Remapeamento pausado. O mouse está com seu funcionamento original.").into();
    }
}
impl InputRemapper for MouseSession {
    fn mode(&self) -> RemapMode { if self.status().active { RemapMode::Native } else { RemapMode::Simulation } }
    fn apply_profile(&mut self, profile: &Profile) -> Result<(), String> {
        let mut state = self.0.lock().map_err(|e| e.to_string())?;
        if let Some(worker) = &state.worker {
            if worker.status().active {
                let result = Plan::compile(profile, worker.interface_id()).and_then(|plan| worker.apply(plan));
                if let Err(e) = result { state.message = e.clone(); return Err(e); }
            }
        }
        let active = state.worker.as_ref().is_some_and(|w| w.status().active);
        state.profile = Some(profile.clone()); if active { state.message.clear(); } Ok(())
    }
    fn reset(&mut self) -> Result<(), String> { self.pause(); Ok(()) }
}
#[cfg(feature = "dbus")]
pub struct MouseInterface(pub MouseSession);
#[cfg(feature = "dbus")]
#[zbus::interface(name = "org.bindforge.Daemon.Input1")]
impl MouseInterface {
    fn get_input_status(&self) -> (u32, bool, String, String) {
        let s = self.0.status(); (1, s.active, s.device_id, s.message)
    }
    async fn start_mouse(&self, device_id: String) -> zbus::fdo::Result<()> {
        if device_id.is_empty() || device_id.len() > 128 || device_id.chars().any(char::is_control) { return Err(zbus::fdo::Error::InvalidArgs("Invalid device ID".into())); }
        let session = self.0.clone();
        blocking::unblock(move || session.start(&device_id)).await.map_err(zbus::fdo::Error::Failed)
    }
    async fn pause_mouse(&self) {
        let session = self.0.clone(); blocking::unblock(move || session.pause()).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn session_starts_paused_and_never_opens_unregistered_paths_or_devices() {
        let mut session = MouseSession::default();
        assert!(!session.status().active); assert_eq!(session.mode(), RemapMode::Simulation);
        assert!(session.start("/dev/input/event0").is_err());
        session.apply_profile(&remap_core::demo_profiles().profiles()[0]).unwrap();
        session.focus(true);
        assert!(session.start("../../dev/input/event0").is_err());
        session.pause_for_error("fixture error".into());
        session.apply_profile(&remap_core::demo_profiles().profiles()[1]).unwrap();
        assert_eq!(session.status().message, "fixture error");
        session.reset().unwrap(); assert!(!session.status().active);
    }
}
