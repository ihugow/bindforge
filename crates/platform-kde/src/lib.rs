//! KWin metadata translation and optional session D-Bus focus monitor.
#[cfg(all(feature = "dbus", target_os = "linux"))]
pub mod dbus;
use remap_core::{ActiveWindow, WindowEvent};

/// Proposed D-Bus payload: app ID only. KWin resourceClass is a fallback;
/// executable paths cannot be inferred from it. Titles are deliberately omitted.
#[derive(Debug, Clone)]
pub struct KwinFocus { pub desktop_file_name: String, pub resource_class: String }
impl KwinFocus {
    pub fn into_event(self) -> WindowEvent {
        let app_id = if self.desktop_file_name.is_empty() { self.resource_class }
            else { self.desktop_file_name };
        if app_id.is_empty() { WindowEvent::NoActiveWindow }
        else { WindowEvent::Focused(ActiveWindow { app_id: Some(app_id), ..Default::default() }) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn desktop_id_takes_precedence() {
        let event = KwinFocus { desktop_file_name: "org.kde.konsole".into(), resource_class: "konsole".into() }.into_event();
        assert_eq!(event, WindowEvent::Focused(ActiveWindow { app_id: Some("org.kde.konsole".into()), ..Default::default() }));
    }
    #[test]
    fn xwayland_fallback_and_no_focus() {
        assert_eq!(KwinFocus { desktop_file_name: "".into(), resource_class: "firefox".into() }.into_event(),
            WindowEvent::Focused(ActiveWindow { app_id: Some("firefox".into()), ..Default::default() }));
        assert_eq!(KwinFocus { desktop_file_name: "".into(), resource_class: "".into() }.into_event(), WindowEvent::NoActiveWindow);
    }
}
