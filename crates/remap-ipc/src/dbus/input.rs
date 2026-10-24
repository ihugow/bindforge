use std::time::Duration;
use zbus::blocking::{connection::Builder, Proxy};
use super::{SERVICE, PATH};
pub const INPUT_INTERFACE: &str = "org.bindforge.Daemon.Input1";
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InputStatus { pub active: bool, pub device_id: String, pub message: String }
pub enum InputCommand<'a> { Status, Start(&'a str), Pause, KeyboardStatus, StartKeyboard(&'a str), PauseKeyboard }
pub fn session_input(command: InputCommand<'_>) -> Result<InputStatus, String> {
    let connection = Builder::session().map_err(|e| e.to_string())?.method_timeout(Duration::from_secs(8)).build().map_err(|e| e.to_string())?;
    let bus = zbus::blocking::fdo::DBusProxy::new(&connection).map_err(|e| e.to_string())?;
    let owner = bus.get_name_owner(SERVICE.try_into().expect("valid name")).map_err(|_| "Inicie o daemon com --kde.".to_string())?;
    let proxy = Proxy::new(&connection, owner.as_str(), PATH, INPUT_INTERFACE).map_err(|e| e.to_string())?;
    let keyboard = matches!(command, InputCommand::KeyboardStatus | InputCommand::StartKeyboard(_) | InputCommand::PauseKeyboard);
    // Query capability before a mutating call: old daemons return a clear restart error.
    let method = if keyboard { "GetKeyboardStatus" } else { "GetInputStatus" };
    let read = || -> Result<InputStatus, String> {
        let (version, active, device_id, message): (u32, bool, String, String) = proxy.call(method, &()).map_err(|e| format!("Reinicie o daemon para habilitar este backend de entrada: {e}"))?;
        decode_status(version, active, device_id, message)
    };
    let status = read()?;
    match command {
        InputCommand::Status | InputCommand::KeyboardStatus => return Ok(status),
        InputCommand::Start(id) => proxy.call::<_, _, ()>("StartMouse", &(id,)).map_err(|e| e.to_string())?,
        InputCommand::Pause => proxy.call::<_, _, ()>("PauseMouse", &()).map_err(|e| e.to_string())?,
        InputCommand::StartKeyboard(id) => proxy.call::<_, _, ()>("StartKeyboard", &(id,)).map_err(|e| e.to_string())?,
        InputCommand::PauseKeyboard => proxy.call::<_, _, ()>("PauseKeyboard", &()).map_err(|e| e.to_string())?,
    }
    read()
}
fn decode_status(version: u32, active: bool, device_id: String, message: String) -> Result<InputStatus, String> {
    if version != 1 || device_id.len() > 128 || message.len() > 4096 { return Err("Resposta de entrada inválida.".into()); }
    Ok(InputStatus { active, device_id, message })
}
