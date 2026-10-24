#[cfg(target_os = "linux")]
use bindforge_access::{Action, Kind};
#[cfg(target_os = "linux")]
fn run() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [action, kind, interface, fingerprint] = args.as_slice() else { return Err("Uso: bindforge-device-access grant|revoke mouse|keyboard <interface> <fingerprint>".into()); };
    let action = Action::parse(action)?; let kind = Kind::parse(kind)?;
    if fingerprint.len() != 64 || !fingerprint.bytes().all(|b| b.is_ascii_hexdigit()) { return Err("Identidade de dispositivo inválida.".into()); }
    if !rustix::process::geteuid().is_root() { return Err("Use a autorização administrativa do aplicativo.".into()); }
    let target = bindforge_access::inspect(interface, kind)?;
    if target.fingerprint != *fingerprint { return Err("O dispositivo mudou durante a autenticação. Atualize a lista e tente novamente.".into()); }
    let result = bindforge_access::apply_system(&target, action)?;
    println!("{}", serde_json::to_string(&result).map_err(|e| e.to_string())?); Ok(())
}
#[cfg(target_os = "linux")]
fn main() {
    if let Err(error) = run() { eprintln!("{error}"); std::process::exit(1); }
}

#[cfg(not(target_os = "linux"))]
fn main() { eprintln!("Gerenciamento de acesso disponível apenas no Linux."); std::process::exit(1); }
