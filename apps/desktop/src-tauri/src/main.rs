#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod system_tray;
#[cfg(target_os = "linux")]
mod kwin_integration;
#[cfg(target_os = "linux")]
mod daemon_service;

#[tauri::command]
async fn get_daemon_service() -> Result<daemon_service_status::Status, String> {
    #[cfg(target_os = "linux")]
    { tauri::async_runtime::spawn_blocking(daemon_service::status).await.map_err(|e| e.to_string())? }
    #[cfg(not(target_os = "linux"))]
    { Err("O gerenciamento do daemon está disponível no Linux com systemd.".into()) }
}
#[tauri::command]
async fn control_daemon_service(action: daemon_service_status::Action, window: tauri::WebviewWindow) -> Result<daemon_service_status::Status, String> {
    if window.label() != "main" { return Err("Unsupported window".into()); }
    #[cfg(target_os = "linux")]
    { tauri::async_runtime::spawn_blocking(move || daemon_service::control(action)).await.map_err(|e| e.to_string())? }
    #[cfg(not(target_os = "linux"))]
    { let _ = action; Err("O gerenciamento do daemon está disponível no Linux com systemd.".into()) }
}
mod daemon_service_status {
    #[cfg(target_os = "linux")]
    pub use super::daemon_service::{ServiceStatus as Status, ServiceAction as Action};
    #[cfg(not(target_os = "linux"))]
    #[derive(serde::Serialize)]
    pub struct Status {}
    #[cfg(not(target_os = "linux"))]
    #[derive(serde::Deserialize)]
    #[serde(rename_all = "snake_case")]
    pub enum Action { Start, Stop, Restart, EnableAutostart, DisableAutostart }
}

#[tauri::command]
async fn get_kwin_integration() -> Result<kwin_integration_status::Status, String> {
    #[cfg(target_os = "linux")]
    { tauri::async_runtime::spawn_blocking(kwin_integration::status).await.map_err(|e| e.to_string())? }
    #[cfg(not(target_os = "linux"))]
    { Err("A integração KWin está disponível apenas no Linux com KDE Plasma 6.".into()) }
}
#[tauri::command]
async fn configure_kwin_integration(window: tauri::WebviewWindow) -> Result<kwin_integration_status::Status, String> {
    if window.label() != "main" { return Err("Unsupported window".into()); }
    #[cfg(target_os = "linux")]
    { tauri::async_runtime::spawn_blocking(kwin_integration::configure).await.map_err(|e| e.to_string())? }
    #[cfg(not(target_os = "linux"))]
    { Err("A integração KWin está disponível apenas no Linux com KDE Plasma 6.".into()) }
}
mod kwin_integration_status {
    #[cfg(target_os = "linux")]
    pub type Status = super::kwin_integration::IntegrationStatus;
    #[cfg(not(target_os = "linux"))]
    #[derive(serde::Serialize)]
    pub struct Status {}
}

#[derive(Clone, serde::Serialize)]
struct InputStatus { active: bool, device_id: String, message: String }
#[cfg(target_os = "linux")]
fn input_result(command: remap_ipc::dbus::InputCommand<'_>) -> Result<InputStatus, String> {
    remap_ipc::dbus::session_input(command).map(|s| InputStatus { active: s.active, device_id: s.device_id, message: s.message })
}
#[tauri::command]
async fn get_input_status() -> Result<InputStatus, String> {
    #[cfg(target_os = "linux")]
    { tauri::async_runtime::spawn_blocking(|| input_result(remap_ipc::dbus::InputCommand::Status)).await.map_err(|e| e.to_string())? }
    #[cfg(not(target_os = "linux"))]
    { Err("Backend de entrada disponível apenas no Linux.".into()) }
}
#[tauri::command]
async fn start_mouse(device_id: String, window: tauri::WebviewWindow) -> Result<InputStatus, String> {
    if window.label() != "main" { return Err("Unsupported window".into()); }
    #[cfg(target_os = "linux")]
    { tauri::async_runtime::spawn_blocking(move || input_result(remap_ipc::dbus::InputCommand::Start(&device_id))).await.map_err(|e| e.to_string())? }
    #[cfg(not(target_os = "linux"))]
    { Err("Backend de entrada disponível apenas no Linux.".into()) }
}
#[tauri::command]
async fn pause_mouse(window: tauri::WebviewWindow) -> Result<InputStatus, String> {
    if window.label() != "main" { return Err("Unsupported window".into()); }
    #[cfg(target_os = "linux")]
    { tauri::async_runtime::spawn_blocking(|| input_result(remap_ipc::dbus::InputCommand::Pause)).await.map_err(|e| e.to_string())? }
    #[cfg(not(target_os = "linux"))]
    { Err("Backend de entrada disponível apenas no Linux.".into()) }
}

