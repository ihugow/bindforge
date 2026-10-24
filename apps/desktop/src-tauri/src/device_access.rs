//! User-side metadata/status and fixed-path polkit invocation. Never elevate the desktop.
use std::{fs, io::Read, os::unix::fs::MetadataExt, process::{Command, Stdio}, sync::Mutex, thread, time::{Duration, Instant}};
use bindforge_access::{AccessStatus, Action, ApplyResult, Kind, Target, HELPER, POLICY};
static OPERATION: Mutex<()> = Mutex::new(());

fn resolve_target(id: &str) -> Result<(Target, remap_config::Config), String> {
    let config = remap_config::load(&remap_config::config_path()?)?;
    config.validate()?;
    let device = config.devices.iter().find(|device| device.id == id).ok_or("Selecione um dispositivo cadastrado e salvo.")?;
    Ok((bindforge_access::inspect(&device.interface_id, Kind::parse(&device.kind)?)?, config))
}
fn trusted_file(path: &str, executable: bool) -> bool {
    fs::symlink_metadata(path).is_ok_and(|m| m.is_file() && m.uid() == 0 && m.mode() & 0o022 == 0 && (!executable || m.mode() & 0o111 != 0))
}
fn helper_available() -> bool {
    ["/usr", "/usr/libexec", "/usr/libexec/bindforge", "/usr/share", "/usr/share/polkit-1", "/usr/share/polkit-1/actions"].iter().all(|p| fs::symlink_metadata(p).is_ok_and(|m| m.is_dir() && m.uid() == 0 && m.mode() & 0o022 == 0))
        && trusted_file(HELPER, true) && trusted_file(POLICY, false) && trusted_file("/usr/bin/pkexec", true)
}
fn status_for(target: &Target) -> Result<AccessStatus, String> {
    let readable = rustix::fs::access(&target.devnode, rustix::fs::Access::READ_OK).is_ok();
    let output_writable = rustix::fs::access("/dev/uinput", rustix::fs::Access::WRITE_OK).is_ok();
    let managed = bindforge_access::managed(target)?;
    let revoked = bindforge_access::revoked(target)?;
    let available = helper_available();
    let message = if !available { "O componente administrativo precisa ser instalado ou atualizado com o pacote BindForge." }
        else if revoked { "Acesso revogado no BindForge. Conceda acesso novamente para habilitar o remapeamento, mesmo que ainda haja permissão no sistema." }
        else if managed && readable && output_writable { "Autorização persistente do BindForge ativa para a sessão local." }
        else if managed { "A regra está instalada. Atualize o acesso ou reconecte o dispositivo para aplicar as permissões." }
        else if readable && output_writable { "Há acesso por permissões existentes. Nenhuma autorização gerenciada pelo BindForge está instalada para esta interface." }
        else { "Este dispositivo precisa de autorização para o remapeamento físico." };
    Ok(AccessStatus { fingerprint: target.fingerprint.clone(), device_name: target.name.clone(), model: target.model.clone(), readable, output_writable, managed, revoked, helper_available: available, scope: target.scope.clone(), message: message.into() })
}
pub fn status(id: &str) -> Result<AccessStatus, String> { status_for(&resolve_target(id)?.0) }

