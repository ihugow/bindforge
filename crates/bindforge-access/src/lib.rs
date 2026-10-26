//! Narrow, opt-in udev permissions. No input events, shell execution or caller-provided rules.
#![cfg(target_os = "linux")]
mod store;
mod device;
pub use device::{inspect, resolve, identities, Target};
pub use store::{apply_system, managed, revoked, ensure_not_revoked};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const HELPER: &str = "/usr/libexec/bindforge/bindforge-device-access";
pub const POLICY: &str = "/usr/share/polkit-1/actions/org.bindforge.device-access.policy";
pub const RULE_DIR: &str = "/etc/udev/rules.d";
const HEADER: &str = "# Managed by BindForge device access v1\n";
const UINPUT_FILE: &str = "72-bindforge-uinput.rules";
const UINPUT_RULE: &str = "# Managed by BindForge device access v1\n# Shared virtual output for explicitly authorized input devices.\nSUBSYSTEM==\"misc\", KERNEL==\"uinput\", TAG+=\"uaccess\"\n";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind { Mouse, Keyboard }
impl Kind {
    pub fn parse(value: &str) -> Result<Self, String> { match value { "mouse" => Ok(Self::Mouse), "keyboard" => Ok(Self::Keyboard), _ => Err("Tipo de dispositivo inválido.".into()) } }
    fn flag(self) -> &'static str { match self { Self::Mouse => "ID_INPUT_MOUSE", Self::Keyboard => "ID_INPUT_KEYBOARD" } }
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Action { Grant, Revoke }
impl Action {
    pub fn parse(value: &str) -> Result<Self, String> { match value { "grant" => Ok(Self::Grant), "revoke" => Ok(Self::Revoke), _ => Err("Ação de acesso inválida.".into()) } }
}
#[derive(Clone, Debug, Serialize)]
pub struct AccessStatus {
    pub fingerprint: String,
    pub device_name: String,
    pub model: String,
    pub readable: bool,
    pub output_writable: bool,
    pub managed: bool,
    pub revoked: bool,
    pub helper_available: bool,
    pub scope: String,
    pub message: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ApplyResult { pub reconnect_required: bool, pub message: String }

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Selector {
    kind: Kind,
    // Fixed property names, metadata-derived values. No executable expressions.
    matches: Vec<(String, String)>,
    scope: String,
}
fn safe_value(value: &str) -> Result<String, String> {
    if value.is_empty() || value.len() > 200 || !value.bytes().all(|c| c.is_ascii_alphanumeric() || b" ._:/@+-".contains(&c)) {
        return Err("O identificador deste dispositivo contém caracteres não suportados para uma regra específica.".into());
    }
    Ok(value.into())
}
impl Selector {
    fn render(&self) -> String {
        let mut fields = vec!["ACTION==\"add|change\"".into(), "SUBSYSTEM==\"input\"".into(), "KERNEL==\"event*\"".into(), format!("ENV{{{}}}==\"1\"", self.kind.flag())];
        fields.extend(self.matches.iter().map(|(key, value)| format!("{key}==\"{value}\"")));
        fields.push("TAG+=\"uaccess\"".into());
        format!("{HEADER}{}\n", fields.join(", "))
    }
    fn fingerprint(&self) -> String { format!("{:x}", Sha256::digest(self.render().as_bytes())) }
    fn revoked_filename(&self) -> String { format!("72-bindforge-access-{}.revoked", self.fingerprint()) }
    fn filename(&self) -> String { format!("72-bindforge-access-{}.rules", self.fingerprint()) }
}
