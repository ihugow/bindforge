use std::{sync::mpsc, time::Duration};
use tauri::{menu::{Menu, MenuItem, Submenu}, AppHandle, Emitter};
use remap_ipc::dbus::{session_input, InputCommand, InputStatus};

const START_PREFIX: &str = "tray-start:";

enum Command { Start(String), Pause }

pub(super) struct Controls(mpsc::SyncSender<Command>);

impl Controls {
    pub(super) fn new(app: &AppHandle, menu: &Menu<tauri::Wry>) -> tauri::Result<Self> {
        let status = MenuItem::new(app, "Consultando remapeamento…", false, None::<&str>)?;
        let start = Submenu::new(app, "Iniciar remapeamento", false)?;
        let pause = MenuItem::with_id(app, "tray-pause", "Pausar remapeamento", false, None::<&str>)?;
        menu.append_items(&[&status, &start, &pause])?;
        let (sender, receiver) = mpsc::sync_channel(1);
        let app = app.clone();
        // Input1 has no change signal. Refresh independently of window visibility,
        // serializing tray commands off the UI thread with bounded D-Bus timeouts.
        std::thread::spawn(move || {
            let mut mice = Vec::new();
            let mut previous = None;
            let mut action_error = None;
            loop {
                if let Err(error) = refresh(&app, &status, &start, &pause, &mut mice, &mut previous, action_error.as_deref()) {
                    eprintln!("Não foi possível atualizar o menu de remapeamento: {error}");
                }
                match receiver.recv_timeout(Duration::from_secs(2)) {
                    Ok(command) => {
                        let _ = start.set_enabled(false);
                        let _ = pause.set_enabled(false);
                        let result = execute(command);
                        action_error = result.err();
                    }
                    Err(mpsc::RecvTimeoutError::Timeout) => {}
                    Err(mpsc::RecvTimeoutError::Disconnected) => break,
                }
            }
        });
        Ok(Self(sender))
    }

    pub(super) fn dispatch(&self, id: &str) {
        let command = if id == "tray-pause" { Command::Pause }
            else if let Some(id) = id.strip_prefix(START_PREFIX) { Command::Start(id.into()) }
            else { return };
        // A full queue already contains an explicit user action; never block GTK.
        let _ = self.0.try_send(command);
    }
}

fn saved_mice() -> Result<Vec<(String, String)>, String> {
    let config = remap_config::load(&remap_config::config_path()?)?;
    config.validate()?;
    Ok(config.devices.into_iter().filter(|d| d.kind == "mouse").map(|d| (d.id, d.name)).collect())
}

fn execute(command: Command) -> Result<InputStatus, String> {
    match command {
        Command::Pause => session_input(InputCommand::Pause),
        Command::Start(id) => {
            if !saved_mice()?.iter().any(|(saved, _)| saved == &id) {
                return Err("Mouse removido do cadastro. Abra a interface para selecionar outro.".into());
            }
            session_input(InputCommand::Start(&id))
        }
    }
}

fn refresh(
    app: &AppHandle,
    status_item: &MenuItem<tauri::Wry>,
    start: &Submenu<tauri::Wry>,
    pause: &MenuItem<tauri::Wry>,
    mice: &mut Vec<(String, String)>,
    previous: &mut Option<InputStatus>,
    action_error: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    let saved = saved_mice();
    let current = saved.as_ref().cloned().unwrap_or_default();
    if current != *mice {
        while start.remove_at(0)?.is_some() {}
        for (id, name) in &current {
            start.append(&MenuItem::with_id(app, format!("{START_PREFIX}{id}"), name.replace('&', "&&"), true, None::<&str>)?)?;
        }
        *mice = current;
    }
    let input = session_input(InputCommand::Status);
    let active = input.as_ref().is_ok_and(|s| s.active);
    start.set_enabled(input.is_ok() && !active && !mice.is_empty() && saved.is_ok())?;
    pause.set_enabled(active)?;
    let label = if let Some(error) = action_error { format!("Falha: {error}") }
        else if let Err(error) = &input { format!("Daemon indisponível: {error}") }
        else if active {
            let id = &input.as_ref().unwrap().device_id;
            let name = mice.iter().find(|(saved, _)| saved == id).map(|(_, name)| name.as_str()).unwrap_or("Mouse");
            format!("Remapeamento ativo: {name}")
        } else if let Err(error) = saved { format!("Cadastro indisponível: {error}") }
        else if mice.is_empty() { "Cadastre um mouse na interface".into() }
        else { "Remapeamento pausado".into() };
    status_item.set_text(label.chars().take(200).collect::<String>().replace('&', "&&"))?;
    if let Ok(input) = input {
        if previous.as_ref() != Some(&input) {
            app.emit("bindforge-input-status", crate::InputStatus {
                active: input.active, device_id: input.device_id.clone(), message: input.message.clone(),
            })?;
        }
        *previous = Some(input);
    } else { *previous = None; }
    Ok(())
}
