use std::{fs::{self, File, OpenOptions}, io::{Read, Write}, os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt}, path::{Path, PathBuf}, process::{Command, Stdio}, thread, time::{Duration, Instant}};
use crate::{Action, ApplyResult, Target, HEADER, RULE_DIR, UINPUT_FILE, UINPUT_RULE};

const PREFIX: &str = "72-bindforge-access-";
struct Store { path: PathBuf, owner: u32 }
impl Store {
    fn system() -> Result<Self, String> {
        // Never follow an administrator path redirected into a writable directory.
        for path in ["/etc", "/etc/udev", RULE_DIR] { trusted_directory(Path::new(path), 0)?; }
        Ok(Self { path: RULE_DIR.into(), owner: 0 })
    }
    fn read(&self, name: &str) -> Result<Option<String>, String> {
        let path = self.path.join(name);
        let file = match OpenOptions::new().read(true).custom_flags(rustix::fs::OFlags::NOFOLLOW.bits() as i32).open(&path) {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(format!("Não foi possível consultar a regra sem seguir links: {error}")),
        };
        let metadata = file.metadata().map_err(|e| e.to_string())?;
        if !metadata.is_file() || metadata.uid() != self.owner || metadata.mode() & 0o022 != 0 || metadata.nlink() != 1 || metadata.len() > 8192 {
            return Err("A regra existente não é um arquivo administrativo confiável. Ela não será alterada.".into());
        }
        let mut content = String::new(); file.take(8193).read_to_string(&mut content).map_err(|e| e.to_string())?;
        if !content.starts_with(HEADER) { return Err("A regra existente não pertence ao gerenciador de acesso do BindForge.".into()); }
        Ok(Some(content))
    }
    fn expected(&self, name: &str, expected: &str) -> Result<Option<String>, String> {
        let current = self.read(name)?;
        if current.as_deref().is_some_and(|value| value != expected) { return Err("A regra foi modificada externamente e não será sobrescrita.".into()); }
        Ok(current)
    }
    fn replace(&self, name: &str, content: Option<&str>) -> Result<(), String> {
        let path = self.path.join(name);
        if let Some(content) = content {
            let temporary = self.path.join(format!(".bindforge-access-{}-tmp", std::process::id()));
            let mut file = OpenOptions::new().write(true).create_new(true).mode(0o644).custom_flags(rustix::fs::OFlags::NOFOLLOW.bits() as i32).open(&temporary).map_err(|e| e.to_string())?;
            let result: Result<(), String> = (|| {
                file.set_permissions(fs::Permissions::from_mode(0o644)).map_err(|e| e.to_string())?;
                file.write_all(content.as_bytes()).map_err(|e| e.to_string())?; file.sync_all().map_err(|e| e.to_string())?;
                fs::rename(&temporary, &path).map_err(|e| e.to_string())
            })();
            if result.is_err() { let _ = fs::remove_file(&temporary); }
            result?;
        } else {
            match fs::remove_file(path) { Ok(()) => {}, Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}, Err(e) => return Err(e.to_string()) }
        }
        File::open(&self.path).and_then(|file| file.sync_all()).map_err(|e| e.to_string())
    }
    fn other_grants(&self, excluded: &str) -> Result<bool, String> {
        for entry in fs::read_dir(&self.path).map_err(|e| e.to_string())? {
            let name = entry.map_err(|e| e.to_string())?.file_name().to_string_lossy().into_owned();
            if name != excluded && name.starts_with(PREFIX) && name.ends_with(".rules") {
                // Unknown or edited files make this fail closed, never delete their shared output.
                self.read(&name)?; return Ok(true);
            }
        }
        Ok(false)
    }
    fn apply(&self, target: &Target, action: Action, mut refresh: impl FnMut(bool) -> Result<(), String>) -> Result<ApplyResult, String> {
        let filename = target.selector.filename(); let rule = target.selector.render();
        let old = self.expected(&filename, &rule)?;
        let revoked_name = target.selector.revoked_filename();
        let old_revoked = self.expected(&revoked_name, &rule)?;
        let old_output = self.expected(UINPUT_FILE, UINPUT_RULE)?;
        if matches!(action, Action::Revoke) && old.is_none() { return Err("Não há autorização gerenciada do BindForge para remover nesta interface.".into()); }
        let next = match action { Action::Grant => Some(rule.as_str()), Action::Revoke => None };
        let next_output = match action {
            Action::Grant => Some(UINPUT_RULE),
            Action::Revoke if self.other_grants(&filename)? => old_output.as_deref(),
            Action::Revoke => None,
        };
        let result: Result<(), String> = (|| {
            // Write the denial before removing the rule; native workers observe it directly.
            if matches!(action, Action::Revoke) { self.replace(&revoked_name, Some(&rule))?; }
            self.replace(&filename, next)?;
            self.replace(UINPUT_FILE, next_output)?;
            refresh(matches!(action, Action::Grant))?;
            // Grant only clears a previous denial after the system rule has been applied.
            if matches!(action, Action::Grant) { self.replace(&revoked_name, None)?; }
            Ok(())
        })();
        if let Err(error) = result {
            let restore = self.replace(&filename, old.as_deref()).and_then(|_| self.replace(UINPUT_FILE, old_output.as_deref())).and_then(|_| self.replace(&revoked_name, old_revoked.as_deref())).and_then(|_| refresh(false));
            return Err(match restore { Ok(()) => format!("A operação falhou e as regras anteriores foram restauradas: {error}. Reconecte o dispositivo se alguma permissão chegou a ser aplicada."), Err(rollback) => format!("Falha: {error}. Não foi possível restaurar completamente as regras: {rollback}") });
        }
        Ok(match action {
            Action::Grant => ApplyResult { reconnect_required: false, message: "Autorização persistente instalada. O remapeamento continua dependendo de ativação explícita.".into() },
            Action::Revoke => ApplyResult { reconnect_required: true, message: "Autorização removida. O BindForge bloqueia novas ativações até uma nova concessão. Reconecte o dispositivo para encerrar permissões residuais do sistema; permissões externas são preservadas.".into() },
        })
    }
}
fn trusted_directory(path: &Path, owner: u32) -> Result<(), String> {
    let metadata = fs::symlink_metadata(path).map_err(|e| e.to_string())?;
    if !metadata.is_dir() || metadata.uid() != owner || metadata.mode() & 0o022 != 0 { return Err("O diretório de regras não é um diretório administrativo confiável.".into()); } Ok(())
}
pub fn managed(target: &Target) -> Result<bool, String> {
    Ok(Store::system()?.expected(&target.selector.filename(), &target.selector.render())?.is_some())
}
pub fn revoked(target: &Target) -> Result<bool, String> {
    Ok(Store::system()?.expected(&target.selector.revoked_filename(), &target.selector.render())?.is_some())
}
pub fn ensure_not_revoked(target: &Target) -> Result<(), String> {
    if revoked(target)? { Err("Acesso revogado no BindForge. Use Gerenciar acesso → Conceder acesso antes de ativar novamente.".into()) } else { Ok(()) }
}
/// No inherited command paths, shell, caller-provided environment or unbounded subprocess waits.
fn command(program: &str, args: &[&std::ffi::OsStr]) -> Result<(), String> {
    let mut child = Command::new(program).args(args).env_clear().env("PATH", "/usr/sbin:/usr/bin").env("LANG", "C").stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn().map_err(|e| e.to_string())?;
    let deadline = Instant::now() + Duration::from_secs(12);
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return if status.success() { Ok(()) } else { Err(format!("O sistema não conseguiu aplicar a alteração ({status}).")) },
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(25)),
            other => { let _ = child.kill(); let _ = child.wait(); return Err(format!("A atualização de permissões não terminou no prazo: {other:?}")); }
        }
    }
}
fn udevadm(args: &[&std::ffi::OsStr]) -> Result<(), String> { command("/usr/bin/udevadm", args) }
pub fn apply_system(target: &Target, action: Action) -> Result<ApplyResult, String> {
    if !rustix::process::geteuid().is_root() { return Err("Esta operação exige autenticação administrativa pelo polkit.".into()); }
    let store = Store::system()?;
    let lock = OpenOptions::new().read(true).write(true).create(true).truncate(false).mode(0o600).custom_flags(rustix::fs::OFlags::NOFOLLOW.bits() as i32).open(store.path.join(".bindforge-access.lock")).map_err(|e| e.to_string())?;
    let metadata = lock.metadata().map_err(|e| e.to_string())?;
    if !metadata.is_file() || metadata.uid() != 0 || metadata.mode() & 0o077 != 0 || metadata.nlink() != 1 { return Err("Arquivo de trava de permissões inválido.".into()); }
    rustix::fs::flock(&lock, rustix::fs::FlockOperation::NonBlockingLockExclusive).map_err(|_| "Outra alteração de permissões está em andamento. Tente novamente.".to_string())?;
    // Reinspect after obtaining the lock; never act on a stale syspath/fingerprint.
    let current = crate::inspect(&target.interface_id, target.selector.kind)?;
    if current.fingerprint != target.fingerprint || current.devnode != target.devnode { return Err("O dispositivo mudou. Atualize a consulta antes de autorizar.".into()); }
    if matches!(action, Action::Grant) && !Path::new("/sys/class/misc/uinput").exists() {
        command("/usr/sbin/modprobe", &["uinput".as_ref()])?;
    }
    store.apply(target, action, |activate| {
        udevadm(&["control".as_ref(), "--reload-rules".as_ref()])?;
        if activate {
            udevadm(&["trigger".as_ref(), "--action=change".as_ref(), target.syspath.as_os_str()])?;
            // The shared output is a fixed kernel node, never a caller-supplied path.
            udevadm(&["trigger".as_ref(), "--action=change".as_ref(), "/sys/class/misc/uinput".as_ref()])?;
            udevadm(&["settle".as_ref(), "--timeout=8".as_ref()])?;
        }
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    struct Fixture(Store);
    impl Fixture {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!("bindforge-access-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
            fs::create_dir(&path).unwrap(); Self(Store { path, owner: rustix::process::getuid().as_raw() })
        }
        fn target(&self, serial: &str) -> Target {
            let selector = crate::Selector { kind: crate::Kind::Keyboard, matches: vec![("ENV{ID_SERIAL_SHORT}".into(), serial.into())], scope: "serial".into() };
            Target { name: "Fixture".into(), model: "3554:fa09".into(), interface_id: "fixture".into(), syspath: "/sys/fixture".into(), devnode: "/dev/input/event-fixture".into(), fingerprint: selector.fingerprint(), scope: selector.scope.clone(), selector }
        }
    }
    impl Drop for Fixture { fn drop(&mut self) { let _ = fs::remove_dir_all(&self.0.path); } }
    #[test]
    fn grants_are_specific_and_revoking_one_preserves_other_grants_and_shared_output() {
        let f = Fixture::new(); let a = f.target("a"); let b = f.target("b");
        f.0.apply(&a, Action::Grant, |_| Ok(())).unwrap(); f.0.apply(&b, Action::Grant, |_| Ok(())).unwrap();
        assert!(f.0.read(&a.selector.filename()).unwrap().is_some());
        let result = f.0.apply(&a, Action::Revoke, |_| Ok(())).unwrap(); assert!(result.reconnect_required);
        assert!(f.0.read(&a.selector.filename()).unwrap().is_none());
        assert!(f.0.read(&b.selector.filename()).unwrap().is_some());
        assert!(f.0.read(UINPUT_FILE).unwrap().is_some());
        f.0.apply(&b, Action::Revoke, |_| Ok(())).unwrap(); assert!(f.0.read(UINPUT_FILE).unwrap().is_none());
    }
    #[test]
    fn failed_reload_rolls_back_rules_and_shared_output() {
        let f = Fixture::new(); let a = f.target("a");
        let mut attempts = 0;
        assert!(f.0.apply(&a, Action::Grant, |_| { attempts += 1; if attempts == 1 { Err("fixture reload failure".into()) } else { Ok(()) } }).unwrap_err().contains("restauradas"));
        assert!(f.0.read(&a.selector.filename()).unwrap().is_none()); assert!(f.0.read(UINPUT_FILE).unwrap().is_none());
        f.0.apply(&a, Action::Grant, |_| Ok(())).unwrap();
        let before = f.0.read(&a.selector.filename()).unwrap();
        let mut attempts = 0;
        assert!(f.0.apply(&a, Action::Revoke, |_| { attempts += 1; if attempts == 1 { Err("failure".into()) } else { Ok(()) } }).is_err());
        assert_eq!(f.0.read(&a.selector.filename()).unwrap(), before);
    }
    #[test]
    fn edited_files_symlinks_hardlinks_and_writable_files_are_not_overwritten_or_removed() {
        let f = Fixture::new(); let a = f.target("a"); let path = f.0.path.join(a.selector.filename());
        fs::write(&path, "# Administrator rule\n").unwrap();
        assert!(f.0.apply(&a, Action::Grant, |_| Ok(())).is_err()); fs::remove_file(&path).unwrap();
        let external = f.0.path.join("external"); fs::write(&external, a.selector.render()).unwrap();
        std::os::unix::fs::symlink(&external, &path).unwrap();
        assert!(f.0.apply(&a, Action::Revoke, |_| Ok(())).is_err()); fs::remove_file(&path).unwrap();
        fs::hard_link(&external, &path).unwrap(); assert!(f.0.apply(&a, Action::Grant, |_| Ok(())).is_err()); fs::remove_file(&path).unwrap();
        fs::write(&path, a.selector.render()).unwrap(); fs::set_permissions(&path, fs::Permissions::from_mode(0o666)).unwrap();
        assert!(f.0.apply(&a, Action::Grant, |_| Ok(())).is_err());
        assert_eq!(fs::read_to_string(&external).unwrap(), a.selector.render());
    }
    #[test]
    fn revoke_never_creates_shared_permissions_and_rejects_an_unmanaged_target() {
        let f = Fixture::new(); let a = f.target("a"); let b = f.target("b");
        assert!(f.0.apply(&a, Action::Revoke, |_| Ok(())).is_err());
        f.0.apply(&a, Action::Grant, |_| Ok(())).unwrap(); f.0.apply(&b, Action::Grant, |_| Ok(())).unwrap();
        fs::remove_file(f.0.path.join(UINPUT_FILE)).unwrap();
        f.0.apply(&a, Action::Revoke, |_| Ok(())).unwrap();
        assert!(f.0.read(UINPUT_FILE).unwrap().is_none());
        assert!(f.0.read(&b.selector.filename()).unwrap().is_some());
    }

    #[test]
    fn explicit_revocation_survives_residual_os_access_and_only_successful_grant_clears_it() {
        let f = Fixture::new(); let a = f.target("a"); let b = f.target("b");
        f.0.apply(&a, Action::Grant, |_| Ok(())).unwrap();
        f.0.apply(&a, Action::Revoke, |_| Ok(())).unwrap();
        assert!(f.0.expected(&a.selector.revoked_filename(), &a.selector.render()).unwrap().is_some());
        assert!(f.0.read(&b.selector.revoked_filename()).unwrap().is_none());
        // Metadata queries never consult or change OS ACLs: a readable node cannot erase denial.
        let mut attempts = 0;
        assert!(f.0.apply(&a, Action::Grant, |_| { attempts += 1; if attempts == 1 { Err("failure".into()) } else { Ok(()) } }).is_err());
        assert!(f.0.read(&a.selector.revoked_filename()).unwrap().is_some());
        f.0.apply(&a, Action::Grant, |_| Ok(())).unwrap();
        assert!(f.0.read(&a.selector.revoked_filename()).unwrap().is_none());
    }

}
