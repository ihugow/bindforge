//! Read-only inventory snapshots, independent of profile notifications.
use std::collections::HashSet;
use std::time::Duration;
use remap_core::{Device, DeviceConnection, DeviceKind};
use zbus::blocking::{connection::Builder, Proxy};
use super::{SERVICE, PATH};

pub const DEVICES_INTERFACE: &str = "org.appbuttons.Remap.Devices1";
pub const MAX_DEVICES: usize = 256;
pub type WireDevice = (String, String, String, String, i32, i32);
pub type WireDevices = (u32, Vec<WireDevice>);

pub fn encode_devices(devices: Vec<Device>) -> Result<WireDevices, String> {
    if devices.len() > MAX_DEVICES { return Err("Too many input interfaces for this protocol".into()); }
    let wire = devices.into_iter().map(|d| (d.id, d.name,
        match d.kind { DeviceKind::Mouse => "mouse", DeviceKind::Keyboard => "keyboard", DeviceKind::Composite => "composite" }.into(),
        match d.connection { DeviceConnection::Usb => "usb", DeviceConnection::Bluetooth => "bluetooth",
            DeviceConnection::Internal => "internal", DeviceConnection::Virtual => "virtual", DeviceConnection::Unknown => "unknown" }.into(),
        d.vendor_id.map(i32::from).unwrap_or(-1), d.product_id.map(i32::from).unwrap_or(-1))).collect();
    Ok((1, wire))
}
fn decode_devices((version, wire): WireDevices) -> Result<Vec<Device>, String> {
    if version != 1 || wire.len() > MAX_DEVICES { return Err("Unsupported inventory protocol".into()); }
    let mut ids = HashSet::new();
    wire.into_iter().map(|(id, name, kind, connection, vendor, product)| {
        if id.is_empty() || id.len() > 4096 || !ids.insert(id.clone()) || id.chars().any(char::is_control)
            || name.trim().is_empty() || name.len() > 1024 || name.chars().any(char::is_control) {
            return Err("Invalid device metadata".into());
        }
        let kind = match kind.as_str() { "mouse" => DeviceKind::Mouse, "keyboard" => DeviceKind::Keyboard,
            "composite" => DeviceKind::Composite, _ => return Err("Unknown device type".into()) };
        let connection = match connection.as_str() { "usb" => DeviceConnection::Usb, "bluetooth" => DeviceConnection::Bluetooth,
            "internal" => DeviceConnection::Internal, "virtual" => DeviceConnection::Virtual,
            "unknown" => DeviceConnection::Unknown, _ => return Err("Unknown device connection".into()) };
        let parse_id = |id: i32| if id == -1 { Ok(None) } else {
            u16::try_from(id).map(Some).map_err(|_| "Invalid device vendor/product ID".to_string())
        };
        Ok(Device { id, name, kind, connection, vendor_id: parse_id(vendor)?, product_id: parse_id(product)? })
    }).collect()
}
fn read(connection: zbus::blocking::Connection) -> Result<Vec<Device>, String> {
    let bus = zbus::blocking::fdo::DBusProxy::new(&connection).map_err(|e| e.to_string())?;
    let owner = bus.get_name_owner(SERVICE.try_into().expect("valid name"))
        .map_err(|_| "Daemon unavailable; start remap-daemon with --kde".to_string())?;
    let proxy = Proxy::new(&connection, owner.as_str(), PATH, DEVICES_INTERFACE).map_err(|e| e.to_string())?;
    let wire = proxy.call("GetDevices", &()).map_err(|e| match e {
        zbus::Error::MethodError(name, _, _) if name.as_str() == "org.freedesktop.DBus.Error.UnknownMethod"
            || name.as_str() == "org.freedesktop.DBus.Error.UnknownInterface" => "Restart the daemon to enable device inventory".to_string(),
        other => format!("Cannot read device inventory: {other}"),
    })?;
    decode_devices(wire)
}
pub fn read_session_devices() -> Result<Vec<Device>, String> {
    read(Builder::session().map_err(|e| e.to_string())?.method_timeout(Duration::from_secs(3))
        .build().map_err(|e| e.to_string())?)
}
pub fn read_devices_at(address: &str) -> Result<Vec<Device>, String> {
    read(Builder::address(address).map_err(|e| e.to_string())?.method_timeout(Duration::from_secs(3))
        .build().map_err(|e| e.to_string())?)
}

/// Optional additive snapshot; older daemons may not implement battery metadata.
pub fn read_session_batteries() -> Result<Vec<(String, i32, String)>, String> {
    let connection = Builder::session().map_err(|e| e.to_string())?.method_timeout(Duration::from_secs(3))
        .build().map_err(|e| e.to_string())?;
    let bus = zbus::blocking::fdo::DBusProxy::new(&connection).map_err(|e| e.to_string())?;
    let owner = bus.get_name_owner(SERVICE.try_into().expect("valid name")).map_err(|e| e.to_string())?;
    let proxy = Proxy::new(&connection, owner.as_str(), PATH, DEVICES_INTERFACE).map_err(|e| e.to_string())?;
    let (version, rows): (u32, Vec<(String, i32, String)>) = proxy.call("GetDeviceBatteries", &()).map_err(|e| e.to_string())?;
    validate_batteries(version, rows)
}
fn validate_batteries(version: u32, rows: Vec<(String, i32, String)>) -> Result<Vec<(String, i32, String)>, String> {
    let mut ids = HashSet::new();
    if version != 1 || rows.len() > MAX_DEVICES || rows.iter().any(|(id, percentage, state)| {
        id.is_empty() || id.len() > 4096 || id.chars().any(char::is_control) || !ids.insert(id.clone())
        || !(0..=100).contains(percentage) || !["charging", "discharging", "full", "unknown"].contains(&state.as_str())
    }) { return Err("Invalid battery metadata".into()); }
    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn battery_snapshot_rejects_invalid_values_and_duplicate_devices() {
        let row = ("mouse".into(), 0, "charging".into());
        assert!(validate_batteries(1, vec![row.clone()]).is_ok());
        assert!(validate_batteries(1, vec![row.clone(), row]).is_err());
        assert!(validate_batteries(1, vec![("mouse".into(), 101, "full".into())]).is_err());
        assert!(validate_batteries(2, vec![]).is_err());
    }
    fn row(id: &str) -> WireDevice { (id.into(), "Keyboard".into(), "keyboard".into(), "usb".into(), 0x1234, -1) }
    #[test]
    fn accepts_empty_inventory_and_preserves_distinct_interfaces() {
        assert!(decode_devices((1, vec![])).unwrap().is_empty());
        let devices = decode_devices((1, vec![row("first"), row("second")])).unwrap();
        assert_eq!(devices.len(), 2);
        assert_eq!(devices[0].product_id, None);
        assert_eq!(devices[0].vendor_id, Some(0x1234));
    }
    #[test]
    fn rejects_invalid_ids_duplicates_and_protocols() {
        assert!(decode_devices((2, vec![])).is_err());
        assert!(decode_devices((1, vec![row("same"), row("same")])).is_err());
        let mut invalid = row("one"); invalid.4 = 65536;
        assert!(decode_devices((1, vec![invalid])).is_err());
        let mut invalid = row("one"); invalid.1 = "bad\nname".into();
        assert!(decode_devices((1, vec![invalid])).is_err());
        assert!(decode_devices((1, (0..257).map(|i| row(&i.to_string())).collect())).is_err());
    }
}
