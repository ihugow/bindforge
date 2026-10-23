use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager, Window,
};

#[cfg(target_os = "linux")]
mod remapping;

const TRAY_ID: &str = "bindforge-tray";

fn reopen(app: &AppHandle) -> tauri::Result<()> {
    if let Some(window) = app.get_webview_window("main") {
        window.unminimize()?;
        window.show()?;
        window.set_focus()?;
    }
    Ok(())
}

pub fn setup(app: &AppHandle) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, "tray-open", "Abrir BindForge", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "tray-quit", "Sair da interface", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open])?;
    #[cfg(target_os = "linux")]
    let remapping = remapping::Controls::new(app, &menu)?;
    menu.append(&quit)?;
    let icon = app.default_window_icon().ok_or_else(|| {
        std::io::Error::other("Ícone do BindForge não encontrado")
    })?;
    TrayIconBuilder::with_id(TRAY_ID)
        .icon(icon.clone())
        .tooltip("BindForge")
        .menu(&menu)
        .on_menu_event(move |app, event| match event.id.as_ref() {
            "tray-open" => {
                if let Err(error) = reopen(app) {
                    eprintln!("Não foi possível reabrir o BindForge: {error}");
                }
            }
            "tray-quit" => app.exit(0),
            #[cfg(target_os = "linux")]
            id => remapping.dispatch(id),
            #[cfg(not(target_os = "linux"))]
            _ => {}
        })
        // Linux exposes the menu instead of tray click events.
        .on_tray_icon_event(|tray, event| {
            if matches!(event, TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            }) {
                if let Err(error) = reopen(tray.app_handle()) {
                    eprintln!("Não foi possível reabrir o BindForge: {error}");
                }
            }
        })
        .build(app)?;
    Ok(())
}

/// Keep the editor alive; the daemon remains independent of the interface.
pub fn hide_to_tray(window: &Window) -> tauri::Result<()> {
    let tray = window.app_handle().tray_by_id(TRAY_ID).ok_or_else(|| {
        std::io::Error::other("Ícone da bandeja indisponível")
    })?;
    tray.set_visible(true)?;
    window.hide()
}
