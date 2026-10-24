//! Read-only preview; never reads input events or changes system permissions.
#[cfg(target_os = "linux")]
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [kind, id] = args.as_slice() else { eprintln!("Uso: preview mouse|keyboard <interface-id>"); std::process::exit(1); };
    match bindforge_access::Kind::parse(kind).and_then(|kind| bindforge_access::inspect(id, kind)) {
        Ok(target) => print!("{}", target.rule_preview()),
        Err(error) => { eprintln!("{error}"); std::process::exit(1); }
    }
}

#[cfg(not(target_os = "linux"))]
fn main() { eprintln!("Prévia disponível apenas no Linux."); }