#[derive(serde::Serialize)]
pub struct AccessResult { pub status: AccessStatus, pub reconnect_required: bool, pub message: String }
pub fn control(id: &str, action: Action, fingerprint: &str) -> Result<AccessResult, String> {
    let _guard = OPERATION.try_lock().map_err(|_| "Outra autorização de acesso está em andamento.".to_string())?;
    let (target, config) = resolve_target(id)?;
    if fingerprint != target.fingerprint { return Err("A interface mudou desde a consulta. Atualize o estado e revise o dispositivo antes de autorizar.".into()); }
    if !helper_available() { return Err("Instale ou atualize o componente administrativo do BindForge antes de gerenciar o acesso.".into()); }
    if matches!(action, Action::Revoke) { pause_target(&target, &config)?; }
    let device = config.devices.iter().find(|d| d.id == id).expect("validated registration");
    let result = execute(action, &device.kind, &target)?;
    if matches!(action, Action::Revoke) { pause_target(&target, &config)?; }
    let current = resolve_target(id)?.0;
    if current.fingerprint != target.fingerprint { return Err("O dispositivo mudou após a operação. Atualize a lista para conferir a autorização.".into()); }
    Ok(AccessResult { status: status_for(&current)?, reconnect_required: result.reconnect_required, message: result.message })
}
fn pause_target(target: &Target, config: &remap_config::Config) -> Result<(), String> {
    if daemon_present()? {
        // Pause either kind using this interface. An already-open fd survives an ACL change.
        for device in config.devices.iter().filter(|d| d.interface_id == target.interface_id) {
            let keyboard = device.kind == "keyboard";
            let current = remap_ipc::dbus::session_input(if keyboard { remap_ipc::dbus::InputCommand::KeyboardStatus } else { remap_ipc::dbus::InputCommand::Status })?;
            if current.active && current.device_id == device.id {
                remap_ipc::dbus::session_input(if keyboard { remap_ipc::dbus::InputCommand::PauseKeyboard } else { remap_ipc::dbus::InputCommand::Pause })?;
            }
        }
    }
    Ok(())
}
fn daemon_present() -> Result<bool, String> {
    let connection = zbus::blocking::connection::Builder::session().map_err(|e| e.to_string())?.method_timeout(Duration::from_secs(5)).build().map_err(|e| e.to_string())?;
    let proxy = zbus::blocking::fdo::DBusProxy::new(&connection).map_err(|e| e.to_string())?;
    match proxy.get_name_owner(remap_ipc::dbus::SERVICE.try_into().expect("valid service")) {
        Ok(_) => Ok(true),
        Err(zbus::fdo::Error::NameHasNoOwner(_) | zbus::fdo::Error::ServiceUnknown(_)) => Ok(false),
        Err(error) => Err(format!("Não foi possível verificar se a captura está ativa: {error}")),
    }
}
fn execute(action: Action, kind: &str, target: &Target) -> Result<ApplyResult, String> {
    let action = match action { Action::Grant => "grant", Action::Revoke => "revoke" };
    let mut child = Command::new("/usr/bin/pkexec").arg("--disable-internal-agent").arg(HELPER).args([action, kind, &target.interface_id, &target.fingerprint])
        .env_clear().env("PATH", "/usr/sbin:/usr/bin").env("LANG", "pt_BR.UTF-8")
        .stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().map_err(|e| e.to_string())?;
    let deadline = Instant::now() + Duration::from_secs(180);
    let exit = loop {
        match child.try_wait() {
            Ok(Some(exit)) => break exit,
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(50)),
            _ => { let _ = child.kill(); let _ = child.wait(); return Err("A autenticação não terminou no prazo. Atualize o estado antes de tentar novamente.".into()); }
        }
    };
    let mut output = String::new(); child.stdout.take().ok_or("Resposta administrativa indisponível.")?.take(8193).read_to_string(&mut output).map_err(|e| e.to_string())?;
    let mut error = String::new(); child.stderr.take().ok_or("Diagnóstico administrativo indisponível.")?.take(8193).read_to_string(&mut error).map_err(|e| e.to_string())?;
    decode_reply(exit.success(), exit.code(), &output, &error)
}
fn decode_reply(success: bool, code: Option<i32>, output: &str, error: &str) -> Result<ApplyResult, String> {
    if !success {
        return Err(match code {
            Some(126) => "Autenticação cancelada. Nenhuma nova autorização foi confirmada.".into(),
            Some(127) => "A operação não foi autorizada. Verifique o agente de autenticação da sessão e tente novamente.".into(),
            _ => if error.is_empty() { "Não foi possível alterar a autorização do dispositivo.".into() } else { error.chars().filter(|c| !c.is_control()).take(2048).collect() },
        });
    }
    if output.len() > 8192 { return Err("Resposta administrativa excedeu o limite.".into()); }
    serde_json::from_str(output).map_err(|_| "Resposta administrativa inválida. Atualize o estado para conferir a autorização.".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn untrusted_or_absent_helpers_are_never_executed() {
        assert!(!trusted_file("/not-a-helper", true));
        let path = std::env::temp_dir().join(format!("bindforge-helper-check-{}", std::process::id()));
        fs::write(&path, "no code").unwrap();
        assert!(!trusted_file(path.to_str().unwrap(), true)); fs::remove_file(path).unwrap();
    }
    #[test]
    fn canceled_denied_and_malformed_replies_never_report_success() {
        assert!(decode_reply(false, Some(126), "", "").unwrap_err().contains("cancelada"));
        assert!(decode_reply(false, Some(127), "", "").unwrap_err().contains("não foi autorizada"));
        assert!(decode_reply(true, Some(0), "not json", "").is_err());
        assert!(decode_reply(true, Some(0), &"x".repeat(8193), "").is_err());
        let result = decode_reply(true, Some(0), r#"{"reconnect_required":true,"message":"removed"}"#, "").unwrap();
        assert!(result.reconnect_required);
    }

}
