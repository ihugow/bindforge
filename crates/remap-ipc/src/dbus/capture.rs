//! Explicit, short-lived capture; never exposes a continuous focus stream.
use std::time::Duration;
use zbus::blocking::{connection::Builder, Connection, Proxy};
use super::{SERVICE, PATH};
const INTERFACE: &str = "org.bindforge.Daemon.Capture1";

pub struct WindowCapture { connection: Connection, owner: String, id: u64 }
impl WindowCapture {
    pub fn session() -> Result<Self, String> {
        Self::begin(Builder::session().map_err(|e| e.to_string())?)
    }
    pub fn at_address(address: &str) -> Result<Self, String> {
        Self::begin(Builder::address(address).map_err(|e| e.to_string())?)
    }
    fn begin(builder: Builder<'_>) -> Result<Self, String> {
        let connection = builder.method_timeout(Duration::from_secs(2)).build().map_err(|e| e.to_string())?;
        let bus = zbus::blocking::fdo::DBusProxy::new(&connection).map_err(|e| e.to_string())?;
        let owner = bus.get_name_owner(SERVICE.try_into().expect("valid service name"))
            .map_err(|_| "Inicie o daemon com --kde para detectar janelas.".to_string())?.to_string();
        let proxy = Proxy::new(&connection, owner.as_str(), PATH, INTERFACE).map_err(|e| e.to_string())?;
        let id = proxy.call("BeginCapture", &()).map_err(|e| match &e {
            zbus::Error::MethodError(name, _, _) if name.as_str() == "org.freedesktop.DBus.Error.UnknownInterface" || name.as_str() == "org.freedesktop.DBus.Error.UnknownMethod" => "Reinicie o daemon para habilitar a detecção de janelas desta versão.".into(),
            _ => format!("Não foi possível iniciar a captura: {e}"),
        })?;
        drop(proxy); drop(bus);
        Ok(Self { connection, owner, id })
    }
    pub fn poll(&self) -> Result<Option<String>, String> {
        let bus = zbus::blocking::fdo::DBusProxy::new(&self.connection).map_err(|e| e.to_string())?;
        let current = bus.get_name_owner(SERVICE.try_into().expect("valid service name")).map_err(|_| "O daemon foi desconectado. Inicie a captura novamente.".to_string())?;
        if current.as_str() != self.owner { return Err("O daemon foi reiniciado. Inicie a captura novamente.".into()); }
        let proxy = Proxy::new(&self.connection, self.owner.as_str(), PATH, INTERFACE).map_err(|e| e.to_string())?;
        let (status, app): (String, String) = proxy.call("ReadCapture", &(self.id,)).map_err(|e| format!("Captura interrompida: {e}"))?;
        match status.as_str() {
            "waiting" if app.is_empty() => Ok(None),
            "captured" if !app.trim().is_empty() && app.len() <= 4096 && !app.chars().any(char::is_control) => Ok(Some(app)),
            _ => Err("Resposta de captura inválida.".into()),
        }
    }
}
impl Drop for WindowCapture {
    fn drop(&mut self) {
        if let Ok(proxy) = Proxy::new(&self.connection, self.owner.as_str(), PATH, INTERFACE) {
            let _ = proxy.call::<_, _, ()>("CancelCapture", &(self.id,));
        }
    }
}
