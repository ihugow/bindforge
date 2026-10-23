//! Diagnostic: only prints the resolved identity, never process arguments/environment.
fn main() {
    let pid = std::env::args().nth(1).and_then(|value| value.parse::<u32>().ok())
        .expect("usage: cargo run -p platform-linux --example identify-gamescope -- PID");
    #[cfg(target_os = "linux")]
    match platform_linux::gamescope_steam_identity("gamescope", pid) {
        Some(identity) => println!("{identity}"),
        None => { eprintln!("Gamescope game identity unavailable"); std::process::exit(1); }
    }
    #[cfg(not(target_os = "linux"))]
    { let _ = pid; eprintln!("Linux only"); std::process::exit(1); }
}
