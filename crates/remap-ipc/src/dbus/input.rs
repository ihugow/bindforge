use std::time::Duration;
use zbus::blocking::{connection::Builder, Proxy};
use super::{SERVICE, PATH};
pub const INPUT_INTERFACE: &str = "org.bindforge.Daemon.Input1";
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InputStatus { pub active: bool, pub device_id: String, pub message: String }
pub enum InputCommand<'a> { Status, Start(&'a str), Pause }
pub fn session_input(command: InputCommand<'_>) -> Result<InputStatus, String> {
    let connection = Builder::session().map_err(|e| e.to_string())?.method_timeout(Duration::from_secs(8)).build().map_err(|e| e.to_string())?;
    let bus = zbus::blocking::fdo::DBusProxy::new(&connection).map_err(|e| e.to_string())?;
    let owner = bus.get_name_owner(SERVICE.try_into().expect("valid name")).map_err(|_| "Inicie o daemon com --kde.".to_string())?;
    let proxy = Proxy::new(&connection, owner.as_str(), PATH, INPUT_INTERFACE).map_err(|e| e.to_string())?;
    match command {
        InputCommand::Status => {}, InputCommand::Start(id) => proxy.call::<_, _, ()>("StartMouse", &(id,)).map_err(|e| e.to_string())?,
        InputCommand::Pause => proxy.call::<_, _, ()>("PauseMouse", &()).map_err(|e| e.to_string())?,
    }
    let (version, active, device_id, message): (u32, bool, String, String) = proxy.call("GetInputStatus", &()).map_err(|e| format!("Reinicie o daemon para habilitar o backend do mouse: {e}"))?;
    if version != 1 || device_id.len() > 128 || message.len() > 4096 { return Err("Resposta de entrada inválida.".into()); }
    Ok(InputStatus { active, device_id, message })
}
