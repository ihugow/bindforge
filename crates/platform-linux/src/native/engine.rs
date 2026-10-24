//! Deterministic event transformation, also used by the isolated virtual test harness.
use std::collections::{BTreeMap, BTreeSet};
use evdev::{InputEvent, EventType, KeyCode};
use remap_core::{Action, InputControl, Profile};
#[derive(Clone, Default, Debug, PartialEq, Eq)]
pub struct Plan(pub BTreeMap<u16, Vec<u16>>);
pub fn mouse_code(id: u16) -> Result<u16, String> {
    Ok(match id { 1 => KeyCode::BTN_LEFT.0, 3 => KeyCode::BTN_RIGHT.0, 2 => KeyCode::BTN_MIDDLE.0,
        8..=12 => KeyCode::BTN_SIDE.0 + id - 8,
        _ => return Err(format!("Botão {id} ainda não tem correspondência física implementada.")), })
}
pub fn key_code(name: &str) -> Result<u16, String> {
    let upper = name.trim().to_ascii_uppercase();
    let normalized = match upper.as_str() {
        "CTRL" | "CONTROL" => "KEY_LEFTCTRL", "SHIFT" => "KEY_LEFTSHIFT", "ALT" => "KEY_LEFTALT", "ALTGR" => "KEY_RIGHTALT",
        "SUPER" | "META" | "WIN" => "KEY_LEFTMETA", "ESC" | "ESCAPE" => "KEY_ESC", "RETURN" => "KEY_ENTER",
        "PAGEUP" => "KEY_PAGEUP", "PAGEDOWN" => "KEY_PAGEDOWN", "CAPSLOCK" => "KEY_CAPSLOCK",
        _ => "",
    };
    let token = if !normalized.is_empty() { normalized.into() } else if upper.starts_with("KEY_") { upper } else { format!("KEY_{upper}") };
    let key = token.parse::<KeyCode>().map_err(|_| format!("Tecla “{name}” não reconhecida. Use V, Ctrl, Shift, Alt, Left ou KEY_* do Linux."))?;
    if key.0 == 0 || key.0 >= KeyCode::BTN_0.0 { return Err("Use uma tecla de teclado no atalho.".into()); }
    Ok(key.0)
}
impl Plan {
    pub fn compile(profile: &Profile, device_id: &str) -> Result<Self, String> {
        let mut codes = BTreeSet::new();
        for binding in &profile.bindings {
            if binding.device_id.as_deref().is_some_and(|id| id != device_id) { continue; }
            if let InputControl::MouseButton(code) = binding.input { codes.insert(code); }
        }
        let mut result = BTreeMap::new();
        for button in codes {
            let binding = profile.binding_for(device_id, &InputControl::MouseButton(button)).expect("collected binding");
            let action = compile_action(&binding.action)?;
            result.insert(mouse_code(button)?, action);
        }
        Ok(Self(result))
    }
    pub fn compile_keyboard(profile: &Profile, device_id: &str) -> Result<Self, String> {
        let mut bindings = BTreeMap::new();
        for binding in &profile.bindings {
            if binding.device_id.as_deref().is_some_and(|id| id != device_id) { continue; }
            let InputControl::Key(name) = &binding.input else { continue; };
            let code = key_code(name)?;
            let specific = binding.device_id.is_some();
            if bindings.insert((code, specific), binding).is_some() {
                return Err(format!("Mais de um mapeamento para a mesma tecla física: {name}."));
            }
        }
        let mut result = BTreeMap::new();
        for ((code, specific), binding) in &bindings {
            if !specific && bindings.contains_key(&(*code, true)) { continue; }
            result.insert(*code, compile_action(&binding.action)?);
        }
        Ok(Self(result))
    }
    fn output(&self, code: u16) -> Vec<u16> { self.0.get(&code).cloned().unwrap_or_else(|| vec![code]) }
}
fn compile_action(action: &Action) -> Result<Vec<u16>, String> {
    match action {
        Action::Disabled => Ok(vec![]), Action::MouseButton(b) => Ok(vec![mouse_code(*b)?]),
        Action::KeyChord(keys) => {
            if keys.is_empty() || keys.len() > 16 { return Err("O atalho deve conter de 1 a 16 teclas.".into()); }
            let keys: Vec<_> = keys.iter().map(|k| key_code(k)).collect::<Result<_, _>>()?;
            if keys.iter().copied().collect::<BTreeSet<_>>().len() != keys.len() { return Err("O atalho contém teclas físicas repetidas.".into()); }
            Ok(keys)
        }
    }
}
fn modifier(code: u16) -> bool {
    [KeyCode::KEY_LEFTCTRL, KeyCode::KEY_RIGHTCTRL, KeyCode::KEY_LEFTSHIFT, KeyCode::KEY_RIGHTSHIFT,
        KeyCode::KEY_LEFTALT, KeyCode::KEY_RIGHTALT, KeyCode::KEY_LEFTMETA, KeyCode::KEY_RIGHTMETA].iter().any(|key| key.0 == code)
}
/// Physical Ctrl+Alt+Pause remains available even if these controls are remapped.
#[derive(Default)]
pub(super) struct EmergencyChord(BTreeSet<u16>);
impl EmergencyChord {
    pub fn event(&mut self, event: InputEvent) -> bool {
        if event.event_type() != EventType::KEY { return false; }
        let fresh_press = match event.value() {
            1 => self.0.insert(event.code()),
            0 => { self.0.remove(&event.code()); false },
            _ => false,
        };
        event.code() == KeyCode::KEY_PAUSE.0 && fresh_press
            && [KeyCode::KEY_LEFTCTRL, KeyCode::KEY_RIGHTCTRL].iter().any(|key| self.0.contains(&key.0))
            && [KeyCode::KEY_LEFTALT, KeyCode::KEY_RIGHTALT].iter().any(|key| self.0.contains(&key.0))
    }
}
#[derive(Default)]
pub struct Engine {
    plan: Plan, held: BTreeMap<u16, Vec<u16>>, blocked: BTreeSet<u16>, refs: BTreeMap<u16, usize>,
}
fn key(code: u16, value: i32) -> InputEvent { InputEvent::new(EventType::KEY.0, code, value) }
impl Engine {
    pub fn new(plan: Plan) -> Self { Self { plan, ..Default::default() } }
    fn press(&mut self, codes: &[u16], out: &mut Vec<InputEvent>) {
        for &code in codes { let n = self.refs.entry(code).or_default(); if *n == 0 { out.push(key(code, 1)); } *n += 1; }
    }
    fn release(&mut self, codes: &[u16], out: &mut Vec<InputEvent>) {
        for &code in codes.iter().rev() {
            if let Some(n) = self.refs.get_mut(&code) { *n -= 1; if *n == 0 { self.refs.remove(&code); out.push(key(code, 0)); } }
        }
    }
    pub fn event(&mut self, event: InputEvent) -> Vec<InputEvent> {
        if event.event_type() != EventType::KEY { return vec![event]; }
        let code = event.code(); let mut out = vec![];
        match event.value() {
            1 if !self.blocked.contains(&code) && !self.held.contains_key(&code) => {
                let output = self.plan.output(code); self.press(&output, &mut out); self.held.insert(code, output);
            }
            0 => { self.blocked.remove(&code); if let Some(output) = self.held.remove(&code) { self.release(&output, &mut out); } }
            2 if self.held.contains_key(&code) => {
                if !self.plan.0.contains_key(&code) { out.push(event); }
                else if code < KeyCode::BTN_0.0 {
                    if let Some(&target) = self.held[&code].last() {
                        if target < KeyCode::BTN_0.0 && !modifier(target) { out.push(key(target, 2)); }
                    }
                }
            },
            _ => {}
        }
        out
    }
    pub fn replace(&mut self, plan: Plan) -> Vec<InputEvent> {
        let changed: Vec<_> = self.held.iter().filter(|(code, output)| plan.output(**code) != **output).map(|(code, _)| *code).collect();
        let mut out = vec![];
        for code in changed { let output = self.held.remove(&code).unwrap(); self.release(&output, &mut out); self.blocked.insert(code); }
        self.plan = plan; out
    }
    pub fn release_all(&mut self) -> Vec<InputEvent> {
        let mut out = vec![];
        for (code, output) in std::mem::take(&mut self.held) { self.release(&output, &mut out); self.blocked.insert(code); }
        out
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use remap_core::{Binding, demo_profiles};
    fn profile(actions: &[(u16, Action)]) -> Profile {
        let mut p = demo_profiles().profiles()[0].clone();
        p.bindings = actions.iter().map(|(b, a)| Binding { device_id: None, input: InputControl::MouseButton(*b), action: a.clone() }).collect(); p
    }
    fn plan(actions: &[(u16, Action)]) -> Plan { Plan::compile(&profile(actions), "fixture").unwrap() }
    fn events(v: Vec<InputEvent>) -> Vec<(u16,i32)> { v.into_iter().map(|e| (e.code(),e.value())).collect() }
    #[test]
    fn virtual_mouse_suppresses_original_and_balances_key_press_and_release() {
        let mut e = Engine::new(plan(&[(8, Action::KeyChord(vec!["V".into()]))]));
        assert_eq!(events(e.event(key(275,1))), vec![(47,1)]);
        assert!(e.event(key(275,1)).is_empty()); assert!(e.event(key(275,2)).is_empty());
        assert_eq!(events(e.event(key(275,0))), vec![(47,0)]);
        assert!(e.event(key(275,0)).is_empty());
        let motion = InputEvent::new(EventType::RELATIVE.0,0,27); assert_eq!(e.event(motion), vec![motion]);
    }
    #[test]
    fn shared_modifiers_survive_another_button_release_and_never_loop_through_mapping() {
        let mut e = Engine::new(plan(&[(8, Action::KeyChord(vec!["Ctrl".into(),"V".into()])), (9,Action::KeyChord(vec!["Ctrl".into(),"C".into()]))]));
        assert_eq!(events(e.event(key(275,1))), vec![(29,1),(47,1)]);
        assert_eq!(events(e.event(key(276,1))), vec![(46,1)]);
        assert_eq!(events(e.event(key(275,0))), vec![(47,0)]);
        assert_eq!(events(e.event(key(276,0))), vec![(46,0),(29,0)]);
        let mut e = Engine::new(plan(&[(8, Action::MouseButton(9)),(9,Action::Disabled)]));
        assert_eq!(events(e.event(key(275,1))), vec![(276,1)]);
        assert_eq!(events(e.release_all()), vec![(276,0)]);
    }
    #[test]
    fn changing_profile_releases_changed_keys_but_keeps_unmodified_drag_and_blocks_held_button_until_up() {
        let mut e = Engine::new(plan(&[(8, Action::KeyChord(vec!["V".into()]))]));
        e.event(key(275,1)); e.event(key(272,1));
        assert_eq!(events(e.replace(plan(&[(8,Action::KeyChord(vec!["C".into()]))]))), vec![(47,0)]);
        assert!(e.event(key(275,1)).is_empty()); assert!(e.event(key(275,0)).is_empty());
        assert_eq!(events(e.event(key(272,0))), vec![(272,0)]);
        assert_eq!(events(e.event(key(275,1))), vec![(46,1)]);
        assert_eq!(events(e.release_all()), vec![(46,0)]);
    }
    #[test]
    fn composite_keyboard_repeat_media_and_volume_pass_through_during_mouse_mapping() {
        let mut e = Engine::new(plan(&[(8, Action::KeyChord(vec!["V".into()]))]));
        e.event(key(KeyCode::BTN_SIDE.0, 1));
        for code in [KeyCode::KEY_A, KeyCode::KEY_VOLUMEUP] {
            for value in [1, 2, 0] {
                let input = key(code.0, value);
                assert_eq!(e.event(input), vec![input]);
            }
        }
        let volume = InputEvent::new(EventType::ABSOLUTE.0, evdev::AbsoluteAxisCode::ABS_VOLUME.0, 80);
        assert_eq!(e.event(volume), vec![volume]);
        assert_eq!(events(e.release_all()), vec![(KeyCode::KEY_V.0, 0)]);
    }
    #[test]
    fn specific_binding_overrides_general_and_invalid_actions_fail_before_mutation() {
        let mut p = profile(&[(8,Action::KeyChord(vec!["V".into()]))]);
        p.bindings.push(Binding { device_id:Some("fixture".into()), input:InputControl::MouseButton(8), action:Action::Disabled });
        assert!(Plan::compile(&p,"fixture").unwrap().0[&275].is_empty());
        assert_eq!(Plan::compile(&p,"other").unwrap().0[&275],vec![47]);
        p.bindings[0].action=Action::KeyChord(vec!["notakey".into()]);
        assert!(Plan::compile(&p,"other").is_err());
        assert!(key_code("Ctrl").is_ok()); assert_eq!(key_code("Left").unwrap(),105);
        assert!(Plan::compile(&profile(&[(8,Action::KeyChord(vec!["Ctrl".into(),"KEY_LEFTCTRL".into()]))]),"fixture").is_err());
    }
    fn keyboard_profile(actions: &[(&str, Action)]) -> Profile {
        let mut p = profile(&[]);
        p.bindings = actions.iter().map(|(name, action)| Binding { device_id: None, input: InputControl::Key((*name).into()), action: action.clone() }).collect(); p
    }
    #[test]
    fn keyboard_compilation_normalizes_aliases_and_preserves_scope_and_input_kind() {
        let mut p = keyboard_profile(&[("CapsLock", Action::KeyChord(vec!["Escape".into()])), ("KEY_RO", Action::Disabled), ("KEY_KPCOMMA", Action::KeyChord(vec!["AltGr".into(), "KEY_SEMICOLON".into()]))]);
        p.bindings.push(Binding { device_id: Some("fixture".into()), input: InputControl::Key("KEY_CAPSLOCK".into()), action: Action::KeyChord(vec!["Ctrl".into(), "V".into()]) });
        p.bindings.push(Binding { device_id: Some("other".into()), input: InputControl::Key("invalid".into()), action: Action::Disabled });
        p.bindings.push(Binding { device_id: None, input: InputControl::MouseButton(8), action: Action::Disabled });
        let plan = Plan::compile_keyboard(&p, "fixture").unwrap();
        assert_eq!(plan.0[&58], vec![29, 47]);
        assert!(plan.0[&89].is_empty());
        assert_eq!(plan.0[&121], vec![100, 39]);
        assert!(!plan.0.contains_key(&275));
        p.bindings.push(Binding { device_id: Some("fixture".into()), input: InputControl::Key("capslock".into()), action: Action::Disabled });
        assert!(Plan::compile_keyboard(&p, "fixture").unwrap_err().contains("mesma tecla física"));
        assert!(Plan::compile_keyboard(&keyboard_profile(&[("A", Action::KeyChord(vec!["AltGr".into(), "KEY_RIGHTALT".into()]))]), "fixture").is_err());
    }
    #[test]
    fn keyboard_repeats_target_only_and_does_not_chain_or_repeat_disabled_mouse_or_modifier_targets() {
        let p = keyboard_profile(&[("CapsLock", Action::KeyChord(vec!["Ctrl".into(), "V".into()])), ("V", Action::KeyChord(vec!["C".into()])), ("A", Action::Disabled), ("B", Action::MouseButton(1)), ("C", Action::KeyChord(vec!["Shift".into()]))]);
        let mut engine = Engine::new(Plan::compile_keyboard(&p, "fixture").unwrap());
        assert_eq!(events(engine.event(key(58, 1))), vec![(29, 1), (47, 1)]);
        assert_eq!(events(engine.event(key(58, 2))), vec![(47, 2)]);
        assert_eq!(events(engine.event(key(58, 0))), vec![(47, 0), (29, 0)]);
        for code in [30, 48, 46] {
            engine.event(key(code, 1)); assert!(engine.event(key(code, 2)).is_empty()); engine.event(key(code, 0));
        }
        assert_eq!(events(engine.event(key(32, 1))), vec![(32, 1)]);
        assert_eq!(events(engine.event(key(32, 2))), vec![(32, 2)]);
        assert_eq!(events(engine.release_all()), vec![(32, 0)]);
        assert!(engine.event(key(32, 2)).is_empty());
    }
    #[test]
    fn physical_and_mapped_modifiers_share_ownership_and_profile_changes_block_held_keys() {
        let p = keyboard_profile(&[("CapsLock", Action::KeyChord(vec!["Ctrl".into(), "V".into()]))]);
        let mut engine = Engine::new(Plan::compile_keyboard(&p, "fixture").unwrap());
        assert_eq!(events(engine.event(key(29, 1))), vec![(29, 1)]);
        assert_eq!(events(engine.event(key(58, 1))), vec![(47, 1)]);
        let next = keyboard_profile(&[("CapsLock", Action::KeyChord(vec!["Esc".into()]))]);
        assert_eq!(events(engine.replace(Plan::compile_keyboard(&next, "fixture").unwrap())), vec![(47, 0)]);
        assert!(engine.event(key(58, 2)).is_empty());
        assert!(engine.event(key(58, 0)).is_empty());
        assert_eq!(events(engine.event(key(58, 1))), vec![(1, 1)]);
        assert_eq!(events(engine.release_all()), vec![(29, 0), (1, 0)]);
    }
    #[test]
    fn emergency_chord_uses_physical_state_and_requires_a_fresh_pause_press() {
        let mut chord = EmergencyChord::default();
        assert!(!chord.event(key(KeyCode::KEY_LEFTCTRL.0, 1)));
        assert!(!chord.event(key(KeyCode::KEY_RIGHTALT.0, 1)));
        assert!(!chord.event(key(KeyCode::KEY_PAUSE.0, 2)));
        assert!(chord.event(key(KeyCode::KEY_PAUSE.0, 1)));
        assert!(!chord.event(key(KeyCode::KEY_PAUSE.0, 1)));
        assert!(!chord.event(key(KeyCode::KEY_LEFTCTRL.0, 0)));
        assert!(!chord.event(key(KeyCode::KEY_PAUSE.0, 1)));
    }

}