use remap_core::ActiveWindow;

#[derive(serde::Serialize)]
struct Selection { id: String, name: String, bindings: usize }

// Preview uses the same saved policy as the daemon, without changing its state.
#[tauri::command]
fn preview_profile(app_id: String) -> Result<Selection, String> {
    if app_id.len() > 4096 || app_id.chars().any(char::is_control) {
        return Err("Invalid app ID".into());
    }
    let profiles = remap_config::load(&remap_config::config_path()?)?.validate()?;
    let window = ActiveWindow { app_id: Some(app_id.clone()), ..Default::default() };
    let profile = profiles.select(if app_id.is_empty() { None } else { Some(&window) });
    Ok(Selection { id: profile.id.clone(), name: profile.name.clone(), bindings: profile.bindings.len() })
}
#[tauri::command]
async fn load_profiles() -> Result<remap_config::Config, String> {
    tauri::async_runtime::spawn_blocking(|| remap_config::load(&remap_config::config_path()?))
        .await.map_err(|e| e.to_string())?
}

#[tauri::command]
async fn save_profiles(config: remap_config::Config, expected_revision: u32) -> Result<remap_config::Config, String> {
    tauri::async_runtime::spawn_blocking(move || remap_config::save(&remap_config::config_path()?, config, expected_revision))
        .await.map_err(|e| e.to_string())?
}

#[derive(serde::Serialize)]
struct ProfileChange { revision: String, profile_id: String }

#[derive(serde::Serialize)]
struct DaemonStatus {
    protocol_version: u32,
    simulation: bool,
    active_profile_id: Option<String>,
    focus_state: String,
    // Preserve all u64 values across JavaScript's number precision boundary.
    revision: String,
    recent_profiles: Vec<ProfileChange>,
    history_available: bool,
}


#[cfg(target_os = "linux")]
impl From<remap_ipc::DaemonStatus> for DaemonStatus {
    fn from(state: remap_ipc::DaemonStatus) -> Self {
        Self {
            protocol_version: state.protocol_version, simulation: state.simulation,
            active_profile_id: state.active_profile_id, focus_state: state.focus_state.as_str().into(),
            revision: state.revision.to_string(),
            recent_profiles: state.recent_profiles.into_iter().map(|change| ProfileChange {
                revision: change.revision.to_string(), profile_id: change.profile_id,
            }).collect(), history_available: state.history_available,
        }
    }
}

#[tauri::command]
async fn get_daemon_status() -> Result<DaemonStatus, String> {
    #[cfg(target_os = "linux")]
    {
        tauri::async_runtime::spawn_blocking(remap_ipc::dbus::read_session_status)
            .await.map_err(|e| e.to_string())?.map(Into::into)
    }
    #[cfg(not(target_os = "linux"))]
    Err("Daemon status is currently supported on Linux only".into())
}

#[derive(serde::Serialize)]
struct DeviceBattery { percentage: i32, state: String }
#[derive(serde::Serialize)]
struct DetectedDevice {
    id: String, name: String, kind: String, connection: String,
    vendor_id: Option<u16>, product_id: Option<u16>,
    battery: Option<DeviceBattery>,
}
#[tauri::command]
async fn list_devices() -> Result<Vec<DetectedDevice>, String> {
    #[cfg(target_os = "linux")]
    {
        let devices = tauri::async_runtime::spawn_blocking(remap_ipc::dbus::read_session_devices)
            .await.map_err(|e| e.to_string())?.map_err(|error| {
                if error.starts_with("Restart the daemon") { "Reinicie o daemon para habilitar a lista de dispositivos desta versão.".into() }
                else if error.starts_with("Daemon unavailable") { "Inicie o daemon com --kde para consultar os dispositivos.".into() }
                else { format!("Não foi possível consultar os dispositivos: {error}") }
            })?;
        let batteries = tauri::async_runtime::spawn_blocking(remap_ipc::dbus::read_session_batteries)
            .await.ok().and_then(Result::ok).unwrap_or_default();
        Ok(devices.into_iter().map(|device| DetectedDevice {
            battery: batteries.iter().find(|(id, _, _)| id == &device.id)
                .map(|(_, percentage, state)| DeviceBattery { percentage: *percentage, state: state.clone() }),
            id: device.id, name: device.name,
            kind: match device.kind { remap_core::DeviceKind::Mouse => "mouse", remap_core::DeviceKind::Keyboard => "keyboard", remap_core::DeviceKind::Composite => "composite" }.into(),
            connection: match device.connection { remap_core::DeviceConnection::Usb => "usb", remap_core::DeviceConnection::Bluetooth => "bluetooth",
                remap_core::DeviceConnection::Internal => "internal", remap_core::DeviceConnection::Virtual => "virtual", remap_core::DeviceConnection::Unknown => "unknown" }.into(),
            vendor_id: device.vendor_id, product_id: device.product_id,
        }).collect())
    }
    #[cfg(not(target_os = "linux"))]
    Err("A identificação de dispositivos está disponível apenas no Linux nesta versão.".into())
}

