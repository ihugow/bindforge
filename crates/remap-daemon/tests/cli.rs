use std::io::Write;
use std::process::{Command, Stdio};

#[test]
fn demo_switches_profiles_without_duplicate_application() {
    let output = Command::new(env!("CARGO_BIN_EXE_remap-daemon")).output().unwrap();
    assert!(output.status.success());
    assert_eq!(String::from_utf8(output.stdout).unwrap(),
        "simulation profile=default\nsimulation profile=firefox\nsimulation profile=konsole\nsimulation profile=default\n");
    assert!(String::from_utf8(output.stderr).unwrap().contains("SIMULATION ONLY"));
}

#[test]
fn stdin_focus_clear_and_eof_exit_cleanly() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_remap-daemon"))
        .arg("--stdin").stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped())
        .spawn().unwrap();
    child.stdin.take().unwrap().write_all(b"firefox\n\n").unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    assert_eq!(String::from_utf8(output.stdout).unwrap(),
        "simulation profile=default\nsimulation profile=firefox\nsimulation profile=default\n");
}

#[test]
fn unknown_option_is_an_error() {
    let output = Command::new(env!("CARGO_BIN_EXE_remap-daemon"))
        .arg("--native").output().unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8(output.stderr).unwrap().contains("unknown arguments"));
}
