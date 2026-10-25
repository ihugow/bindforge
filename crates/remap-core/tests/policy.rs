use remap_core::*;

fn rule(app: &str) -> AppRule {
    AppRule { app_id: Some(app.into()), ..Default::default() }
}
fn profile(id: &str, priority: i32, rules: Vec<AppRule>) -> Profile {
    Profile { icon: None, id: id.into(), name: id.into(), enabled: true, priority, rules, bindings: vec![] }
}
fn fallback() -> Profile { profile("default", 999, vec![]) }
fn window() -> ActiveWindow {
    ActiveWindow { app_id: Some("firefox".into()), executable: Some("/usr/bin/firefox".into()), title: Some("Docs — Browser".into()) }
}
fn set(mut profiles: Vec<Profile>) -> ProfileSet {
    profiles.push(fallback()); ProfileSet::new(profiles, "default").unwrap()
}
#[test]
fn exact_matching_and_missing_metadata() {
    assert!(rule("firefox").matches(&window()));
    assert!(!rule("Firefox").matches(&window()));
    assert!(!rule("fire").matches(&window()));
    assert!(!rule("firefox").matches(&ActiveWindow::default()));
    assert!(!AppRule::default().matches(&window()));
}
#[test]
fn predicates_are_and_rules_are_or() {
    let combined = AppRule { executable: Some("/usr/bin/firefox".into()), title_contains: Some("Docs".into()), ..rule("firefox") };
    assert!(combined.matches(&window()));
    let mut other = window(); other.title = None;
    assert!(!combined.matches(&other));
    let profiles = set(vec![profile("browser", 1, vec![rule("other"), combined])]);
    assert_eq!(profiles.select(Some(&window())).id, "browser");
}
#[test]
fn priority_beats_specificity_and_fallback_priority_is_ignored() {
    let specific = AppRule { title_contains: Some("Docs".into()), ..rule("firefox") };
    let profiles = set(vec![profile("specific", 1, vec![specific]), profile("priority", 2, vec![rule("firefox")])]);
    assert_eq!(profiles.select(Some(&window())).id, "priority");
}
#[test]
fn specificity_then_stable_id_break_ties_independent_of_order() {
    let specific = AppRule { title_contains: Some("Docs".into()), ..rule("firefox") };
    for reverse in [false, true] {
        let mut profiles = vec![profile("a", 1, vec![rule("firefox")]), profile("z", 1, vec![specific.clone()]), profile("b", 1, vec![specific.clone()])];
        if reverse { profiles.reverse(); }
        assert_eq!(set(profiles).select(Some(&window())).id, "b");
    }
}
#[test]
fn nonmatching_rule_does_not_inflate_specificity() {
    let profiles = set(vec![
        profile("z", 1, vec![rule("firefox"), AppRule { title_contains: Some("Docs".into()), ..rule("other") }]),
        profile("a", 1, vec![rule("firefox")]),
    ]);
    assert_eq!(profiles.select(Some(&window())).id, "a");
}
#[test]
fn disabled_unknown_and_no_focus_use_fallback() {
    let mut disabled = profile("disabled", 100, vec![rule("firefox")]); disabled.enabled = false;
    let profiles = set(vec![disabled]);
    assert_eq!(profiles.select(Some(&window())).id, "default");
    assert_eq!(demo_profiles().select(Some(&ActiveWindow::default())).id, "default");
    assert_eq!(demo_profiles().select(None).id, "default");
}
#[test]
fn reject_invalid_configurations() {
    assert!(ProfileSet::new(vec![], "default").is_err());
    assert!(ProfileSet::new(vec![fallback(), fallback()], "default").is_err());
    let mut disabled = fallback(); disabled.enabled = false;
    assert!(ProfileSet::new(vec![disabled], "default").is_err());
    assert!(ProfileSet::new(vec![profile("default", 0, vec![rule("x")])], "default").is_err());
    assert!(ProfileSet::new(vec![fallback(), profile("bad", 0, vec![AppRule::default()])], "default").is_err());
    assert!(ProfileSet::new(vec![fallback(), profile("", 0, vec![])], "default").is_err());
    assert!(ProfileSet::new(vec![fallback(), profile("bad", 0, vec![rule(" ")])], "default").is_err());
}

