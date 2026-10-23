//! Read-only power_supply metadata, matched to the input interface's physical parent.
use std::{fs, path::{Path, PathBuf}};
use remap_core::{Device, DeviceConnection};

struct Supply { owner: PathBuf, percentage: i32, state: String }
fn supplies(root: &Path) -> Vec<Supply> {
    let Ok(entries) = fs::read_dir(root) else { return vec![]; };
    entries.flatten().filter_map(|entry| {
        let path = fs::canonicalize(entry.path()).ok()?;
        let read = |name| fs::read_to_string(path.join(name)).ok().map(|v| v.trim().to_owned());
        if read("scope")?.as_str() != "Device" || read("type")?.as_str() != "Battery"
            || read("present").as_deref() == Some("0") || read("online").as_deref() == Some("0") { return None; }
        let percentage = read("capacity")?.parse::<i32>().ok()?;
        if !(0..=100).contains(&percentage) { return None; }
        let state = match read("status").as_deref() {
            Some("Charging") => "charging", Some("Discharging") => "discharging",
            Some("Full") => "full", _ => "unknown",
        }.to_owned();
        // /physical/device/power_supply/battery -> /physical/device
        let owner = path.parent()?.parent()?.to_path_buf();
        Some(Supply { owner, percentage, state })
    }).collect()
}
fn match_supply<'a>(id: &str, supplies: &'a [Supply]) -> Option<&'a Supply> {
    let path = Path::new(id.strip_prefix("linux:")?);
    let mut matches = supplies.iter().filter(|s| path.starts_with(&s.owner));
    let found = matches.next()?;
    // Ambiguous associations are unavailable rather than reporting another battery.
    if matches.next().is_some() { return None; }
    Some(found)
}
pub fn device_batteries(devices: &[Device]) -> Vec<(String, i32, String)> {
    let supplies = supplies(Path::new("/sys/class/power_supply"));
    devices.iter().filter(|d| d.connection != DeviceConnection::Virtual).filter_map(|d| {
        let s = match_supply(&d.id, &supplies)?;
        Some((d.id.clone(), s.percentage, s.state.clone()))
    }).collect()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn battery_belongs_to_exact_physical_subtree_and_ambiguous_matches_are_rejected() {
        let source = Supply { owner: "/sys/devices/usb/hid1".into(), percentage: 28, state: "discharging".into() };
        let mut supplies = vec![source];
        assert_eq!(match_supply("linux:/sys/devices/usb/hid1/input/event14", &supplies).unwrap().percentage, 28);
        assert!(match_supply("linux:/sys/devices/usb/hid2/input/event3", &supplies).is_none());
        assert!(match_supply("linux:/sys/devices/usb/hid10/input/event3", &supplies).is_none());
        supplies.push(Supply { owner: "/sys/devices/usb/hid1".into(), percentage: 90, state: "charging".into() });
        assert!(match_supply("linux:/sys/devices/usb/hid1/input/event14", &supplies).is_none());
    }
    #[test]
    fn malformed_and_offline_power_data_are_not_reported() {
        let root = std::env::temp_dir().join(format!("remap-battery-test-{}", std::process::id()));
        let path = root.join("physical/power_supply/battery");
        fs::create_dir_all(&path).unwrap();
        for (key, value) in [("scope", "Device"), ("type", "Battery"), ("capacity", "0"), ("status", "Charging")] { fs::write(path.join(key), value).unwrap(); }
        let class = root.join("class"); fs::create_dir_all(&class).unwrap();
        std::os::unix::fs::symlink(&path, class.join("battery")).unwrap();
        let valid = supplies(&class); assert_eq!(valid[0].percentage, 0); assert_eq!(valid[0].state, "charging");
        for invalid in ["101", "-1", "bad"] { fs::write(path.join("capacity"), invalid).unwrap(); assert!(supplies(&class).is_empty()); }
        fs::write(path.join("capacity"), "28").unwrap();
        fs::write(path.join("online"), "0").unwrap(); assert!(supplies(&class).is_empty());
        fs::remove_dir_all(root).unwrap();
    }
}