#[derive(serde::Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum DaemonEvent { Status { status: DaemonStatus }, Unavailable { message: String } }

#[derive(Default)]
struct StatusWatches {
    #[cfg(target_os = "linux")]
    next: std::sync::Mutex<u32>,
    #[cfg(target_os = "linux")]
    active: std::sync::Mutex<std::collections::BTreeMap<u32, remap_ipc::dbus::WatchCancel>>,
}

#[tauri::command]
fn start_daemon_status(
    events: tauri::ipc::Channel<DaemonEvent>,
    window: tauri::WebviewWindow,
    state: tauri::State<'_, StatusWatches>,
) -> Result<u32, String> {
    if window.label() != "main" { return Err("Unsupported window".into()); }
    #[cfg(target_os = "linux")]
    {
        let mut active = state.active.lock().map_err(|e| e.to_string())?;
        // StrictMode/HMR can briefly overlap subscriptions while an earlier invoke
        // completes. Each gets its own ID so a late cleanup cannot cancel a new one.
        if active.len() >= 4 { return Err("Too many status subscriptions".into()); }
        let mut next = state.next.lock().map_err(|e| e.to_string())?;
        *next = next.checked_add(1).ok_or("Subscription IDs exhausted")?;
        let id = *next;
        drop(next);
        let (cancel, receiver) = remap_ipc::dbus::watch_cancel();
        active.insert(id, cancel);
        drop(active);
        tauri::async_runtime::spawn(async move {
            remap_ipc::dbus::watch_session_status(receiver, |event| {
                let event = match event {
                    remap_ipc::dbus::WatchEvent::Status(status) => DaemonEvent::Status { status: status.into() },
                    remap_ipc::dbus::WatchEvent::Unavailable { restart_required } => DaemonEvent::Unavailable {
                        message: if restart_required { "Reinicie o daemon para habilitar as atualizações por evento desta versão." }
                            else { "Daemon indisponível. Aguardando reconexão…" }.into(),
                    },
                };
                events.send(event).is_ok()
            }).await;
        });
        Ok(id)
    }
    #[cfg(not(target_os = "linux"))]
    Err("Daemon status is currently supported on Linux only".into())
}

#[tauri::command]
fn stop_daemon_status(id: u32, window: tauri::WebviewWindow, state: tauri::State<'_, StatusWatches>) -> Result<(), String> {
    if window.label() != "main" { return Err("Unsupported window".into()); }
    #[cfg(target_os = "linux")]
    state.active.lock().map_err(|e| e.to_string())?.remove(&id);
    Ok(())
}

#[derive(serde::Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum WindowCaptureEvent { Captured { app_id: String }, Error { message: String } }

#[derive(Default)]
struct WindowCaptures {
    #[cfg(target_os = "linux")]
    next: std::sync::Mutex<u32>,
    #[cfg(target_os = "linux")]
    active: std::sync::Arc<std::sync::Mutex<std::collections::BTreeMap<u32, std::sync::Arc<std::sync::atomic::AtomicBool>>>>,
}
impl WindowCaptures {
    fn cancel_all(&self) {
        #[cfg(target_os = "linux")]
        if let Ok(mut active) = self.active.lock() {
            for cancel in active.values() { cancel.store(true, std::sync::atomic::Ordering::Relaxed); }
            active.clear();
        }
    }
}

