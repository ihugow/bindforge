//! Controls only BindForge's user service. Never signals an independently started daemon.
use std::{path::{Path, PathBuf}, time::{Duration, Instant}};
use zbus::{blocking::{Connection, Proxy, connection::Builder}, zvariant::OwnedObjectPath};

const UNIT: &str = "bindforge-daemon.service";
const TEMPLATE: &str = include_str!("../../../../integrations/systemd/bindforge-daemon.service");
const PACKAGED_BINARY: &str = "/usr/libexec/bindforge/remap-daemon";

#[derive(serde::Serialize, Debug)]
pub struct ServiceStatus {
    pub active_state: String,
    pub autostart: bool,
    pub connected: bool,
    pub external: bool,
    pub binary_available: bool,
    pub result: String,
    pub exit_status: i32,
}

fn connection() -> Result<Connection, String> {
    Builder::session().map_err(error)?.method_timeout(Duration::from_secs(5)).build().map_err(error)
}
fn error(e: impl std::fmt::Display) -> String { format!("Não foi possível gerenciar o serviço do usuário: {e}") }
fn manager(connection: &Connection) -> Result<Proxy<'_>, String> {
    Proxy::new(connection, "org.freedesktop.systemd1", "/org/freedesktop/systemd1", "org.freedesktop.systemd1.Manager").map_err(error)
}
fn missing(e: &zbus::Error) -> bool {
    matches!(e, zbus::Error::MethodError(name, _, _) if matches!(name.as_str(),
        "org.freedesktop.systemd1.NoSuchUnit" | "org.freedesktop.systemd1.NoSuchUnitFile" | "org.freedesktop.DBus.Error.FileNotFound"))
}
fn daemon_pid(connection: &Connection) -> Result<Option<u32>, String> {
    let bus = zbus::blocking::fdo::DBusProxy::new(connection).map_err(error)?;
    match bus.get_connection_unix_process_id(remap_ipc::dbus::SERVICE.try_into().expect("valid name")) {
        Ok(pid) => Ok(Some(pid)),
        Err(zbus::fdo::Error::NameHasNoOwner(_)) | Err(zbus::fdo::Error::ServiceUnknown(_)) => Ok(None),
        Err(e) => Err(error(e)),
    }
}
fn binary() -> PathBuf {
    if cfg!(debug_assertions) {
        // cargo build -p remap-daemon --features kde-dbus
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../target/debug/remap-daemon")
    } else { PathBuf::from(PACKAGED_BINARY) }
}
fn executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    path.metadata().is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
}
fn snapshot(connection: &Connection, executable_path: &Path) -> Result<ServiceStatus, String> {
    let manager = manager(connection)?;
    let enabled: String = match manager.call("GetUnitFileState", &(UNIT,)) {
        Ok(state) => state, Err(e) if missing(&e) => "disabled".into(), Err(e) => return Err(error(e)),
    };
    let mut state = ServiceStatus { active_state: "inactive".into(), autostart: matches!(enabled.as_str(), "enabled" | "enabled-runtime"),
        connected: false, external: false, binary_available: executable(executable_path), result: "success".into(), exit_status: 0 };
    let mut main_pid = 0;
    let path: Result<OwnedObjectPath, _> = manager.call("GetUnit", &(UNIT,));
    match path {
        Ok(path) => {
            let unit = Proxy::new(connection, "org.freedesktop.systemd1", path.clone(), "org.freedesktop.systemd1.Unit").map_err(error)?;
            state.active_state = unit.get_property("ActiveState").map_err(error)?;
            let service = Proxy::new(connection, "org.freedesktop.systemd1", path, "org.freedesktop.systemd1.Service").map_err(error)?;
            main_pid = service.get_property::<u32>("MainPID").map_err(error)?;
            state.result = service.get_property("Result").map_err(error)?;
            state.exit_status = service.get_property("ExecMainStatus").map_err(error)?;
        }
        Err(e) if missing(&e) => {}, Err(e) => return Err(error(e)),
    }
    if let Some(pid) = daemon_pid(connection)? {
        state.connected = true;
        state.external = pid != main_pid || main_pid == 0;
    }
    Ok(state)
}
pub fn status() -> Result<ServiceStatus, String> { snapshot(&connection()?, &binary()) }