#[derive(Default)]
struct Probe { fail: bool, applied: Vec<String> }
impl InputRemapper for Probe {
    fn mode(&self) -> RemapMode { RemapMode::Simulation }
    fn apply_profile(&mut self, profile: &Profile) -> Result<(), String> {
        if self.fail { self.fail = false; return Err("simulated failure".into()); }
        self.applied.push(profile.id.clone()); Ok(())
    }
    fn reset(&mut self) -> Result<(), String> { Ok(()) }
}
#[test]
fn switching_deduplicates_and_returns_to_fallback() {
    let mut engine = ProfileSwitcher::new(demo_profiles(), Probe::default());
    let event = WindowEvent::Focused(window());
    assert!(engine.handle(&event).unwrap());
    assert_eq!(engine.active_id(), Some("firefox"));
    assert!(!engine.handle(&event).unwrap());
    assert!(engine.handle(&WindowEvent::NoActiveWindow).unwrap());
    assert_eq!(engine.active_id(), Some("default"));
    engine.reset().unwrap();
    assert_eq!(engine.active_id(), None);
    assert!(engine.handle(&WindowEvent::NoActiveWindow).unwrap());
}
#[test]
fn failed_application_is_not_committed_and_is_retried() {
    let mut engine = ProfileSwitcher::new(demo_profiles(), Probe { fail: true, ..Default::default() });
    let event = WindowEvent::Focused(window());
    assert!(engine.handle(&event).is_err());
    assert_eq!(engine.active_id(), None);
    assert!(engine.handle(&event).unwrap());
    assert_eq!(engine.active_id(), Some("firefox"));
}

#[test]
fn replacement_reapplies_same_id_and_commits_only_after_success() {
    use std::cell::RefCell;
    use std::rc::Rc;
    #[derive(Default)]
    struct State { fail: bool, applications: Vec<Profile> }
    struct Recorder(Rc<RefCell<State>>);
    impl InputRemapper for Recorder {
        fn mode(&self) -> RemapMode { RemapMode::Simulation }
        fn apply_profile(&mut self, profile: &Profile) -> Result<(), String> {
            let mut s = self.0.borrow_mut();
            if s.fail { return Err("rejected".into()); }
            s.applications.push(profile.clone()); Ok(())
        }
        fn reset(&mut self) -> Result<(), String> { Ok(()) }
    }
    let state = Rc::new(RefCell::new(State::default()));
    let mut engine = ProfileSwitcher::new(demo_profiles(), Recorder(state.clone()));
    let event = WindowEvent::Focused(window()); engine.handle(&event).unwrap();
    let mut profiles = demo_profiles().profiles().to_vec();
    profiles[1].bindings.clear();
    engine.replace_profiles(ProfileSet::new(profiles.clone(), "default").unwrap(), &event).unwrap();
    assert_eq!(engine.active_id(), Some("firefox"));
    assert_eq!(state.borrow().applications.len(), 2);
    assert!(state.borrow().applications[1].bindings.is_empty());
    state.borrow_mut().fail = true;
    profiles[1].enabled = false;
    assert!(engine.replace_profiles(ProfileSet::new(profiles, "default").unwrap(), &event).is_err());
    assert_eq!(engine.active_id(), Some("firefox"));
    state.borrow_mut().fail = false;
    assert!(!engine.handle(&event).unwrap(), "failed replacement must retain the old policy");
}

#[test]
fn steam_identities_select_separate_profiles_for_gamescope_games() {
    let profiles = set(vec![profile("arc", 10, vec![rule("steam:1808500")]), profile("other", 10, vec![rule("steam:570")])]);
    for (app, expected) in [("steam:1808500", "arc"), ("steam:570", "other"), ("gamescope", "default"), ("steam:999", "default")] {
        assert_eq!(profiles.select(Some(&ActiveWindow { app_id: Some(app.into()), ..Default::default() })).id, expected);
    }
}

#[test]
fn device_specific_binding_wins_over_general_and_other_devices_use_general() {
    let mut p = fallback();
    let input = InputControl::MouseButton(8);
    p.bindings = vec![Binding { device_id: None, input: input.clone(), action: Action::KeyChord(vec!["V".into()]) },
        Binding { device_id: Some("mouse-a".into()), input: input.clone(), action: Action::Disabled }];
    assert_eq!(p.binding_for("mouse-a", &input).unwrap().action, Action::Disabled);
    assert_eq!(p.binding_for("mouse-b", &input).unwrap().action, Action::KeyChord(vec!["V".into()]));
    p.bindings.reverse();
    assert_eq!(p.binding_for("mouse-a", &input).unwrap().action, Action::Disabled);
    assert!(p.binding_for("mouse-a", &InputControl::MouseButton(9)).is_none());
}
