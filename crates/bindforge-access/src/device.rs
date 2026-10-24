use std::{collections::BTreeMap, path::{Path, PathBuf}};
use remap_core::{DeviceConnection, DeviceEnumerator, DeviceKind};
use crate::{Kind, Selector, safe_value};

pub struct Target {
    pub name: String,
    pub model: String,
    pub interface_id: String,
    pub syspath: PathBuf,
    pub devnode: PathBuf,
    pub fingerprint: String,
    pub scope: String,
    pub(crate) selector: Selector,
}
impl Target {
    pub fn rule_preview(&self) -> String { self.selector.render() }
}
#[derive(Default)]
struct Metadata { properties: BTreeMap<String, String>, vendor: String, product: String, uniq: String }
fn metadata(node: &udev::Device) -> Metadata {
    let parent = node.parent();
    let attr = |name: &str| parent.as_ref().and_then(|p| p.attribute_value(name)).map(|v| v.to_string_lossy().trim().to_string()).unwrap_or_default();
    Metadata { properties: node.properties().map(|p| (p.name().to_string_lossy().into_owned(), p.value().to_string_lossy().into_owned())).collect(), vendor: attr("id/vendor"), product: attr("id/product"), uniq: attr("uniq") }
}
fn selector(meta: &Metadata, kind: Kind) -> Result<Selector, String> {
    let property = |name| meta.properties.get(name).map(String::as_str).unwrap_or("");
    let mut matches = Vec::new();
    let mut add = |key: &str, value: &str| -> Result<(), String> { matches.push((key.into(), safe_value(value)?)); Ok(()) };
    let bus = property("ID_BUS");
    if bus == "usb" {
        for key in ["ID_BUS", "ID_VENDOR_ID", "ID_MODEL_ID", "ID_USB_INTERFACE_NUM"] {
            let value = property(key);
            if key != "ID_BUS" && (value.len() != if key == "ID_USB_INTERFACE_NUM" { 2 } else { 4 } || !value.bytes().all(|b| b.is_ascii_hexdigit())) {
                return Err("Identificação USB incompleta. Não é possível criar uma autorização específica.".into());
            }
            add(&format!("ENV{{{key}}}"), value)?;
        }
    } else {
        // Input-parent attributes also cover Bluetooth and integrated keyboards.
        for (key, value) in [("ATTRS{id/vendor}", &meta.vendor), ("ATTRS{id/product}", &meta.product)] {
            if value.len() != 4 || !value.bytes().all(|b| b.is_ascii_hexdigit()) { return Err("Identificação de fabricante/produto indisponível.".into()); }
            add(key, value)?;
        }
    }
    let scope;
    if bus == "usb" && !property("ID_SERIAL_SHORT").is_empty() {
        add("ENV{ID_SERIAL_SHORT}", property("ID_SERIAL_SHORT"))?;
        scope = "Unidade identificada pelo número de série e interface";
    } else if !meta.uniq.is_empty() {
        add("ATTRS{uniq}", &meta.uniq)?;
        scope = "Unidade identificada pelo endereço físico e interface";
        // Disambiguate several input interfaces belonging to the same Bluetooth unit.
        if bus != "usb" { add("ATTRS{phys}", property("BINDFORGE_PHYS"))?; }
    } else {
        add("ENV{ID_PATH}", property("ID_PATH"))?;
        scope = "Porta e interface atuais; sem número de série individual";
    }
    Ok(Selector { kind, matches, scope: scope.into() })
}
pub fn inspect(id: &str, kind: Kind) -> Result<Target, String> {
    if id.len() > 4096 || !id.starts_with("linux:/sys/devices/") || id.bytes().any(|b| b.is_ascii_control()) {
        return Err("Identificador de interface inválido.".into());
    }
    let inventory = platform_linux::LinuxDeviceEnumerator.enumerate()?;
    let device = inventory.iter().find(|d| d.id == id).ok_or("Interface desconectada. Atualize a lista e confira o vínculo do dispositivo.")?;
    if matches!(device.connection, DeviceConnection::Virtual | DeviceConnection::Unknown)
        || device.name.starts_with("BindForge") || device.name.starts_with("App Buttons Remap")
        || !matches!((&device.kind, kind), (DeviceKind::Mouse, Kind::Mouse) | (DeviceKind::Keyboard, Kind::Keyboard) | (DeviceKind::Composite, _)) {
        return Err("Selecione uma interface física compatível de mouse ou teclado.".into());
    }
    let path = Path::new(id.strip_prefix("linux:").expect("validated prefix"));
    let node = udev::Device::from_syspath(path).map_err(|e| e.to_string())?;
    if node.syspath() != path || node.subsystem().and_then(|s| s.to_str()) != Some("input") { return Err("A interface mudou durante a consulta.".into()); }
    let mut meta = metadata(&node);
    if let Some(phys) = node.parent().and_then(|p| p.attribute_value("phys").map(|v| v.to_string_lossy().into_owned())) { meta.properties.insert("BINDFORGE_PHYS".into(), phys); }
    let selected = selector(&meta, kind)?;
    // A generated rule must identify one live interface, not all devices of a model.
    let mut matching = 0;
    for item in &inventory {
        if let Some(other) = item.id.strip_prefix("linux:").and_then(|p| udev::Device::from_syspath(Path::new(p)).ok()) {
            if other.property_value(kind.flag()).and_then(|v| v.to_str()) != Some("1") { continue; }
            let mut other_meta = metadata(&other);
            if let Some(phys) = other.parent().and_then(|p| p.attribute_value("phys").map(|v| v.to_string_lossy().into_owned())) { other_meta.properties.insert("BINDFORGE_PHYS".into(), phys); }
            if selector(&other_meta, kind).as_ref().ok() == Some(&selected) { matching += 1; }
        }
    }
    if matching != 1 { return Err("A identificação corresponde a mais de uma interface. A autorização automática foi recusada.".into()); }
    let devnode = node.devnode().ok_or("Nó de entrada indisponível.")?.to_path_buf();
    if !devnode.starts_with("/dev/input") { return Err("Nó de entrada incompatível.".into()); }
    Ok(Target { name: device.name.clone(), model: format!("{:04x}:{:04x}", device.vendor_id.unwrap_or_default(), device.product_id.unwrap_or_default()), interface_id: id.into(), syspath: node.syspath().to_path_buf(), devnode, fingerprint: selected.fingerprint(), scope: selected.scope.clone(), selector: selected })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn usb() -> Metadata { Metadata { properties: [("ID_BUS", "usb"), ("ID_VENDOR_ID", "3554"), ("ID_MODEL_ID", "fa09"), ("ID_USB_INTERFACE_NUM", "00"), ("ID_PATH", "pci-0000:11:00.0-usb-0:4:1.0")].into_iter().map(|(k,v)| (k.into(), v.into())).collect(), ..Default::default() } }
    #[test]
    fn no_serial_is_port_scoped_and_serial_survives_port_changes() {
        let mut a = usb(); let initial = selector(&a, Kind::Keyboard).unwrap();
        a.properties.insert("ID_PATH".into(), "another-port".into());
        assert_ne!(initial.fingerprint(), selector(&a, Kind::Keyboard).unwrap().fingerprint());
        a.properties.insert("ID_SERIAL_SHORT".into(), "serial-123".into());
        let serial = selector(&a, Kind::Keyboard).unwrap();
        a.properties.insert("ID_PATH".into(), "third-port".into());
        assert_eq!(serial, selector(&a, Kind::Keyboard).unwrap());
        assert_ne!(serial.fingerprint(), selector(&a, Kind::Mouse).unwrap().fingerprint());
        a.properties.insert("ID_USB_INTERFACE_NUM".into(), "01".into());
        assert_ne!(serial.fingerprint(), selector(&a, Kind::Keyboard).unwrap().fingerprint());
    }
    #[test]
    fn metadata_cannot_inject_rules_or_glob_patterns() {
        for malicious in ["*", "?", "[abc]", "one|two", "x\", RUN+=\"evil", "line\nbreak", "$env{HOME}", "a\\b", "%k"] {
            let mut meta = usb(); meta.properties.insert("ID_SERIAL_SHORT".into(), malicious.into());
            assert!(selector(&meta, Kind::Keyboard).is_err(), "{malicious}");
        }
        let mut meta = usb(); meta.properties.remove("ID_PATH");
        assert!(selector(&meta, Kind::Keyboard).is_err());
    }
    #[test]
    fn bluetooth_and_integrated_interfaces_keep_hardware_and_interface_scope() {
        let mut bt = Metadata { vendor: "046d".into(), product: "b012".into(), uniq: "aa:bb:cc:dd:ee:ff".into(), ..Default::default() };
        bt.properties.insert("ID_BUS".into(), "bluetooth".into());
        bt.properties.insert("BINDFORGE_PHYS".into(), "11:22:33:44:55:66".into());
        let first = selector(&bt, Kind::Mouse).unwrap();
        assert!(first.render().contains("ATTRS{uniq}"));
        bt.properties.insert("BINDFORGE_PHYS".into(), "different-interface".into());
        assert_ne!(first.fingerprint(), selector(&bt, Kind::Mouse).unwrap().fingerprint());
        let mut internal = Metadata { vendor: "0001".into(), product: "0001".into(), ..Default::default() };
        internal.properties.insert("ID_PATH".into(), "platform-i8042-serio-0".into());
        assert!(selector(&internal, Kind::Keyboard).unwrap().render().contains("ENV{ID_PATH}"));
        internal.properties.clear(); assert!(selector(&internal, Kind::Keyboard).is_err());
    }

}