fn config_root() -> Result<PathBuf, String> {
    let root = std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from).filter(|p| p.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|p| PathBuf::from(p).join(".config")))
        .ok_or("Diretório de configuração do usuário indisponível.")?;
    Ok(root.join("systemd/user"))
}
fn render_unit(binary: &Path) -> Result<String, String> {
    let path = binary.to_str().ok_or("Caminho do daemon inválido.")?;
    if !binary.is_absolute() || path.chars().any(char::is_control) { return Err("Caminho do daemon inválido.".into()); }
    // ExecStart is systemd syntax, never shell syntax. Escape specifiers and expansion.
    let escaped = path.replace('\\', "\\\\").replace('"', "\\\"").replace('%', "%%").replace('$', "$$");
    Ok(TEMPLATE.replace(PACKAGED_BINARY, &format!("\"{escaped}\"")))
}
fn install(connection: &Connection, root: &Path, binary: &Path) -> Result<(), String> {
    if !executable(binary) { return Err("Executável do daemon não encontrado. Instale o pacote completo ou compile remap-daemon com a feature kde-dbus.".into()); }
    let rendered = render_unit(binary)?;
    std::fs::create_dir_all(root).map_err(error)?;
    let path = root.join(UNIT);
    if let Ok(meta) = std::fs::symlink_metadata(&path) {
        if !meta.is_file() || !std::fs::read_to_string(&path).map_err(error)?.starts_with("# Managed by BindForge\n") {
            return Err("Existe uma unidade personalizada com o mesmo nome. Preserve ou remova essa configuração antes de continuar.".into());
        }
    }
    if std::fs::read_to_string(&path).ok().as_deref() != Some(&rendered) {
        let nonce = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_err(error)?.as_nanos();
        let temporary = root.join(format!(".{UNIT}.{}.{nonce}.tmp", std::process::id()));
        use std::io::Write;
        let mut file = std::fs::OpenOptions::new().write(true).create_new(true).open(&temporary).map_err(error)?;
        let written = file.write_all(rendered.as_bytes()).and_then(|_| file.sync_all()).and_then(|_| std::fs::rename(&temporary, &path));
        if written.is_err() { let _ = std::fs::remove_file(&temporary); }
        written.map_err(error)?;
    }
    manager(connection)?.call::<_, _, ()>("Reload", &()).map_err(error)
}

#[derive(serde::Deserialize, Debug)]
#[serde(rename_all = "snake_case")]
pub enum ServiceAction { Start, Stop, Restart, EnableAutostart, DisableAutostart }

