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
        "CTRL" | "CONTROL" => "KEY_LEFTCTRL", "SHIFT" => "KEY_LEFTSHIFT", "ALT" => "KEY_LEFTALT",
        "SUPER" | "META" | "WIN" => "KEY_LEFTMETA", "ESC" => "KEY_ESC", "RETURN" => "KEY_ENTER",
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
            let action = match &binding.action {
                Action::Disabled => vec![], Action::MouseButton(b) => vec![mouse_code(*b)?],
                Action::KeyChord(keys) => {
                    if keys.is_empty() || keys.len() > 16 { return Err("O atalho deve conter de 1 a 16 teclas.".into()); }
                    let keys: Vec<_> = keys.iter().map(|k| key_code(k)).collect::<Result<_, _>>()?;
                    if keys.iter().copied().collect::<BTreeSet<_>>().len() != keys.len() { return Err("O atalho contém teclas físicas repetidas.".into()); }
                    keys
                }
            };
            result.insert(mouse_code(button)?, action);
        }
        Ok(Self(result))
    }
    fn output(&self, code: u16) -> Vec<u16> { self.0.get(&code).cloned().unwrap_or_else(|| vec![code]) }
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
            2 if !self.plan.0.contains_key(&code) && self.held.contains_key(&code) => out.push(event),
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
}
