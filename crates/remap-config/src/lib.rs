//! Versioned user configuration shared by the desktop and daemon.
use std::{fs::{self, OpenOptions}, io::{Read, Write}, path::{Path, PathBuf}};
use remap_core::{demo_profiles, Action, InputControl, Profile, ProfileSet};
use serde::{Deserialize, Serialize};

const MAX_BYTES: u64 = 1024 * 1024;
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    #[serde(default)]
    pub devices: Vec<ConfiguredDevice>,
    pub version: u32,
    pub revision: u32,
    pub fallback_id: String,
    pub profiles: Vec<Profile>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConfiguredDevice {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub interface_id: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub connections: Vec<Connection>,
    pub button_count: Option<u8>,
    pub layout: Option<String>,
}
/// Explicitly associated transport, with an optional persistent udev identity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Connection {
    pub interface_id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interface_name: Option<String>,
    #[serde(default)]
    pub fingerprint: Option<String>,
}
impl ConfiguredDevice {
    pub fn binding_id(&self) -> String {
        if self.connections.is_empty() { self.interface_id.clone() } else { format!("device:{}", self.id) }
    }
}
impl Default for Config {
    fn default() -> Self {
        Self { devices: vec![], version: 1, revision: 0, fallback_id: "default".into(), profiles: demo_profiles().profiles().to_vec() }
    }
}
fn text(value: &str, max: usize) -> bool {
    !value.trim().is_empty() && value.len() <= max && !value.chars().any(char::is_control)
}
impl Config {
    pub fn validate(&self) -> Result<ProfileSet, String> {
        if self.version != 1 { return Err("Versão de configuração não suportada.".into()); }
        if self.profiles.is_empty() || self.profiles.len() > 256 { return Err("Use entre 1 e 256 perfis.".into()); }
        if self.devices.len() > 64 { return Err("Use no máximo 64 dispositivos configurados.".into()); }
        let mut ids = std::collections::HashSet::new();
        let mut interfaces = std::collections::HashSet::new();
        let mut fingerprints = std::collections::HashSet::new();
        for d in &self.devices {
            if !text(&d.id, 128) || !text(&d.name, 128) || !text(&d.interface_id, 4096)
                || !ids.insert(&d.id) || !interfaces.insert((&d.interface_id, &d.kind)) {
                return Err("Nome, ID ou interface do dispositivo inválido ou duplicado.".into());
            }
            if d.connections.len() > 8 { return Err("Use no máximo oito conexões por dispositivo.".into()); }
            let mut local = std::collections::HashSet::new();
            for c in &d.connections {
                if c.fingerprint.as_ref().is_some_and(|f| !fingerprints.insert((f, &d.kind))) { return Err("Esta conexão já pertence a um dispositivo.".into()); }
                if !text(&c.interface_id, 4096) || !text(&c.name, 128) || c.interface_name.as_ref().is_some_and(|name| !text(name, 1024)) || !local.insert(&c.interface_id)
                    || (c.interface_id != d.interface_id && !interfaces.insert((&c.interface_id, &d.kind)))
                    || c.fingerprint.as_ref().is_some_and(|f| f.len() != 64 || !f.bytes().all(|b| b.is_ascii_hexdigit())) {
                    return Err("Conexão inválida ou já associada a outro dispositivo.".into());
                }
            }
            if !d.connections.is_empty() && !local.contains(&d.interface_id) { return Err("A interface principal precisa pertencer às conexões.".into()); }
            let valid = match d.kind.as_str() {
                "mouse" => d.button_count.is_some_and(|n| (3..=32).contains(&n)) && d.layout.is_none(),
                "keyboard" => d.button_count.is_none() && d.layout.as_ref().is_some_and(|l| ["abnt2", "ansi", "iso"].contains(&l.as_str())),
                _ => false,
            };
            if !valid { return Err("Propriedades do dispositivo inválidas.".into()); }
        }
        for p in &self.profiles {
            if let Some(icon) = &p.icon {
                let payload = icon.strip_prefix("data:image/png;base64,");
                if !payload.is_some_and(|data| data.starts_with("iVBORw0KGgo") && data.len() <= 65536 && data.len() % 4 == 0
                    && data.bytes().all(|b| b.is_ascii_alphanumeric() || b"+/=".contains(&b))) {
                    return Err("Ícone do perfil inválido. Use uma imagem PNG de até 64 KiB em base64.".into());
                }
            }
            if !text(&p.id, 128) || !text(&p.name, 128) { return Err("Nome e ID do perfil devem ter até 128 bytes e não conter controles.".into()); }
            if p.id != self.fallback_id && p.rules.is_empty() { return Err("Associe pelo menos um aplicativo ao perfil.".into()); }
            if p.rules.len() > 256 || p.bindings.len() > 512 { return Err("Limite de regras ou ações excedido.".into()); }
            for rule in &p.rules {
                for value in [&rule.app_id, &rule.executable, &rule.title_contains].into_iter().flatten() {
                    if !text(value, 4096) { return Err("Identificador de aplicativo inválido.".into()); }
                }
            }
            for binding in &p.bindings {
                if binding.device_id.as_ref().is_some_and(|s| !text(s, 4096)) { return Err("Dispositivo inválido.".into()); }
                let valid_input = match &binding.input { InputControl::Key(k) => text(k, 128), InputControl::MouseButton(b) => *b > 0 };
                let valid_action = match &binding.action {
                    Action::KeyChord(keys) => !keys.is_empty() && keys.len() <= 16 && keys.iter().all(|k| text(k, 128)),
                    Action::MouseButton(b) => *b > 0, Action::Disabled => true,
                };
                if !valid_input || !valid_action { return Err("Controle ou ação inválida.".into()); }
            }
        }
        ProfileSet::new(self.profiles.clone(), &self.fallback_id)
    }
}
/// Linux uses XDG. Other targets have their conventional per-user config roots.
/// An explicit override is also useful for isolated tests; the UI cannot supply a path.
pub fn config_path() -> Result<PathBuf, String> {
    if let Some(path) = std::env::var_os("BINDFORGE_CONFIG").or_else(|| std::env::var_os("APP_BUTTONS_CONFIG")) { return Ok(PathBuf::from(path)); }
    let base = if cfg!(target_os = "windows") {
        std::env::var_os("APPDATA").map(PathBuf::from)
    } else if cfg!(target_os = "macos") {
        std::env::var_os("HOME").map(|p| PathBuf::from(p).join("Library/Application Support"))
    } else {
        std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from).filter(|p| p.is_absolute())
            .or_else(|| std::env::var_os("HOME").map(|p| PathBuf::from(p).join(".config")))
    }.ok_or("Não foi possível localizar a pasta de configurações do usuário.")?;
    branded_config_path(&base)
}
fn branded_config_path(base: &Path) -> Result<PathBuf, String> {
    let path = base.join("bindforge/profiles.json");
    let legacy = base.join("app-buttons-remap/profiles.json");
    if path.try_exists().map_err(|e| e.to_string())? || !legacy.try_exists().map_err(|e| e.to_string())? { return Ok(path); }
    // Validate before copying. Preserve the original and never replace a destination
    // another process has already published. A hard link publishes the complete file.
    let config = load(&legacy)?;
    let bytes = serde_json::to_vec_pretty(&config).map_err(|e| e.to_string())?;
    let parent = path.parent().expect("config parent");
    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let temporary = parent.join(format!(".migration-{}-{}.tmp", std::process::id(), NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)));
    let mut options = OpenOptions::new(); options.write(true).create_new(true);
    #[cfg(unix)] { use std::os::unix::fs::OpenOptionsExt; options.mode(0o600); }
    let mut file = options.open(&temporary).map_err(|e| e.to_string())?;
    let _cleanup = RemoveOnDrop(temporary.clone());
    file.write_all(&bytes).and_then(|_| file.sync_all()).map_err(|e| e.to_string())?;
    drop(file);
    match fs::hard_link(&temporary, &path) {
        Ok(()) => {}, Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {},
        Err(e) => return Err(format!("Não foi possível migrar os perfis para BindForge: {e}")),
    }
    Ok(path)
}
pub fn load(path: &Path) -> Result<Config, String> {
    let file = match fs::File::open(path) {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Config::default()),
        Err(e) => return Err(format!("Não foi possível ler os perfis: {e}")),
    };
    let mut bytes = Vec::new();
    file.take(MAX_BYTES + 1).read_to_end(&mut bytes).map_err(|e| e.to_string())?;
    if bytes.len() as u64 > MAX_BYTES { return Err("Arquivo de perfis excede 1 MiB.".into()); }
    let config: Config = serde_json::from_slice(&bytes).map_err(|e| format!("Arquivo de perfis inválido: {e}"))?;
    config.validate()?;
    Ok(config)
}
/// Portable backup uses the same versioned format as the saved configuration.
/// Parse and validate before allowing a restore; devices may be disconnected.
pub fn parse_backup(bytes: &[u8]) -> Result<Config, String> {
    if bytes.len() as u64 > MAX_BYTES { return Err("Backup excede 1 MiB.".into()); }
    let config: Config = serde_json::from_slice(bytes).map_err(|e| format!("Backup inválido: {e}"))?;
    config.validate()?;
    Ok(config)
}
pub fn export_backup(config: &Config) -> Result<Vec<u8>, String> {
    config.validate()?;
    let bytes = serde_json::to_vec_pretty(config).map_err(|e| e.to_string())?;
    if bytes.len() as u64 > MAX_BYTES { return Err("Backup excede 1 MiB.".into()); }
    Ok(bytes)
}
pub fn restore_backup(path: &Path, bytes: &[u8], expected_revision: u32) -> Result<Config, String> {
    // The backup's revision belongs to its original installation. save() assigns
    // a fresh local revision and checks that the preview is still current.
    save(path, parse_backup(bytes)?, expected_revision)
}
struct RemoveOnDrop(PathBuf);
impl Drop for RemoveOnDrop { fn drop(&mut self) { let _ = fs::remove_file(&self.0); } }
/// Serialize writers, reject stale editors, then atomically replace the file.
/// Existing invalid/future-version files are never overwritten by this operation.
pub fn save(path: &Path, mut config: Config, expected_revision: u32) -> Result<Config, String> {
    config.validate()?;
    let parent = path.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new("."));
    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let lock_path = path.with_extension("json.lock");
    let mut options = OpenOptions::new(); options.write(true).create_new(true);
    #[cfg(unix)] { use std::os::unix::fs::OpenOptionsExt; options.mode(0o600); }
    let _lock_file = options.open(&lock_path).map_err(|e| format!("Não foi possível bloquear o arquivo de perfis ({}, outra gravação pode estar em andamento): {e}", lock_path.display()))?;
    let _lock = RemoveOnDrop(lock_path);
    let current = load(path)?;
    if current.revision != expected_revision { return Err("Os perfis foram alterados em outra janela. Recarregue a lista antes de salvar.".into()); }
    config.revision = current.revision.checked_add(1).ok_or("Limite de revisões atingido.")?;
    let bytes = serde_json::to_vec_pretty(&config).map_err(|e| e.to_string())?;
    if bytes.len() as u64 > MAX_BYTES { return Err("Arquivo de perfis excede 1 MiB.".into()); }
    let temp_path = path.with_extension(format!("json.{}.tmp", std::process::id()));
    let mut file = options.open(&temp_path).map_err(|e| format!("Não foi possível criar o arquivo temporário: {e}"))?;
    let _temp = RemoveOnDrop(temp_path.clone());
    file.write_all(&bytes).and_then(|_| file.sync_all()).map_err(|e| e.to_string())?;
    drop(file);
    fs::rename(&temp_path, path).map_err(|e| format!("Não foi possível salvar os perfis: {e}"))?;
    Ok(config)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn backup_restores_complete_offline_config_with_a_local_revision() {
        let target = path();
        let mut original = Config::default();
        original.devices.push(ConfiguredDevice {
            id: "offline-mouse".into(), name: "Meu mouse".into(), kind: "mouse".into(),
            interface_id: "disconnected".into(), button_count: Some(5), layout: None,
            connections: vec![Connection { interface_id: "disconnected".into(), name: "Cabo".into(),
                interface_name: Some("Mouse USB".into()), fingerprint: Some("a".repeat(64)) }],
        });
        original.profiles[0].icon = Some("data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+a2ioAAAAASUVORK5CYII=".into());
        original.profiles[1].bindings[0].device_id = Some("device:offline-mouse".into());
        original.revision = 99;
        let bytes = export_backup(&original).unwrap();
        assert_eq!(parse_backup(&bytes).unwrap(), original);
        let restored = restore_backup(&target, &bytes, 0).unwrap();
        original.revision = 1;
        assert_eq!(restored, original);
        assert_eq!(load(&target).unwrap(), original);
        assert!(restore_backup(&target, &bytes, 0).is_err());
        let mut future = original.clone(); future.version = 2;
        let mut invalid_device = original.clone(); invalid_device.devices[0].button_count = Some(0);
        for invalid in [b"{broken".to_vec(), vec![b' '; MAX_BYTES as usize + 1],
            serde_json::to_vec(&future).unwrap(), serde_json::to_vec(&invalid_device).unwrap()] {
            assert!(restore_backup(&target, &invalid, 1).is_err());
            assert_eq!(load(&target).unwrap(), original);
        }
        fs::remove_dir_all(target.parent().unwrap()).unwrap();
    }
    #[test]
    fn profile_icons_roundtrip_and_legacy_profiles_keep_the_default() {
        let legacy = serde_json::to_value(Config::default()).unwrap();
        assert!(legacy["profiles"][0].get("icon").is_none());
        let mut config: Config = serde_json::from_value(legacy).unwrap();
        assert!(config.profiles[0].icon.is_none());
        let icon = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+a2ioAAAAASUVORK5CYII=";
        config.profiles[0].icon = Some(icon.into());
        let path = path();
        let saved = save(&path, config.clone(), 0).unwrap();
        assert_eq!(load(&path).unwrap(), saved);
        assert_eq!(saved.profiles[0].icon.as_deref(), Some(icon));
        for invalid in ["https://example.com/icon.png".to_string(), "data:image/svg+xml;base64,PHN2Zz4=".into(), format!("data:image/png;base64,iVBORw0KGgo{}", "A".repeat(65536))] {
            config.profiles[0].icon = Some(invalid);
            assert!(config.validate().is_err());
        }
        config.profiles[0].icon = None;
        let reset = save(&path, config, saved.revision).unwrap();
        assert!(load(&path).unwrap().profiles[0].icon.is_none());
        assert_eq!(reset.profiles[0].bindings, saved.profiles[0].bindings);
        fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }
    #[test]
    fn brand_migration_preserves_profiles_revisions_and_existing_files() {
        let root = path().parent().unwrap().to_path_buf();
        let legacy = root.join("app-buttons-remap/profiles.json");
        let saved = save(&legacy, Config::default(), 0).unwrap();
        let original = fs::read(&legacy).unwrap();
        let migrated = branded_config_path(&root).unwrap();
        assert_eq!(migrated, root.join("bindforge/profiles.json"));
        assert_eq!(load(&migrated).unwrap(), saved);
        assert_eq!(fs::read(&legacy).unwrap(), original);
        fs::write(&legacy, b"{broken").unwrap();
        assert_eq!(branded_config_path(&root).unwrap(), migrated);
        assert_eq!(load(&migrated).unwrap(), saved);
        fs::remove_file(&migrated).unwrap();
        assert!(branded_config_path(&root).is_err());
        assert!(!migrated.exists());
        fs::remove_dir_all(root).unwrap();
    }
    fn path() -> PathBuf {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        std::env::temp_dir().join(format!("remap-config-{}-{}", std::process::id(), NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed))).join("profiles.json")
    }
    #[test]
    fn device_registration_roundtrip_and_legacy_config() {
        let mut config = Config::default();
        let mut legacy = serde_json::to_value(&config).unwrap();
        legacy.as_object_mut().unwrap().remove("devices");
        assert!(serde_json::from_value::<Config>(legacy).unwrap().devices.is_empty());
        config.devices.push(ConfiguredDevice { id: "mouse-a".into(), name: "G PRO".into(), kind: "mouse".into(), interface_id: "interface-a".into(), connections: vec![], button_count: Some(5), layout: None });
        let path = path();
        let saved = save(&path, config.clone(), 0).unwrap();
        assert_eq!(load(&path).unwrap(), saved);
        config.devices.push(config.devices[0].clone());
        assert!(config.validate().is_err());
        config.devices.pop(); config.devices[0].button_count = Some(33);
        assert!(config.validate().is_err());
        config.devices[0].kind = "keyboard".into(); config.devices[0].button_count = None;
        config.devices[0].layout = Some("abnt2".into());
        assert!(config.validate().is_ok());
        config.devices[0].layout = Some("invalid".into());
        assert!(config.validate().is_err());
        fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }
    #[test]
    fn roundtrip_stale_writes_and_invalid_file_preservation() {
        let path = path();
        let mut config = load(&path).unwrap();
        assert!(!path.exists());
        config.profiles[1].name = "Navegador pessoal".into();
        let saved = save(&path, config.clone(), 0).unwrap();
        assert_eq!(load(&path).unwrap(), saved);
        assert!(save(&path, config, 0).is_err());
        fs::write(&path, b"{broken").unwrap();
        assert!(save(&path, Config::default(), 1).is_err());
        assert_eq!(fs::read(&path).unwrap(), b"{broken");
        fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }
    #[test]
    fn validates_versions_fallback_ids_and_actions() {
        let mut c = Config::default(); c.version = 2; assert!(c.validate().is_err());
        c = Config::default(); c.profiles[0].enabled = false; assert!(c.validate().is_err());
        c = Config::default(); c.profiles[1].id = "default".into(); assert!(c.validate().is_err());
        c = Config::default(); c.profiles[1].rules.clear(); assert!(c.validate().is_err());
        c = Config::default(); c.profiles[1].bindings[0].action = Action::KeyChord(vec![]); assert!(c.validate().is_err());
        c = Config::default(); c.profiles[1].rules[0].app_id = Some("x\ny".into()); assert!(c.validate().is_err());
    }
    #[test]
    fn failed_validation_keeps_previous_config() {
        let path = path(); let saved = save(&path, Config::default(), 0).unwrap();
        let mut invalid = saved.clone(); invalid.profiles[0].rules = invalid.profiles[1].rules.clone();
        assert!(save(&path, invalid, 1).is_err());
        assert_eq!(load(&path).unwrap(), saved);
        fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }
    #[test]
    fn multi_connection_identity_roundtrips_and_rejects_ambiguous_ownership() {
        let mut config = Config::default();
        let fingerprint = "a".repeat(64);
        config.devices.push(ConfiguredDevice { id: "mouse".into(), name: "Mouse".into(), kind: "mouse".into(),
            interface_id: "cable".into(), connections: vec![
                Connection { interface_id: "cable".into(), name: "Cabo".into(), interface_name: Some("Mouse USB de exemplo".into()), fingerprint: Some(fingerprint.clone()) },
                Connection { interface_id: "receiver".into(), name: "Receptor".into(), interface_name: None, fingerprint: None },
            ], button_count: Some(5), layout: None });
        assert_eq!(config.devices[0].binding_id(), "device:mouse");
        let path = path();
        let saved = save(&path, config.clone(), 0).unwrap();
        assert_eq!(load(&path).unwrap(), saved);
        let mut second = config.devices[0].clone();
        second.id = "second".into(); second.interface_id = "other".into();
        second.connections = vec![Connection { interface_id: "other".into(), name: "Outra".into(), interface_name: None, fingerprint: Some(fingerprint) }];
        config.devices.push(second);
        assert!(config.validate().is_err());
        config.devices.pop();
        let duplicate = config.devices[0].connections[0].clone();
        config.devices[0].connections.push(duplicate);
        assert!(config.validate().is_err());
        config.devices[0].connections.pop();
        config.devices[0].interface_id = "not-linked".into();
        assert!(config.validate().is_err());
        fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }

}
