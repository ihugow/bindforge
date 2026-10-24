use std::{sync::mpsc, time::Duration};
use tauri::{menu::{Menu, MenuItem, Submenu}, AppHandle, Emitter};
use remap_ipc::dbus::{session_input, InputCommand, InputStatus};

enum Command { Start(String), Pause }
struct Group { keyboard: bool, devices: Vec<(String, String)>, previous: Option<InputStatus>, action_error: Option<String> }
impl Group {
    fn title(&self) -> &'static str { if self.keyboard { "Teclado" } else { "Mouse" } }
    fn prefix(&self) -> &'static str { if self.keyboard { "tray-start-keyboard:" } else { "tray-start:" } }
    fn pause_id(&self) -> &'static str { if self.keyboard { "tray-pause-keyboard" } else { "tray-pause" } }
}
pub(super) struct Controls([GroupControls; 2]);
struct GroupControls { sender: mpsc::SyncSender<Command>, prefix: &'static str, pause_id: &'static str }
impl Controls {
    pub(super) fn new(app: &AppHandle, menu: &Menu<tauri::Wry>) -> tauri::Result<Self> {
        Ok(Self([GroupControls::new(app, menu, false)?, GroupControls::new(app, menu, true)?]))
    }
    pub(super) fn dispatch(&self, id: &str) { for group in &self.0 { group.dispatch(id); } }
}
impl GroupControls {
    fn new(app: &AppHandle, menu: &Menu<tauri::Wry>, keyboard: bool) -> tauri::Result<Self> {
        let mut group = Group { keyboard, devices: Vec::new(), previous: None, action_error: None };
        let title = group.title();
        let status = MenuItem::new(app, format!("{title}: consultando…"), false, None::<&str>)?;
        let start = Submenu::new(app, format!("Iniciar remapeamento do {}", title.to_lowercase()), false)?;
        let pause = MenuItem::with_id(app, group.pause_id(), format!("Pausar remapeamento do {}", title.to_lowercase()), false, None::<&str>)?;
        menu.append_items(&[&status, &start, &pause])?;
        let (sender, receiver) = mpsc::sync_channel(1);
        let app = app.clone(); let prefix = group.prefix(); let pause_id = group.pause_id();
        // Input1 has no change signal. Keep controls available with the window hidden.
        std::thread::spawn(move || {
            loop {
                if let Err(error) = refresh(&app, &status, &start, &pause, &mut group) {
                    eprintln!("Não foi possível atualizar o menu de remapeamento: {error}");
                }
                match receiver.recv_timeout(Duration::from_secs(2)) {
                    Ok(command) => {
                        let _ = start.set_enabled(false); let _ = pause.set_enabled(false);
                        group.action_error = execute(command, keyboard).err();
                    }
                    Err(mpsc::RecvTimeoutError::Timeout) => {}
                    Err(mpsc::RecvTimeoutError::Disconnected) => break,
                }
            }
        });
        Ok(Self { sender, prefix, pause_id })
    }
    fn dispatch(&self, id: &str) {
        let command = if id == self.pause_id { Command::Pause }
            else if let Some(id) = id.strip_prefix(self.prefix) { Command::Start(id.into()) }
            else { return };
        let _ = self.sender.try_send(command);
    }
}
fn saved_devices(keyboard: bool) -> Result<Vec<(String, String)>, String> {
    let config = remap_config::load(&remap_config::config_path()?)?;
    config.validate()?;
    Ok(config.devices.into_iter().filter(|d| d.kind == if keyboard { "keyboard" } else { "mouse" }).map(|d| (d.id, d.name)).collect())
}
fn execute(command: Command, keyboard: bool) -> Result<InputStatus, String> {
    match command {
        Command::Pause => session_input(if keyboard { InputCommand::PauseKeyboard } else { InputCommand::Pause }),
        Command::Start(id) => {
            if !saved_devices(keyboard)?.iter().any(|(saved, _)| saved == &id) {
                return Err("Dispositivo removido do cadastro. Abra a interface para selecionar outro.".into());
            }
            session_input(if keyboard { InputCommand::StartKeyboard(&id) } else { InputCommand::Start(&id) })
        }
    }
}
fn refresh(app: &AppHandle, status_item: &MenuItem<tauri::Wry>, start: &Submenu<tauri::Wry>, pause: &MenuItem<tauri::Wry>, group: &mut Group) -> Result<(), Box<dyn std::error::Error>> {
    let saved = saved_devices(group.keyboard);
    let current = saved.as_ref().cloned().unwrap_or_default();
    if current != group.devices {
        while start.remove_at(0)?.is_some() {}
        for (id, name) in &current {
            start.append(&MenuItem::with_id(app, format!("{}{id}", group.prefix()), name.replace('&', "&&"), true, None::<&str>)?)?;
        }
        group.devices = current;
    }
    let input = session_input(if group.keyboard { InputCommand::KeyboardStatus } else { InputCommand::Status });
    let active = input.as_ref().is_ok_and(|s| s.active);
    start.set_enabled(input.is_ok() && !active && !group.devices.is_empty() && saved.is_ok())?;
    pause.set_enabled(active)?;
    let label = if let Some(error) = &group.action_error { format!("Falha: {error}") }
        else if let Err(error) = &input { format!("Daemon indisponível: {error}") }
        else if active {
            let id = &input.as_ref().unwrap().device_id;
            group.devices.iter().find(|(saved, _)| saved == id).map(|(_, name)| format!("Ativo · {name}")).unwrap_or_else(|| "Ativo".into())
        } else if let Err(error) = saved { format!("Cadastro indisponível: {error}") }
        else { "Pausado".into() };
    status_item.set_text(format!("{}: {}", group.title(), label.chars().take(180).collect::<String>()).replace('&', "&&"))?;
    if let Ok(input) = input {
        if group.previous.as_ref() != Some(&input) {
            app.emit(if group.keyboard { "bindforge-keyboard-status" } else { "bindforge-input-status" }, crate::InputStatus {
                active: input.active, device_id: input.device_id.clone(), message: input.message.clone(),
            })?;
        }
        group.previous = Some(input);
    } else { group.previous = None; }
    Ok(())
}
