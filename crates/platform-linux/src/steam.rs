//! Resolve a per-game Gamescope launch using its own SteamAppId, never a
//! machine-wide search for a running game. Other environment values are discarded.
use std::{fs, io::Read, path::Path};
use std::os::unix::fs::MetadataExt;
const MAX_ENV: usize = 256 * 1024;

fn bounded(path: &Path, limit: usize) -> Option<Vec<u8>> {
    let mut bytes = Vec::new();
    fs::File::open(path).ok()?.take((limit + 1) as u64).read_to_end(&mut bytes).ok()?;
    (bytes.len() <= limit).then_some(bytes)
}
fn steam_app_id(environment: &[u8]) -> Option<u32> {
    if environment.len() > MAX_ENV || !environment.ends_with(&[0]) { return None; }
    let mut found = None;
    for entry in environment.split(|b| *b == 0) {
        if let Some(value) = entry.strip_prefix(b"SteamAppId=") {
            // Canonical positive Steam application IDs only. Never treat a shortcut
            // SteamGameId, a command-line number, or a process ID as an application ID.
            if value.is_empty() || value.len() > 10 || !value.iter().all(u8::is_ascii_digit) { return None; }
            let id = std::str::from_utf8(value).ok()?.parse::<u32>().ok().filter(|id| *id != 0)?;
            if found.is_some_and(|previous| previous != id) { return None; }
            found = Some(id);
        }
    }
    found
}
fn launch_app_id(command_line: &[u8]) -> Option<u32> {
    if !command_line.ends_with(&[0]) { return None; }
    let args: Vec<_> = command_line.split(|b| *b == 0).collect();
    // Steam can set SteamAppId inside its reaper, after Gamescope has already
    // started. Accept only the explicit Steam launch marker in its child command.
    let mut child = args.iter().position(|arg| *arg == b"--")? + 1;
    // Current Steam launches can wrap reaper in steam-launch-wrapper --.
    // Skip only this known wrapper, never search arbitrary game arguments.
    let first = std::str::from_utf8(args.get(child)?).ok()?;
    if Path::new(first).file_name()?.to_str()? == "steam-launch-wrapper" {
        if *args.get(child + 1)? != b"--" { return None; }
        child += 2;
    }
    let executable = std::str::from_utf8(args.get(child)?).ok()?;
    if Path::new(executable).file_name()?.to_str()? != "reaper" || *args.get(child + 1)? != b"SteamLaunch" { return None; }
    let value = args.get(child + 2)?.strip_prefix(b"AppId=")?;
    if *args.get(child + 3)? != b"--" || value.is_empty() || value.len() > 10 || !value.iter().all(u8::is_ascii_digit) { return None; }
    std::str::from_utf8(value).ok()?.parse::<u32>().ok().filter(|id| *id != 0)
}
fn start_time(bytes: &[u8]) -> Option<u64> {
    // comm can contain spaces and parentheses. Fields after its final ')' start
    // with state (field 3); starttime is field 22, index 19 in this suffix.
    let stat = std::str::from_utf8(bytes).ok()?;
    stat.rsplit_once(") ")?.1.split_whitespace().nth(19)?.parse().ok()
}
fn is_gamescope_process(executable: std::io::Result<std::path::PathBuf>, command_line: &[u8], comm: Option<&[u8]>) -> bool {
    match executable {
        Ok(path) => path.file_name().is_some_and(|name| name == "gamescope"),
        // File capabilities (e.g. CAP_SYS_NICE) can deny /proc/PID/exe and
        // environ even for the same UID. Do not require either to read Steam's
        // explicit launch marker from the still-accessible command line.
        Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => {
            let argv0 = command_line.split(|b| *b == 0).next().and_then(|arg| std::str::from_utf8(arg).ok());
            let name = argv0.and_then(|arg| Path::new(arg).file_name()).and_then(|name| name.to_str());
            let comm = comm.and_then(|bytes| std::str::from_utf8(bytes).ok()).map(str::trim);
            name == Some("gamescope") && matches!(comm, Some("gamescope" | "gamescope-wl"))
        }
        Err(_) => false,
    }
}
fn resolve_at(proc_root: &Path, pid: u32) -> Option<String> {
    if pid == 0 { return None; }
    let process = proc_root.join(pid.to_string());
    if fs::metadata(&process).ok()?.uid() != fs::metadata(proc_root.join("self")).ok()?.uid() { return None; }
    let before = start_time(&bounded(&process.join("stat"), 8192)?)?;
    let command_line = bounded(&process.join("cmdline"), MAX_ENV).unwrap_or_default();
    let comm = bounded(&process.join("comm"), 256);
    if !is_gamescope_process(fs::read_link(process.join("exe")), &command_line, comm.as_deref()) { return None; }
    let environment_id = bounded(&process.join("environ"), MAX_ENV).and_then(|bytes| steam_app_id(&bytes));
    let launch_id = launch_app_id(&command_line);
    let app = match (environment_id, launch_id) {
        (Some(a), Some(b)) if a != b => None,
        (Some(id), _) | (_, Some(id)) => Some(id),
        _ => None,
    };
    if start_time(&bounded(&process.join("stat"), 8192)?)? != before { return None; }
    Some(app.map(|id| format!("steam:{id}")).unwrap_or_else(|| "gamescope".into()))
}
/// The daemon supplies pid from an authenticated KWin message, never UI input.
/// Inaccessible processes, missing IDs, other owners and namespace mismatches
/// remain unresolved rather than guessing the identity of a game.
pub fn gamescope_steam_identity(app_id: &str, pid: u32) -> Option<String> {
    gamescope_window_identity(app_id, pid).filter(|id| id.starts_with("steam:"))
}
/// KWin may provide a valid Gamescope PID with both application labels empty.
/// Only return the generic Gamescope label after checking the actual process;
/// the capture UI reports that label as ambiguous instead of silently timing out.
pub fn gamescope_window_identity(app_id: &str, pid: u32) -> Option<String> {
    if !app_id.is_empty() && !app_id.eq_ignore_ascii_case("gamescope") { return None; }
    resolve_at(Path::new("/proc"), pid)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_only_valid_unambiguous_steam_app_ids() {
        assert_eq!(steam_app_id(b"OTHER=discard\0SteamAppId=1808500\0SteamGameId=other\0"), Some(1808500));
        for invalid in [b"SteamAppId=0\0".as_slice(), b"SteamAppId=1\0SteamAppId=2\0", b"SteamGameId=570\0", b"SteamAppId=570", b"SteamAppId=4294967296\0", b"SteamAppId=+570\0", b"SteamAppId=\xff\0"] {
            assert_eq!(steam_app_id(invalid), None);
        }
        assert_eq!(steam_app_id(&vec![0; MAX_ENV + 1]), None);
    }
    #[test]
    fn recognizes_only_the_explicit_steam_child_launch_marker() {
        assert_eq!(launch_app_id(b"gamescope\0-f\0--\0/path/reaper\0SteamLaunch\0AppId=1808500\0--\0game.exe\0"), Some(1808500));
        assert_eq!(launch_app_id(b"gamescope\0-f\0--\0/steam/steam-launch-wrapper\0--\0/steam/reaper\0SteamLaunch\0AppId=1808500\0--\0game.exe\0"), Some(1808500));
        assert_eq!(launch_app_id(b"gamescope\0--\0/steam/steam-launch-wrapper\0wrong\0/steam/reaper\0SteamLaunch\0AppId=1808500\0--\0"), None);
        assert_eq!(launch_app_id(b"gamescope\0--\0game.exe\0SteamLaunch\0AppId=1808500\0--\0"), None);
        assert_eq!(launch_app_id(b"gamescope\0--\0/reaper\0SteamLaunch\0AppId=0\0--\0"), None);
        assert_eq!(launch_app_id(b"gamescope\0--\0/reaper\0SteamLaunch\0AppId=570\0other\0"), None);
    }
    #[test]
    fn capability_restricted_process_can_use_command_line_without_exe_or_environment() {
        let denied = || Err(std::io::Error::from(std::io::ErrorKind::PermissionDenied));
        let command = b"gamescope\0--\0/steam/reaper\0SteamLaunch\0AppId=1808500\0--\0game.exe\0";
        assert!(is_gamescope_process(denied(), command, Some(b"gamescope-wl\n")));
        assert_eq!(launch_app_id(command), Some(1808500));
        assert!(!is_gamescope_process(denied(), command, Some(b"other\n")));
        assert!(!is_gamescope_process(denied(), b"other\0", Some(b"gamescope-wl\n")));
        assert!(!is_gamescope_process(Err(std::io::Error::from(std::io::ErrorKind::NotFound)), command, Some(b"gamescope-wl\n")));
        assert!(!is_gamescope_process(Ok("/bin/other".into()), command, Some(b"gamescope-wl\n")));
    }
    #[test]
    fn resolves_separate_games_and_rejects_wrappers_and_missing_metadata() {
        use std::os::unix::fs::symlink;
        let root = std::env::temp_dir().join(format!("remap-steam-test-{}", std::process::id()));
        fs::create_dir_all(root.join("self")).unwrap();
        for (pid, app) in [(101, 1808500), (102, 570)] {
            let process = root.join(pid.to_string()); fs::create_dir_all(&process).unwrap();
            let mut fields = vec!["0"; 20]; fields[0] = "S"; fields[19] = "1234";
            fs::write(process.join("stat"), format!("{pid} (gamescope (nested)) {}", fields.join(" "))).unwrap();
            symlink("/usr/bin/gamescope", process.join("exe")).unwrap();
            fs::write(process.join("environ"), format!("SteamAppId={app}\0")).unwrap();
            assert_eq!(resolve_at(&root, pid), Some(format!("steam:{app}")));
        }
        fs::remove_file(root.join("101/exe")).unwrap(); symlink("/usr/bin/other", root.join("101/exe")).unwrap();
        assert_eq!(resolve_at(&root, 101), None);
        fs::write(root.join("102/environ"), b"SteamAppId=0\0").unwrap();
        assert_eq!(resolve_at(&root, 102).as_deref(), Some("gamescope"));
        fs::write(root.join("102/cmdline"), b"gamescope\0--\0/steam/reaper\0SteamLaunch\0AppId=1808500\0--\0game.exe\0").unwrap();
        assert_eq!(resolve_at(&root, 102).as_deref(), Some("steam:1808500"));
        fs::write(root.join("102/environ"), b"SteamAppId=570\0").unwrap();
        assert_eq!(resolve_at(&root, 102).as_deref(), Some("gamescope"), "conflicting metadata must not guess a game");
        assert_eq!(resolve_at(&root, 0), None); assert_eq!(resolve_at(&root, 999), None);
        assert_eq!(gamescope_steam_identity("firefox", std::process::id()), None);
        fs::remove_dir_all(root).unwrap();
    }
}
