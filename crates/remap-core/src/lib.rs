//! Platform-neutral policy. No device access, IPC, or OS dependencies.
use std::collections::HashSet;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeviceKind { Mouse, Keyboard, Composite }

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeviceConnection { Usb, Bluetooth, Internal, Virtual, Unknown }

/// Adapter-provided identifier. Never assume an OS device path is portable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Device {
    pub id: String,
    pub name: String,
    pub kind: DeviceKind,
    pub connection: DeviceConnection,
    pub vendor_id: Option<u16>,
    pub product_id: Option<u16>,
}

/// Enumeration reads device metadata, never the input event stream.
pub trait DeviceEnumerator {
    fn enumerate(&self) -> Result<Vec<Device>, String>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum InputControl { Key(String), MouseButton(u16) }
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Action { KeyChord(Vec<String>), MouseButton(u16), Disabled }
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Binding {
    /// None means all devices supported by the backend.
    pub device_id: Option<String>,
    pub input: InputControl,
    pub action: Action,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ActiveWindow {
    pub app_id: Option<String>,
    pub executable: Option<String>,
    pub title: Option<String>,
}

/// Populated predicates are ANDed. Profile rules are ORed.
/// Identifiers are case-sensitive exact matches; title is a case-sensitive substring.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct AppRule {
    pub app_id: Option<String>,
    pub executable: Option<String>,
    pub title_contains: Option<String>,
}
impl AppRule {
    fn specificity(&self) -> usize {
        usize::from(self.app_id.is_some()) + usize::from(self.executable.is_some())
            + usize::from(self.title_contains.is_some())
    }
    fn valid(&self) -> bool {
        self.specificity() > 0 && [&self.app_id, &self.executable, &self.title_contains]
            .iter().all(|v| v.as_ref().is_none_or(|s| !s.trim().is_empty()))
    }
    pub fn matches(&self, window: &ActiveWindow) -> bool {
        self.valid()
            && self.app_id.as_ref().is_none_or(|v| window.app_id.as_ref() == Some(v))
            && self.executable.as_ref().is_none_or(|v| window.executable.as_ref() == Some(v))
            && self.title_contains.as_ref().is_none_or(|v| window.title.as_ref().is_some_and(|t| t.contains(v)))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Profile {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub priority: i32,
    pub rules: Vec<AppRule>,
    pub bindings: Vec<Binding>,
}

impl Profile {
    /// A device-specific binding overrides the general binding for that control.
    pub fn binding_for(&self, device_id: &str, input: &InputControl) -> Option<&Binding> {
        self.bindings.iter().find(|b| b.device_id.as_deref() == Some(device_id) && &b.input == input)
            .or_else(|| self.bindings.iter().find(|b| b.device_id.is_none() && &b.input == input))
    }
}

/// Validated policy; private fields prevent bypassing validation after construction.
#[derive(Debug, Clone)]
pub struct ProfileSet { profiles: Vec<Profile>, fallback_id: String }
impl ProfileSet {
    pub fn new(profiles: Vec<Profile>, fallback_id: &str) -> Result<Self, String> {
        let mut ids = HashSet::new();
        for p in &profiles {
            if p.id.trim().is_empty() || !ids.insert(p.id.clone()) {
                return Err("profile IDs must be nonempty and unique".into());
            }
            if p.rules.iter().any(|r| !r.valid()) {
                return Err(format!("profile {} contains an empty predicate", p.id));
            }
        }
        let fallback = profiles.iter().find(|p| p.id == fallback_id)
            .ok_or("fallback profile does not exist")?;
        if !fallback.enabled || !fallback.rules.is_empty() {
            return Err("fallback must be enabled and have no rules".into());
        }
        Ok(Self { profiles, fallback_id: fallback_id.into() })
    }
    pub fn profiles(&self) -> &[Profile] { &self.profiles }
    /// Highest priority, then most matching predicates, then lexicographically smallest ID.
    /// Only matching rules contribute specificity. No focus / no match selects fallback.
    pub fn select(&self, window: Option<&ActiveWindow>) -> &Profile {
        let mut candidates: Vec<(&Profile, usize)> = self.profiles.iter()
            .filter(|p| p.enabled && p.id != self.fallback_id)
            .filter_map(|p| {
                let w = window?;
                p.rules.iter().filter(|r| r.matches(w)).map(AppRule::specificity)
                    .max().map(|s| (p, s))
            }).collect();
        candidates.sort_by(|(a, sa), (b, sb)| b.priority.cmp(&a.priority)
            .then(sb.cmp(sa)).then(a.id.cmp(&b.id)));
        candidates.first().map(|(p, _)| *p).unwrap_or_else(|| {
            self.profiles.iter().find(|p| p.id == self.fallback_id).expect("validated fallback")
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WindowEvent { Focused(ActiveWindow), NoActiveWindow }

pub trait WindowMonitor {
    /// Ok(None) = orderly end of stream; NoActiveWindow is an actual focus update.
    /// On lost connection return an error, never silently reuse stale focus.
    fn next_event(&mut self) -> Result<Option<WindowEvent>, String>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RemapMode { Simulation, Native }

pub trait InputRemapper {
    fn mode(&self) -> RemapMode;
    /// Replace the complete mapping atomically, including removing obsolete bindings.
    /// Invalid plans must leave the previous mapping intact. Unrecoverable device I/O
    /// failure may stop the backend, releasing all synthetic keys and physical grabs.
    /// Reset must release held synthetic keys before returning control to the source.
    fn apply_profile(&mut self, profile: &Profile) -> Result<(), String>;
    fn reset(&mut self) -> Result<(), String>;
}

#[derive(Debug, Default)]
pub struct DryRunRemapper { pub last_profile: Option<String> }
impl InputRemapper for DryRunRemapper {
    fn mode(&self) -> RemapMode { RemapMode::Simulation }
    fn apply_profile(&mut self, profile: &Profile) -> Result<(), String> {
        self.last_profile = Some(profile.id.clone()); Ok(())
    }
    fn reset(&mut self) -> Result<(), String> { self.last_profile = None; Ok(()) }
}

pub struct ProfileSwitcher<R: InputRemapper> {
    profiles: ProfileSet,
    remapper: R,
    active_id: Option<String>,
}
impl<R: InputRemapper> ProfileSwitcher<R> {
    pub fn new(profiles: ProfileSet, remapper: R) -> Self {
        Self { profiles, remapper, active_id: None }
    }
    pub fn active_id(&self) -> Option<&str> { self.active_id.as_deref() }
    pub fn handle(&mut self, event: &WindowEvent) -> Result<bool, String> {
        let window = match event { WindowEvent::Focused(w) => Some(w), WindowEvent::NoActiveWindow => None };
        let selected = self.profiles.select(window);
        if self.active_id.as_deref() == Some(selected.id.as_str()) { return Ok(false); }
        self.remapper.apply_profile(selected)?;
        self.active_id = Some(selected.id.clone());
        Ok(true)
    }
    /// Reapply even when the ID stays the same: bindings may have changed.
    /// Commit the new policy only after the backend accepts its selected profile.
    pub fn replace_profiles(&mut self, profiles: ProfileSet, event: &WindowEvent) -> Result<(), String> {
        let window = match event { WindowEvent::Focused(w) => Some(w), WindowEvent::NoActiveWindow => None };
        let selected = profiles.select(window);
        self.remapper.apply_profile(selected)?;
        self.active_id = Some(selected.id.clone());
        self.profiles = profiles;
        Ok(())
    }
    pub fn reset(&mut self) -> Result<(), String> {
        self.remapper.reset()?; self.active_id = None; Ok(())
    }
}

/// Shared fixture for the daemon and UI. This is not persisted user configuration.
pub fn demo_profiles() -> ProfileSet {
    ProfileSet::new(vec![
        Profile { id: "default".into(), name: "Padrão".into(), enabled: true,
            priority: 0, rules: vec![], bindings: vec![] },
        Profile { id: "firefox".into(), name: "Firefox".into(), enabled: true,
            priority: 10, rules: vec![AppRule { app_id: Some("firefox".into()), ..Default::default() }],
            bindings: vec![Binding { device_id: None, input: InputControl::MouseButton(8),
                action: Action::KeyChord(vec!["Alt".into(), "Left".into()]) }] },
        Profile { id: "konsole".into(), name: "Konsole".into(), enabled: true,
            priority: 20, rules: vec![AppRule { app_id: Some("org.kde.konsole".into()), ..Default::default() }],
            bindings: vec![] },
    ], "default").expect("valid demo profiles")
}
