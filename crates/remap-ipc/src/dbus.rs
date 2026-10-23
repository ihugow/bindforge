//! Linux session transport: read-only snapshots and event-driven updates.
mod watch;
mod devices;
mod capture;
mod integration;
mod input;
pub use input::{InputCommand, InputStatus, INPUT_INTERFACE, session_input};
pub use integration::{BridgeStatus, BridgeStore, IntegrationInterface, INTEGRATION_INTERFACE, read_session_integration, reconnect_session_integration};
pub use capture::WindowCapture;
pub use devices::{read_session_batteries, read_session_devices, read_devices_at, encode_devices, WireDevices, WireDevice, DEVICES_INTERFACE, MAX_DEVICES};
pub use watch::{watch_session_status, watch_status_at, watch_cancel, WatchCancel, WatchEvent, WatchReceiver};
use std::time::Duration;
use zbus::blocking::{connection::Builder, Connection, Proxy};
use crate::{DaemonStatus, FocusState, StatusStore, ProfileChange, HISTORY_LIMIT, PROTOCOL_VERSION};

pub const SERVICE: &str = "org.appbuttons.Remap";
pub const PATH: &str = "/org/appbuttons/Remap";
pub const STATUS_INTERFACE: &str = "org.appbuttons.Remap.Status1";
// Fixed wire layout, no serde dependency in the platform-neutral contract.
type WireStatus = (u32, bool, String, String, u64);
type WireHistory = (u32, bool, String, String, u64, Vec<(u64, String)>);

pub struct StatusInterface(pub StatusStore);
#[zbus::interface(name = "org.appbuttons.Remap.Status1")]
impl StatusInterface {
    #[zbus(out_args("protocol_version", "simulation", "active_profile_id", "focus_state", "revision"))]
    fn get_status(&self) -> WireStatus {
        let state = self.0.snapshot();
        (state.protocol_version, state.simulation, state.active_profile_id.unwrap_or_default(),
            state.focus_state.as_str().into(), state.revision)
    }
    #[zbus(out_args("protocol_version", "simulation", "active_profile_id", "focus_state", "revision", "recent_profiles"))]
    fn get_status_with_history(&self) -> WireHistory {
        // One snapshot keeps history and current selection consistent.
        let state = self.0.snapshot();
        (state.protocol_version, state.simulation, state.active_profile_id.unwrap_or_default(),
            state.focus_state.as_str().into(), state.revision,
            state.recent_profiles.into_iter().map(|e| (e.revision, e.profile_id)).collect())
    }

    fn get_status_watch_version(&self) -> u32 { 1 }

    #[zbus(signal)]
    async fn status_changed(
        emitter: &zbus::object_server::SignalEmitter<'_>,
        protocol_version: u32,
        simulation: bool,
        active_profile_id: String,
        focus_state: String,
        revision: u64,
        recent_profiles: Vec<(u64, String)>,
    ) -> zbus::Result<()>;

}

fn encode_status(state: &DaemonStatus) -> WireHistory {
    (state.protocol_version, state.simulation, state.active_profile_id.clone().unwrap_or_default(),
        state.focus_state.as_str().into(), state.revision,
        state.recent_profiles.iter().map(|e| (e.revision, e.profile_id.clone())).collect())
}

/// Called by the daemon only after the new selected profile has been committed.
pub fn emit_status_changed(connection: &Connection, state: &DaemonStatus) -> Result<(), String> {
    connection.emit_signal(None::<&str>, PATH, STATUS_INTERFACE, "StatusChanged", &encode_status(state))
        .map_err(|e| format!("Cannot publish daemon status: {e}"))
}

