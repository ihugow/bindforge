//! Versioned daemon status contract. No device access or control commands.
use std::sync::{Arc, RwLock};

#[cfg(all(feature = "dbus", target_os = "linux"))]
pub mod dbus;

pub const PROTOCOL_VERSION: u32 = 1;
pub const HISTORY_LIMIT: usize = 12;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileChange { pub revision: u64, pub profile_id: String }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusState { Waiting, Focused, NoActiveWindow, Stopped }
impl FocusState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Waiting => "waiting", Self::Focused => "focused",
            Self::NoActiveWindow => "no_active_window", Self::Stopped => "stopped",
        }
    }
}

/// Only the selected profile is exposed: no application IDs, titles, or input data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DaemonStatus {
    pub protocol_version: u32,
    pub simulation: bool,
    pub active_profile_id: Option<String>,
    pub focus_state: FocusState,
    /// Increases after each consumed focus event, even if the profile is unchanged.
    pub revision: u64,
    /// Oldest to newest. Bounded to HISTORY_LIMIT; retained only in process memory.
    pub recent_profiles: Vec<ProfileChange>,
    pub history_available: bool,
}
impl Default for DaemonStatus {
    fn default() -> Self {
        Self { protocol_version: PROTOCOL_VERSION, simulation: true,
            active_profile_id: None, focus_state: FocusState::Waiting, revision: 0, recent_profiles: vec![], history_available: true }
    }
}

/// The daemon owns the writer. IPC clients only receive cloned snapshots.
#[derive(Clone, Default)]
pub struct StatusStore(Arc<RwLock<DaemonStatus>>);
impl StatusStore {
    pub fn snapshot(&self) -> DaemonStatus {
        self.0.read().unwrap_or_else(|e| e.into_inner()).clone()
    }
    pub fn publish(&self, active_profile_id: Option<&str>, focus_state: FocusState) {
        let mut state = self.0.write().unwrap_or_else(|e| e.into_inner());
        let changed = state.active_profile_id.as_deref() != active_profile_id;
        state.active_profile_id = active_profile_id.map(str::to_owned);
        state.focus_state = focus_state;
        state.revision = state.revision.saturating_add(1);
        if changed {
            if let Some(profile_id) = active_profile_id {
                if state.recent_profiles.len() == HISTORY_LIMIT { state.recent_profiles.remove(0); }
                let revision = state.revision;
                state.recent_profiles.push(ProfileChange { revision, profile_id: profile_id.into() });
            }
        }
    }
    pub fn set_simulation(&self, simulation: bool) { self.0.write().unwrap_or_else(|e| e.into_inner()).simulation = simulation; }
    pub fn stopped(&self) { self.publish(None, FocusState::Stopped); }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn publishes_complete_snapshots_and_clears_on_stop() {
        let store = StatusStore::default();
        assert_eq!(store.snapshot(), DaemonStatus::default());
        store.publish(Some("firefox"), FocusState::Focused);
        let previous = store.snapshot();
        store.publish(Some("default"), FocusState::NoActiveWindow);
        assert_eq!(previous.active_profile_id.as_deref(), Some("firefox"));
        assert_eq!(store.snapshot().revision, 2);
        store.stopped();
        let stopped = store.snapshot();
        assert_eq!(stopped.active_profile_id, None);
        assert_eq!(stopped.focus_state, FocusState::Stopped);
        assert!(stopped.simulation);
        assert_eq!(stopped.revision, 3);
    }
    #[test]
    fn retains_short_profile_visits_without_duplicating_unchanged_profiles() {
        let store = StatusStore::default();
        store.publish(Some("default"), FocusState::Waiting);
        store.publish(Some("konsole"), FocusState::Focused);
        store.publish(Some("konsole"), FocusState::Focused);
        store.publish(Some("default"), FocusState::Focused);
        let state = store.snapshot();
        assert_eq!(state.active_profile_id.as_deref(), Some("default"));
        assert_eq!(state.recent_profiles.iter().map(|e| e.profile_id.as_str()).collect::<Vec<_>>(),
            vec!["default", "konsole", "default"]);
        assert_eq!(state.recent_profiles[2].revision, 4);
        store.stopped();
        assert_eq!(store.snapshot().recent_profiles, state.recent_profiles);
    }
    #[test]
    fn history_is_bounded_and_clears_with_a_new_daemon_store() {
        let store = StatusStore::default();
        for i in 0..100 { store.publish(Some(&format!("profile-{i}")), FocusState::Focused); }
        let state = store.snapshot();
        assert_eq!(state.recent_profiles.len(), HISTORY_LIMIT);
        assert_eq!(state.recent_profiles.first().unwrap().profile_id, "profile-88");
        assert_eq!(state.recent_profiles.last().unwrap().profile_id, "profile-99");
        assert!(StatusStore::default().snapshot().recent_profiles.is_empty());
    }

}
