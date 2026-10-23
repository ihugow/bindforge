//! Linux metadata adapters and an explicitly activated native mouse backend.
#[cfg(target_os = "linux")]
mod linux;
#[cfg(all(target_os = "linux", feature = "dbus"))]
mod battery;
#[cfg(all(target_os = "linux", feature = "native-input"))]
pub mod native;
#[cfg(target_os = "linux")]
mod steam;
#[cfg(target_os = "linux")]
pub use steam::{gamescope_steam_identity, gamescope_window_identity};
#[cfg(target_os = "linux")]
pub use linux::LinuxDeviceEnumerator;
#[cfg(all(target_os = "linux", feature = "dbus"))]
pub mod dbus;

use remap_core::{Device, DeviceConnection, DeviceKind};

/// Owned udev metadata, separated from live handles for deterministic classification.
#[derive(Default)]
struct Metadata {
    id: String,
    name: String,
    mouse: bool,
    keyboard: bool,
    touchpad: bool,
    touchscreen: bool,
    bus: String,
    virtual_input: bool,
    vendor: Option<String>,
    product: Option<String>,
}
fn hex_id(value: Option<String>) -> Option<u16> {
    value.and_then(|s| u16::from_str_radix(s.trim(), 16).ok())
}
fn classify(meta: Metadata) -> Option<Device> {
    let mouse = meta.mouse && !meta.touchpad && !meta.touchscreen;
    let kind = match (mouse, meta.keyboard) {
        (true, true) => DeviceKind::Composite,
        (true, false) => DeviceKind::Mouse,
        (false, true) => DeviceKind::Keyboard,
        _ => return None,
    };
    let connection = if meta.virtual_input { DeviceConnection::Virtual } else {
        match meta.bus.as_str() {
            "usb" => DeviceConnection::Usb,
            "bluetooth" => DeviceConnection::Bluetooth,
            "i8042" | "serio" | "platform" => DeviceConnection::Internal,
            _ => DeviceConnection::Unknown,
        }
    };
    // Device names are untrusted metadata. Keep labels short and printable.
    let name: String = meta.name.chars().filter(|c| !c.is_control()).take(256).collect();
    let name = name.trim().to_owned();
    Some(Device { id: meta.id, name: if name.is_empty() { "Dispositivo sem nome".into() } else { name },
        kind, connection, vendor_id: hex_id(meta.vendor), product_id: hex_id(meta.product) })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn distinguishes_mouse_keyboard_and_composite() {
        for (mouse, keyboard, expected) in [(true, false, DeviceKind::Mouse),
            (false, true, DeviceKind::Keyboard), (true, true, DeviceKind::Composite)] {
            let device = classify(Metadata { id: "test".into(), name: "Device".into(),
                mouse, keyboard, bus: "usb".into(), ..Default::default() }).unwrap();
            assert_eq!(device.kind, expected);
            assert_eq!(device.connection, DeviceConnection::Usb);
        }
        assert!(classify(Metadata::default()).is_none());
    }
    #[test]
    fn excludes_touchpads_and_unknown_keys_without_losing_keyboard_interfaces() {
        assert!(classify(Metadata { mouse: true, touchpad: true, ..Default::default() }).is_none());
        assert!(classify(Metadata { mouse: true, touchscreen: true, ..Default::default() }).is_none());
        let device = classify(Metadata { mouse: true, touchpad: true, keyboard: true, ..Default::default() }).unwrap();
        assert_eq!(device.kind, DeviceKind::Keyboard);
    }
    #[test]
    fn handles_unknown_ids_and_sanitizes_labels() {
        let device = classify(Metadata { keyboard: true, name: "  My\nKeyboard\0 ".into(),
            vendor: Some("3554".into()), product: Some("invalid".into()), ..Default::default() }).unwrap();
        assert_eq!(device.name, "MyKeyboard");
        assert_eq!(device.vendor_id, Some(0x3554));
        assert_eq!(device.product_id, None);
        assert_eq!(device.connection, DeviceConnection::Unknown);
    }
    #[test]
    fn does_not_label_bluetooth_uhid_as_virtual_input() {
        let device = classify(Metadata { mouse: true, bus: "bluetooth".into(), ..Default::default() }).unwrap();
        assert_eq!(device.connection, DeviceConnection::Bluetooth);
        let virtual_device = classify(Metadata { keyboard: true, virtual_input: true, ..Default::default() }).unwrap();
        assert_eq!(virtual_device.connection, DeviceConnection::Virtual);
    }
}