fn decode(wire: WireStatus) -> Result<DaemonStatus, String> {
    let (protocol_version, simulation, profile, focus, revision) = wire;
    if protocol_version != PROTOCOL_VERSION {
        return Err("Unsupported daemon protocol".into());
    }
    if profile.len() > 4096 || profile.chars().any(char::is_control) {
        return Err("Invalid daemon profile metadata".into());
    }
    let focus_state = match focus.as_str() {
        "waiting" => FocusState::Waiting,
        "focused" => FocusState::Focused,
        "no_active_window" => FocusState::NoActiveWindow,
        "stopped" => FocusState::Stopped,
        _ => return Err("Unsupported daemon focus state".into()),
    };
    Ok(DaemonStatus { protocol_version, simulation,
        active_profile_id: if profile.is_empty() { None } else { Some(profile) }, focus_state, revision, recent_profiles: vec![], history_available: false })
}
fn decode_history(wire: WireHistory) -> Result<DaemonStatus, String> {
    let (version, simulation, profile, focus, revision, history) = wire;
    let mut state = decode((version, simulation, profile, focus, revision))?;
    if history.len() > HISTORY_LIMIT { return Err("Daemon history exceeds its limit".into()); }
    let mut previous = 0;
    for (change_revision, profile_id) in history {
        if change_revision <= previous || change_revision > revision || profile_id.is_empty()
            || profile_id.len() > 4096 || profile_id.chars().any(char::is_control) {
            return Err("Invalid daemon profile history".into());
        }
        previous = change_revision;
        state.recent_profiles.push(ProfileChange { revision: change_revision, profile_id });
    }
    state.history_available = true;
    Ok(state)
}

fn read(connection: Connection) -> Result<DaemonStatus, String> {
    // Pin the name owner: a restart must not combine data from different daemons.
    let bus = zbus::blocking::fdo::DBusProxy::new(&connection).map_err(|e| e.to_string())?;
    let owner = bus.get_name_owner(SERVICE.try_into().expect("valid service name"))
        .map_err(|_| "Daemon unavailable; start remap-daemon with --kde".to_string())?;
    let proxy = Proxy::new(&connection, owner.as_str(), PATH, STATUS_INTERFACE).map_err(|e| e.to_string())?;
    // Do not auto-start any service when the daemon is absent.
    match proxy.call::<_, _, WireHistory>("GetStatusWithHistory", &()) {
        Ok(wire) => decode_history(wire),
        Err(zbus::Error::MethodError(name, _, _)) if name.as_str() == "org.freedesktop.DBus.Error.UnknownMethod" => {
            // Keep current-state monitoring working with a running older daemon.
            let wire: WireStatus = proxy.call("GetStatus", &()).map_err(|e| format!("Cannot read daemon status: {e}"))?;
            decode(wire)
        }
        Err(e) => Err(format!("Cannot read daemon status: {e}")),
    }
}
pub fn read_session_status() -> Result<DaemonStatus, String> {
    read(Builder::session().map_err(|e| e.to_string())?
        .method_timeout(Duration::from_secs(2)).build().map_err(|e| e.to_string())?)
}
/// Explicit address for isolated tests. Never changes the process session bus.
pub fn read_status_at(address: &str) -> Result<DaemonStatus, String> {
    read(Builder::address(address).map_err(|e| e.to_string())?
        .method_timeout(Duration::from_secs(2)).build().map_err(|e| e.to_string())?)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_incompatible_or_invalid_snapshots() {
        for wire in [
            (2, true, "default".into(), "waiting".into(), 0),
            (1, true, "default".into(), "unknown".into(), 0),
            (1, true, "bad\nprofile".into(), "waiting".into(), 0),
        ] { assert!(decode(wire).is_err()); }
        assert_eq!(decode((1, true, "".into(), "stopped".into(), 3)).unwrap().active_profile_id, None);
        assert!(!decode((1, false, "default".into(), "focused".into(), 1)).unwrap().simulation);
    }
    #[test]
    fn validates_history_order_and_snapshot_revision() {
        let decode = |history| decode_history((1, true, "default".into(), "focused".into(), 4, history));
        assert!(decode(vec![(2, "konsole".into()), (4, "default".into())]).unwrap().history_available);
        assert!(decode(vec![(5, "konsole".into())]).is_err());
        assert!(decode(vec![(2, "konsole".into()), (1, "default".into())]).is_err());
        assert!(decode(vec![(1, "bad\nprofile".into())]).is_err());
        assert!(decode((1..=13).map(|i| (i, "default".into())).collect()).is_err());
    }

}