// One lifecycle mutation at a time, including repeated clicks from multiple windows.
static CONTROL: std::sync::Mutex<()> = std::sync::Mutex::new(());
pub fn control(action: ServiceAction) -> Result<ServiceStatus, String> {
    let _guard = CONTROL.lock().map_err(error)?;
    control_at(&connection()?, &config_root()?, &binary(), action)
}
fn control_at(connection: &Connection, root: &Path, binary: &Path, action: ServiceAction) -> Result<ServiceStatus, String> {
    let current = snapshot(connection, binary)?;
    if current.external && matches!(action, ServiceAction::Start | ServiceAction::Stop | ServiceAction::Restart) {
        return Err("O daemon foi iniciado fora do aplicativo. Encerre-o no terminal de origem e clique em Iniciar. Seus perfis serão preservados.".into());
    }
    if matches!(action, ServiceAction::Start | ServiceAction::Restart | ServiceAction::EnableAutostart) { install(connection, root, binary)?; }
    let manager = manager(connection)?;
    match action {
        ServiceAction::EnableAutostart => {
            let _: (bool, Vec<(String, String, String)>) = manager.call("EnableUnitFiles", &(vec![UNIT], false, false)).map_err(error)?;
            manager.call::<_, _, ()>("Reload", &()).map_err(error)?;
        }
        ServiceAction::DisableAutostart => {
            let _: Vec<(String, String, String)> = manager.call("DisableUnitFiles", &(vec![UNIT], false)).map_err(error)?;
            manager.call::<_, _, ()>("Reload", &()).map_err(error)?;
        }
        action => {
            let method = match action { ServiceAction::Start => "StartUnit", ServiceAction::Stop => "StopUnit", _ => "RestartUnit" };
            let job: OwnedObjectPath = manager.call(method, &(UNIT, "replace")).map_err(error)?;
            let job_id: u32 = job.as_str().rsplit('/').next().and_then(|s| s.parse().ok()).ok_or("Identificador de operação inválido.")?;
            let deadline = Instant::now() + Duration::from_secs(18);
            loop {
                let pending: Result<OwnedObjectPath, _> = manager.call("GetJob", &(job_id,));
                let job_done = match pending {
                    Ok(_) => false,
                    Err(zbus::Error::MethodError(name, _, _)) if name.as_str() == "org.freedesktop.systemd1.NoSuchJob" => true,
                    Err(e) => return Err(error(e)),
                };
                let state = snapshot(connection, binary)?;
                let done = if matches!(action, ServiceAction::Stop) { matches!(state.active_state.as_str(), "inactive" | "failed") && !state.connected }
                    else { state.active_state == "active" && state.connected && !state.external };
                if done && job_done { return Ok(state); }
                if (job_done && state.active_state == "failed") || Instant::now() >= deadline {
                    return Err(format!("O serviço não concluiu a operação. Estado: {}; resultado: {}; código: {}. Confira o diagnóstico e tente novamente.", state.active_state, state.result, state.exit_status));
                }
                std::thread::sleep(Duration::from_millis(100));
            }
        }
    }
    snapshot(connection, binary)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{io::{BufRead, BufReader}, process::{Child, Command, Stdio}, sync::{Arc, Mutex}};
    struct Bus(Child);
    impl Drop for Bus { fn drop(&mut self) { let _ = self.0.kill(); let _ = self.0.wait(); } }
    #[derive(Default)]
    struct FakeState { active: bool, enabled: bool, installed: bool, pending: u32, calls: Vec<String> }
    struct FakeManager(Arc<Mutex<FakeState>>);
    struct FakeUnit(Arc<Mutex<FakeState>>);
    struct FakeService(Arc<Mutex<FakeState>>);
    #[derive(Debug, zbus::DBusError)]
    #[zbus(prefix = "org.freedesktop.systemd1")]
    enum SystemdError { NoSuchJob(String), NoSuchUnit(String) }
    #[zbus::interface(name = "org.freedesktop.systemd1.Manager")]
    impl FakeManager {
        fn get_unit_file_state(&self, name: &str) -> zbus::fdo::Result<String> {
            assert_eq!(name, UNIT);
            let state = self.0.lock().unwrap();
            if !state.installed { return Err(zbus::fdo::Error::FileNotFound("Not installed".into())); }
            Ok(if state.enabled { "enabled" } else { "disabled" }.into())
        }
        fn get_unit(&self, name: &str) -> Result<OwnedObjectPath, SystemdError> {
            assert_eq!(name, UNIT);
            if !self.0.lock().unwrap().installed { return Err(SystemdError::NoSuchUnit("Not loaded".into())); }
            Ok("/org/freedesktop/systemd1/unit/test".try_into().unwrap())
        }
        fn reload(&self) { let mut state = self.0.lock().unwrap(); state.installed = true; state.calls.push("reload".into()); }
        fn enable_unit_files(&self, names: Vec<String>, runtime: bool, force: bool) -> (bool, Vec<(String, String, String)>) {
            assert_eq!(names, [UNIT]); assert!(!runtime); assert!(!force);
            self.0.lock().unwrap().enabled = true; (true, vec![])
        }
        fn disable_unit_files(&self, names: Vec<String>, runtime: bool) -> Vec<(String, String, String)> {
            assert_eq!(names, [UNIT]); assert!(!runtime);
            self.0.lock().unwrap().enabled = false; vec![]
        }
        async fn start_unit(&self, name: &str, mode: &str, #[zbus(connection)] connection: &zbus::Connection) -> OwnedObjectPath {
            assert_eq!(name, UNIT); assert_eq!(mode, "replace");
            connection.request_name(remap_ipc::dbus::SERVICE).await.unwrap();
            let mut state = self.0.lock().unwrap(); state.active = true; state.pending = 2; state.calls.push("start".into());
            "/org/freedesktop/systemd1/job/1".try_into().unwrap()
        }
        async fn stop_unit(&self, name: &str, mode: &str, #[zbus(connection)] connection: &zbus::Connection) -> OwnedObjectPath {
            assert_eq!(name, UNIT); assert_eq!(mode, "replace");
            connection.release_name(remap_ipc::dbus::SERVICE).await.unwrap();
            let mut state = self.0.lock().unwrap(); state.active = false; state.calls.push("stop".into());
            "/org/freedesktop/systemd1/job/2".try_into().unwrap()
        }
        async fn restart_unit(&self, name: &str, mode: &str, #[zbus(connection)] connection: &zbus::Connection) -> OwnedObjectPath {
            let job = self.start_unit(name, mode, connection).await;
            self.0.lock().unwrap().calls.push("restart".into()); job
        }
        fn get_job(&self, _id: u32) -> Result<OwnedObjectPath, SystemdError> {
            let mut state = self.0.lock().unwrap();
            state.calls.push("job".into());
            if state.pending > 0 { state.pending -= 1; return Ok("/org/freedesktop/systemd1/job/1".try_into().unwrap()); }
            Err(SystemdError::NoSuchJob("Complete".into()))
        }
    }
    #[zbus::interface(name = "org.freedesktop.systemd1.Unit")]
    impl FakeUnit {
        #[zbus(property)]
        fn active_state(&self) -> String { if self.0.lock().unwrap().active { "active" } else { "inactive" }.into() }
    }
    #[zbus::interface(name = "org.freedesktop.systemd1.Service")]
    impl FakeService {
        #[zbus(property, name = "MainPID")]
        fn main_pid(&self) -> u32 { if self.0.lock().unwrap().active { std::process::id() } else { 0 } }
        #[zbus(property)]
        fn result(&self) -> &str { "success" }
        #[zbus(property)]
        fn exec_main_status(&self) -> i32 { 0 }
    }
    #[test]
    fn isolated_service_lifecycle_and_external_daemon_protection() {
        let mut bus = Bus(Command::new("dbus-daemon").args(["--session", "--nofork", "--print-address=1"])
            .stdout(Stdio::piped()).spawn().unwrap());
        let mut address = String::new(); BufReader::new(bus.0.stdout.take().unwrap()).read_line(&mut address).unwrap();
        let state = Arc::new(Mutex::new(FakeState::default()));
        let server = Builder::address(address.trim()).unwrap().name("org.freedesktop.systemd1").unwrap()
            .serve_at("/org/freedesktop/systemd1", FakeManager(state.clone())).unwrap()
            .serve_at("/org/freedesktop/systemd1/unit/test", FakeUnit(state.clone())).unwrap()
            .serve_at("/org/freedesktop/systemd1/unit/test", FakeService(state.clone())).unwrap().build().unwrap();
        let client = Builder::address(address.trim()).unwrap().method_timeout(Duration::from_secs(2)).build().unwrap();
        let root = std::env::temp_dir().join(format!("bindforge-service-test-{}", std::process::id()));
        std::fs::create_dir(&root).unwrap();
        let binary = Path::new("/usr/bin/true");
        assert!(!snapshot(&client, binary).unwrap().connected);
        assert!(control_at(&client, &root, binary, ServiceAction::EnableAutostart).unwrap().autostart);
        assert!(!state.lock().unwrap().active, "enable must not start the daemon");
        let started = control_at(&client, &root, binary, ServiceAction::Start).unwrap();
        assert!(started.connected && !started.external);
        assert_eq!(state.lock().unwrap().pending, 0, "wait until the queued job completes");
        assert!(std::fs::read_to_string(root.join(UNIT)).unwrap().contains("ExecStart=\"/usr/bin/true\" --kde"));
        assert!(control_at(&client, &root, binary, ServiceAction::Restart).unwrap().connected);
        let disabled = control_at(&client, &root, binary, ServiceAction::DisableAutostart).unwrap();
        assert!(!disabled.autostart && disabled.connected, "disable must not stop the daemon");
        assert!(!control_at(&client, &root, binary, ServiceAction::Stop).unwrap().connected);
        let external = Builder::address(address.trim()).unwrap().name(remap_ipc::dbus::SERVICE).unwrap().build().unwrap();
        assert!(snapshot(&client, binary).unwrap().external);
        let calls = state.lock().unwrap().calls.clone();
        for action in [ServiceAction::Start, ServiceAction::Stop, ServiceAction::Restart] {
            assert!(control_at(&client, &root, binary, action).unwrap_err().contains("fora do aplicativo"));
        }
        assert_eq!(state.lock().unwrap().calls, calls);
        drop(external);
        std::fs::write(root.join(UNIT), "custom configuration").unwrap();
        assert!(control_at(&client, &root, binary, ServiceAction::Start).unwrap_err().contains("personalizada"));
        assert_eq!(std::fs::read_to_string(root.join(UNIT)).unwrap(), "custom configuration");
        assert!(control_at(&client, &root, Path::new("/missing/daemon"), ServiceAction::Start).is_err());
        drop(server); std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn unit_escapes_paths_and_keeps_capture_opt_in() {
        let unit = render_unit(Path::new("/tmp/a b/%i/$HOME/\"mouse\"/daemon")).unwrap();
        assert!(unit.contains("ExecStart=\"/tmp/a b/%%i/$$HOME/\\\"mouse\\\"/daemon\" --kde"));
        assert!(unit.contains("WantedBy=graphical-session.target"));
        assert!(unit.contains("Type=dbus"));
        assert!(!unit.contains("--native-self-test"));
        assert!(render_unit(Path::new("relative/path")).is_err());
        assert!(render_unit(Path::new("/tmp/evil\nExecStart=other")).is_err());
    }
}
