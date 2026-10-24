#![cfg(target_os = "linux")]
use std::process::Command;
fn rejected(args: &[&str]) -> String {
    let output = Command::new(env!("CARGO_BIN_EXE_bindforge-device-access")).args(args).output().unwrap();
    assert!(!output.status.success()); assert!(output.stdout.is_empty());
    String::from_utf8(output.stderr).unwrap()
}
#[test]
fn unexpected_actions_and_extra_arguments_never_reach_system_operations() {
    assert!(rejected(&[]).contains("Uso:"));
    assert!(rejected(&["shell", "keyboard", "anything", &"a".repeat(64)]).contains("Ação"));
    assert!(rejected(&["grant", "keyboard", "anything", &"a".repeat(64), "--rules-dir=/tmp"]).contains("Uso:"));
}
#[test]
fn paths_types_and_fingerprints_cannot_be_used_as_an_arbitrary_root_operation() {
    for args in [vec!["grant", "other", "/etc/shadow", "invalid"], vec!["grant", "keyboard", "/etc/shadow", "invalid"], vec!["grant", "keyboard", "linux:/sys/devices/../../etc/shadow", "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"]] {
        rejected(&args);
    }
}
