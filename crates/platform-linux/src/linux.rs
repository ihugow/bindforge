use remap_core::{Device, DeviceEnumerator};
use super::{classify, Metadata};

pub struct LinuxDeviceEnumerator;
impl DeviceEnumerator for LinuxDeviceEnumerator {
    fn enumerate(&self) -> Result<Vec<Device>, String> {
        let mut enumerator = udev::Enumerator::new().map_err(|e| format!("Cannot initialize udev: {e}"))?;
        enumerator.match_subsystem("input").map_err(|e| e.to_string())?;
        enumerator.match_sysname("event*").map_err(|e| e.to_string())?;
        enumerator.match_is_initialized().map_err(|e| e.to_string())?;
        let mut devices = Vec::new();
        for node in enumerator.scan_devices().map_err(|e| format!("Cannot enumerate input metadata: {e}"))? {
            let property = |key| node.property_value(key).map(|v| v.to_string_lossy().into_owned());
            let flag = |key| property(key).as_deref() == Some("1");
            let input = node.parent();
            let name = input.as_ref().and_then(|p| p.attribute_value("name"))
                .map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
            let id_attribute = |name| input.as_ref().and_then(|p|
                std::fs::read_to_string(p.syspath().join("id").join(name)).ok());
            let path = node.syspath().to_string_lossy();
            let metadata = Metadata {
                // A connection-local interface ID, not a persistent hardware/serial ID.
                id: format!("linux:{path}"), name,
                mouse: flag("ID_INPUT_MOUSE"), keyboard: flag("ID_INPUT_KEYBOARD"),
                touchpad: flag("ID_INPUT_TOUCHPAD"), touchscreen: flag("ID_INPUT_TOUCHSCREEN"),
                bus: property("ID_BUS").unwrap_or_default(),
                virtual_input: path.starts_with("/sys/devices/virtual/input/"),
                vendor: property("ID_VENDOR_ID").or_else(|| id_attribute("vendor")),
                product: property("ID_MODEL_ID").or_else(|| id_attribute("product")),
            };
            if let Some(device) = classify(metadata) { devices.push(device); }
        }
        devices.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()).then(a.id.cmp(&b.id)));
        Ok(devices)
    }
}
