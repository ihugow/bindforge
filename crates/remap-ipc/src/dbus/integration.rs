//! Daemon-owned temporary KWin bridge diagnostics and bounded reconnect requests.
use std::{sync::{Arc, Mutex}, time::{Duration, Instant}};
use zbus::blocking::{connection::Builder, Connection, Proxy};
use super::{SERVICE, PATH};
pub const INTEGRATION_INTERFACE: &str = "org.appbuttons.Remap.Integration1";
pub type WireIntegration = (u32, bool, bool, bool, bool, String, u64);
#[derive(Clone, Debug, Default)]
pub struct BridgeStatus {
    pub kwin_available: bool, pub loaded: bool, pub focus_received: bool,
    pub legacy_disabled: bool, pub message: String, pub completed: u64,
}
#[derive(Default)]
struct Inner { status: BridgeStatus, requested: u64 }
#[derive(Clone, Default)]
pub struct BridgeStore(Arc<Mutex<Inner>>);
impl BridgeStore {
    pub fn snapshot(&self) -> BridgeStatus { self.0.lock().unwrap_or_else(|e| e.into_inner()).status.clone() }
    pub fn update(&self, change: impl FnOnce(&mut BridgeStatus)) { change(&mut self.0.lock().unwrap_or_else(|e| e.into_inner()).status); }
    pub fn requested(&self) -> u64 { self.0.lock().unwrap_or_else(|e| e.into_inner()).requested }
    pub fn request(&self) -> u64 {
        let mut inner = self.0.lock().unwrap_or_else(|e| e.into_inner());
        if inner.requested <= inner.status.completed { inner.requested = inner.status.completed.saturating_add(1); }
        inner.requested
    }
}
pub struct IntegrationInterface(pub BridgeStore);
#[zbus::interface(name = "org.appbuttons.Remap.Integration1")]
impl IntegrationInterface {
    fn get_integration(&self) -> WireIntegration {
        let s = self.0.snapshot(); (1, s.kwin_available, s.loaded, s.focus_received, s.legacy_disabled, s.message, s.completed)
    }
    fn request_reconnect(&self) -> u64 { self.0.request() }
}
fn decode((version, kwin_available, loaded, focus_received, legacy_disabled, message, completed): WireIntegration) -> Result<BridgeStatus, String> {
    if version != 1 || message.len() > 4096 { return Err("Resposta de integração incompatível. Atualize e reinicie o daemon.".into()); }
    Ok(BridgeStatus { kwin_available, loaded, focus_received, legacy_disabled, message, completed })
}
fn client(connection: &Connection) -> Result<Proxy<'_>, String> {
    let owner = zbus::blocking::fdo::DBusProxy::new(connection).map_err(|e| e.to_string())?
        .get_name_owner(SERVICE.try_into().expect("valid name")).map_err(|_| "Inicie o daemon com --kde para conectar a ponte temporária.".to_string())?;
    Proxy::new_owned(connection.clone(), owner.to_string(), PATH, INTEGRATION_INTERFACE).map_err(|e| e.to_string())
}
fn read(proxy: &Proxy<'_>) -> Result<BridgeStatus, String> {
    decode(proxy.call("GetIntegration", &()).map_err(|e| format!("Não foi possível consultar a ponte temporária. Reinicie o daemon com a versão atual: {e}"))?)
}
pub fn read_session_integration() -> Result<BridgeStatus, String> {
    let connection = Builder::session().map_err(|e| e.to_string())?.method_timeout(Duration::from_secs(2)).build().map_err(|e| e.to_string())?;
    let proxy = client(&connection)?;
    read(&proxy)
}
pub fn reconnect_session_integration() -> Result<BridgeStatus, String> {
    let connection = Builder::session().map_err(|e| e.to_string())?.method_timeout(Duration::from_secs(2)).build().map_err(|e| e.to_string())?;
    let proxy = client(&connection)?; // Pin the daemon owner throughout the request.
    let target: u64 = proxy.call("RequestReconnect", &()).map_err(|e| format!("Reinicie o daemon para habilitar a reconexão desta versão: {e}"))?;
    let deadline = Instant::now() + Duration::from_secs(12);
    loop {
        let state = read(&proxy)?;
        if state.completed >= target {
            return if state.loaded { Ok(state) } else { Err(if state.message.is_empty() { "A ponte não conectou ao KWin.".into() } else { state.message }) };
        }
        if Instant::now() >= deadline { return Err("A reconexão ainda não terminou. Use Verificar novamente.".into()); }
        std::thread::sleep(Duration::from_millis(100));
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reconnect_requests_coalesce_and_versions_are_validated() {
        let store = BridgeStore::default(); assert_eq!(store.request(), 1); assert_eq!(store.request(), 1);
        store.update(|s| s.completed = 1); assert_eq!(store.request(), 2);
        assert!(decode((2, true, true, true, false, String::new(), 0)).is_err());
    }
}