#[tauri::command]
async fn start_window_capture(events: tauri::ipc::Channel<WindowCaptureEvent>, window: tauri::WebviewWindow, state: tauri::State<'_, WindowCaptures>) -> Result<u32, String> {
    if window.label() != "main" { return Err("Unsupported window".into()); }
    #[cfg(target_os = "linux")]
    {
        use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
        let active = state.active.clone();
        let cancel = Arc::new(AtomicBool::new(false));
        let id = {
            let mut slots = active.lock().map_err(|e| e.to_string())?;
            if !slots.is_empty() { return Err("Uma captura já está em andamento. Aguarde o cancelamento e tente novamente.".into()); }
            let mut next = state.next.lock().map_err(|e| e.to_string())?;
            *next = next.checked_add(1).ok_or("Limite de capturas atingido.")?;
            slots.insert(*next, cancel.clone()); *next
        };
        let started = tauri::async_runtime::spawn_blocking(remap_ipc::dbus::WindowCapture::session).await;
        let client = match started {
            Ok(Ok(client)) => client,
            result => {
                if let Ok(mut slots) = active.lock() { slots.remove(&id); }
                return Err(match result { Ok(Err(e)) => e, Err(e) => e.to_string(), _ => unreachable!() });
            }
        };
        // Return only after the daemon has armed the capture. The channel can deliver
        // a quick result before invoke returns; the frontend handles that ordering.
        tauri::async_runtime::spawn_blocking(move || {
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(32);
            let mut completed = None;
            while !cancel.load(Ordering::Relaxed) {
                let result = client.poll();
                let event = match result {
                    Ok(Some(app_id)) => Some(WindowCaptureEvent::Captured { app_id }),
                    Err(message) => Some(WindowCaptureEvent::Error { message }),
                    Ok(None) if std::time::Instant::now() >= deadline => Some(WindowCaptureEvent::Error { message: "Tempo esgotado. Inicie a captura novamente.".into() }),
                    Ok(None) => None,
                };
                if let Some(event) = event {
                    completed = Some(event);
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
            drop(client); // Cancels the caller-owned request even on timeout or reload.
            if let Ok(mut slots) = active.lock() { slots.remove(&id); }
            if !cancel.load(Ordering::Relaxed) {
                if let Some(event) = completed { let _ = events.send(event); }
            }
        });
        Ok(id)
    }
    #[cfg(not(target_os = "linux"))]
    Err("A detecção de janelas está disponível apenas no KDE Linux nesta versão.".into())
}

#[tauri::command]
fn stop_window_capture(id: u32, window: tauri::WebviewWindow, state: tauri::State<'_, WindowCaptures>) -> Result<(), String> {
    if window.label() != "main" { return Err("Unsupported window".into()); }
    #[cfg(target_os = "linux")]
    if let Some(cancel) = state.active.lock().map_err(|e| e.to_string())?.get(&id) {
        cancel.store(true, std::sync::atomic::Ordering::Relaxed);
    }
    Ok(())
}

fn main() {
    let builder = tauri::Builder::default().manage(StatusWatches::default()).manage(WindowCaptures::default())
        .setup(|app| {
            system_tray::setup(app.handle())?;
            Ok(())
        });
    #[cfg(target_os = "linux")]
    let builder = builder.on_page_load(|webview, payload| {
        use tauri::Manager;
        if matches!(payload.event(), tauri::webview::PageLoadEvent::Started) {
            if let Ok(mut active) = webview.state::<StatusWatches>().active.lock() { active.clear(); }
            webview.state::<WindowCaptures>().cancel_all();
        }
    });
    let builder = builder.on_window_event(|window, event| {
        if window.label() == "main" {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                match system_tray::hide_to_tray(window) {
                    Ok(()) => api.prevent_close(),
                    Err(error) => eprintln!("Não foi possível minimizar para a bandeja: {error}"),
                }
            }
        }
        #[cfg(target_os = "linux")]
        {
            use tauri::Manager;
            if matches!(event, tauri::WindowEvent::Destroyed) {
                if let Ok(mut active) = window.state::<StatusWatches>().active.lock() { active.clear(); }
                window.state::<WindowCaptures>().cancel_all();
            }
        }
    });
    builder.invoke_handler(tauri::generate_handler![get_daemon_service, control_daemon_service, get_input_status, start_mouse, pause_mouse, get_kwin_integration, configure_kwin_integration, start_window_capture, stop_window_capture, load_profiles, save_profiles, preview_profile, get_daemon_status, list_devices, start_daemon_status, stop_daemon_status])
        .run(tauri::generate_context!()).expect("failed to start desktop shell");
}
