//! UI only queries/reconnects the bridge owned by the daemon. No script installation.
#[derive(serde::Serialize)]
pub struct IntegrationStatus {
    pub kwin_available: bool, pub daemon_connected: bool, pub loaded: bool,
    pub focus_received: bool, pub legacy_disabled: bool, pub message: String,
}
impl From<remap_ipc::dbus::BridgeStatus> for IntegrationStatus {
    fn from(s: remap_ipc::dbus::BridgeStatus) -> Self {
        Self { kwin_available: s.kwin_available, daemon_connected: true, loaded: s.loaded,
            focus_received: s.focus_received, legacy_disabled: s.legacy_disabled, message: s.message }
    }
}
pub fn status() -> Result<IntegrationStatus, String> {
    match remap_ipc::dbus::read_session_integration() {
        Ok(s) => Ok(s.into()),
        Err(error) => {
            if remap_ipc::dbus::read_session_status().is_ok() { return Err(error); }
            let kwin_available = zbus::blocking::connection::Builder::session().ok()
                .and_then(|b| b.method_timeout(std::time::Duration::from_secs(2)).build().ok())
                .and_then(|c| zbus::blocking::fdo::DBusProxy::new(&c).ok()
                    .and_then(|p| p.get_name_owner("org.kde.KWin".try_into().expect("valid name")).ok())).is_some();
            Ok(IntegrationStatus { kwin_available, daemon_connected: false, loaded: false, focus_received: false,
                legacy_disabled: false, message: "Inicie o daemon com --kde. Ele carrega a ponte automaticamente.".into() })
        }
    }
}
pub fn configure() -> Result<IntegrationStatus, String> {
    remap_ipc::dbus::reconnect_session_integration().map(Into::into)
}
