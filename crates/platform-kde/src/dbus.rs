//! Session-bus observer only. This module never accesses input devices.
mod capture;
mod runtime;
use capture::{CaptureStore, CaptureInterface};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

use remap_core::{WindowEvent, WindowMonitor};
use zbus::blocking::{connection::Builder, Connection};
use zbus::{fdo, message::Header};

use crate::KwinFocus;

pub use remap_ipc::dbus::{SERVICE, PATH};
use remap_ipc::{StatusStore, dbus::StatusInterface};
pub const INTERFACE: &str = "org.bindforge.Daemon.Focus1";
const KWIN: &str = "org.kde.KWin";
const MAX_FIELD_BYTES: usize = 4096;
const HEALTH_INTERVAL: Duration = Duration::from_millis(500);

#[derive(Default)]
struct Slot {
    sequence: u64,
    pending: Option<(String, WindowEvent)>,
}
#[derive(Default)]
struct Mailbox {
    slot: Mutex<Slot>,
    ready: Condvar,
}
impl Mailbox {
    fn publish(&self, sequence: u64, sender: String, event: WindowEvent) {
        let mut slot = self.slot.lock().unwrap_or_else(|e| e.into_inner());
        // Authentication queries may complete out of order. Never resurrect old focus.
        if sequence > slot.sequence {
            slot.sequence = sequence;
            slot.pending = Some((sender, event));
            self.ready.notify_one();
        }
    }
    fn take(&self) -> Option<(String, WindowEvent)> {
        let slot = self.slot.lock().unwrap_or_else(|e| e.into_inner());
        let (mut slot, _) = self.ready.wait_timeout_while(slot, HEALTH_INTERVAL, |s| s.pending.is_none())
            .unwrap_or_else(|e| e.into_inner());
        slot.pending.take()
    }
}

struct FocusReceiver {
    mailbox: Arc<Mailbox>,
    sequence: Arc<std::sync::atomic::AtomicU64>,
    capture: CaptureStore,
    bridge: remap_ipc::dbus::BridgeStore,
}

#[zbus::interface(name = "org.bindforge.Daemon.Focus1")]
impl FocusReceiver {
    async fn focus_changed(
        &self,
        desktop_file_name: String,
        resource_class: String,
        #[zbus(header)] header: Header<'_>,
        #[zbus(connection)] connection: &zbus::Connection,
    ) -> fdo::Result<()> {
        self.receive(desktop_file_name, resource_class, 0, header, connection).await
    }
    async fn focus_changed_with_process(
        &self, desktop_file_name: String, resource_class: String, process_id: String,
        #[zbus(header)] header: Header<'_>, #[zbus(connection)] connection: &zbus::Connection,
    ) -> fdo::Result<()> {
        if process_id.is_empty() || process_id.len() > 10 || !process_id.bytes().all(|b| b.is_ascii_digit()) {
            return Err(fdo::Error::InvalidArgs("invalid process ID".into()));
        }
        let pid = process_id.parse::<u32>().map_err(|_| fdo::Error::InvalidArgs("invalid process ID".into()))?;
        self.receive(desktop_file_name, resource_class, pid, header, connection).await
    }
}
impl FocusReceiver {
    async fn receive(&self, desktop_file_name: String, resource_class: String, pid: u32, header: Header<'_>, connection: &zbus::Connection) -> fdo::Result<()> {
        let sequence = self.sequence.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
        let sender = header.sender().ok_or_else(|| fdo::Error::AccessDenied("missing sender".into()))?;
        let proxy = fdo::DBusProxy::new(connection).await?;
        let owner = proxy.get_name_owner(KWIN.try_into().expect("valid bus name")).await
            .map_err(|_| fdo::Error::AccessDenied("KWin is not available".into()))?;
        if sender.as_str() != owner.as_str() {
            return Err(fdo::Error::AccessDenied("only the current KWin owner may publish focus".into()));
        }
        for field in [&desktop_file_name, &resource_class] {
            if field.len() > MAX_FIELD_BYTES || field.chars().any(char::is_control) {
                return Err(fdo::Error::InvalidArgs("invalid focus metadata".into()));
            }
        }
        let mut event = KwinFocus { desktop_file_name, resource_class }.into_event();
        let raw_id = match &event { WindowEvent::Focused(window) => window.app_id.clone().unwrap_or_default(), WindowEvent::NoActiveWindow => String::new() };
        if (raw_id.is_empty() || raw_id.eq_ignore_ascii_case("gamescope")) && pid != 0 {
            let resolved = blocking::unblock(move || platform_linux::gamescope_window_identity(&raw_id, pid)).await;
            if let Some(identity) = resolved {
                event = WindowEvent::Focused(remap_core::ActiveWindow { app_id: Some(identity), ..Default::default() });
            }
        }
        self.bridge.update(|s| s.focus_received = true);
        self.capture.observe(sequence, owner.as_str(), &event);
        self.mailbox.publish(sequence, sender.to_string(), event);
        Ok(())
    }
}

/// Bounded latest-value mailbox: intermediate focus updates can be coalesced.
/// The bus is polled for owner changes while waiting, at most every 500 ms plus
/// bus round-trip time. Disconnects return an error so the daemon resets and exits.
pub struct KdeWindowMonitor {
    connection: Connection,
    mailbox: Arc<Mailbox>,
    owner: Option<String>,
    bridge: runtime::BridgeRuntime,
}
impl KdeWindowMonitor {
    pub fn session() -> Result<Self, String> {
        Self::session_with_status(StatusStore::default())
    }
    pub fn session_with_status(status: StatusStore) -> Result<Self, String> {
        Self::session_with_input(status, platform_linux::native::InputSession::default())
    }
    pub fn session_with_input(status: StatusStore, input: platform_linux::native::InputSession) -> Result<Self, String> {
        Self::build(Builder::session().map_err(|e| e.to_string())?, status, true, input)
    }
    /// Explicit bus address for isolated integration tests and diagnostics.
    /// Sender authentication is identical to the session-bus path.
    pub fn at_address(address: &str) -> Result<Self, String> {
        Self::build(Builder::address(address).map_err(|e| e.to_string())?, StatusStore::default(), false, platform_linux::native::InputSession::default())
    }
    fn build(builder: Builder<'_>, status: StatusStore, migrate: bool, input: platform_linux::native::InputSession) -> Result<Self, String> {
        let mailbox = Arc::new(Mailbox::default());
        let capture = CaptureStore::default();
        let sequence = Arc::new(std::sync::atomic::AtomicU64::new(0));
        let bridge_store = remap_ipc::dbus::BridgeStore::default();
        let receiver = FocusReceiver { bridge: bridge_store.clone(), mailbox: mailbox.clone(), sequence: sequence.clone(), capture: capture.clone() };
        let connection = builder
            .max_queued(64)
            .method_timeout(Duration::from_secs(2))
            .serve_at(PATH, platform_linux::dbus::DevicesInterface).map_err(|e| e.to_string())?
            .serve_at(PATH, CaptureInterface { store: capture, sequence }).map_err(|e| e.to_string())?
            .serve_at(PATH, StatusInterface(status)).map_err(|e| e.to_string())?
            .serve_at(PATH, remap_ipc::dbus::IntegrationInterface(bridge_store.clone())).map_err(|e| e.to_string())?
            .serve_at(PATH, platform_linux::native::InputInterface(input)).map_err(|e| e.to_string())?
            .serve_at(PATH, receiver).map_err(|e| e.to_string())?
            .build().map_err(|e| e.to_string())?;
        let bus = zbus::blocking::fdo::DBusProxy::new(&connection).map_err(|e| e.to_string())?;
        if bus.name_has_owner(remap_ipc::dbus::LEGACY_SERVICE.try_into().expect("valid name")).map_err(|e| e.to_string())? {
            return Err("Uma versão anterior do BindForge está em execução. Encerre-a antes de iniciar a nova versão.".into());
        }
        // zbus 5.19's builder does not check the RequestName reply for us.
        let claimed = connection.request_name_with_flags(SERVICE, fdo::RequestNameFlags::DoNotQueue.into());
        if !matches!(claimed, Ok(fdo::RequestNameReply::PrimaryOwner | fdo::RequestNameReply::AlreadyOwner)) {
            let _ = connection.close();
            return Err("D-Bus service name unavailable; another daemon may be running".into());
        }
        let owner = Self::kwin_owner(&connection)?;
        let bridge = runtime::BridgeRuntime::new(connection.clone(), bridge_store, migrate);
        Ok(Self { connection, mailbox, owner, bridge })
    }
    pub fn status_notifier(&self) -> impl Fn(&remap_ipc::DaemonStatus) -> Result<(), String> + 'static {
        let connection = self.connection.clone();
        move |state| remap_ipc::dbus::emit_status_changed(&connection, state)
    }
    fn kwin_owner(connection: &Connection) -> Result<Option<String>, String> {
        let proxy = zbus::blocking::fdo::DBusProxy::new(connection).map_err(|e| e.to_string())?;
        match proxy.get_name_owner(KWIN.try_into().expect("valid bus name")) {
            Ok(owner) => Ok(Some(owner.to_string())),
            Err(fdo::Error::NameHasNoOwner(_)) => Ok(None),
            Err(e) => Err(format!("session bus health check failed: {e}")),
        }
    }
}
impl KdeWindowMonitor {
    /// One bounded mailbox wait. None means idle, not end of stream.
    /// The daemon uses idle periods to reload saved configuration.
    pub fn poll_event(&mut self) -> Result<Option<WindowEvent>, String> {
        let owner = Self::kwin_owner(&self.connection)?;
        let mut bridge_lost = self.bridge.tick(owner.as_deref());
        let pending = self.mailbox.take();
        // Recheck after waiting: never deliver a pending event from a departed owner.
        let after_wait = Self::kwin_owner(&self.connection)?;
        if after_wait != owner { bridge_lost |= self.bridge.tick(after_wait.as_deref()); }
        let owner = after_wait;
        if owner != self.owner || bridge_lost {
            self.owner = owner.clone();
            if let Some((sender, event)) = pending {
                if owner.as_deref() == Some(sender.as_str()) {
                    let mut slot = self.mailbox.slot.lock().unwrap_or_else(|e| e.into_inner());
                    if slot.pending.is_none() { slot.pending = Some((sender, event)); }
                }
            }
            return Ok(Some(WindowEvent::NoActiveWindow));
        }
        if let Some((sender, event)) = pending {
            if owner.as_deref() == Some(sender.as_str()) { return Ok(Some(event)); }
        }
        Ok(None)
    }
}
impl WindowMonitor for KdeWindowMonitor {
    fn next_event(&mut self) -> Result<Option<WindowEvent>, String> {
        loop { if let Some(event) = self.poll_event()? { return Ok(Some(event)); } }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn coalesces_and_ignores_late_authentication() {
        let mailbox = Mailbox::default();
        let focused = KwinFocus { desktop_file_name: "firefox".into(), resource_class: "".into() }.into_event();
        mailbox.publish(2, ":1.2".into(), focused.clone());
        mailbox.publish(1, ":1.2".into(), WindowEvent::NoActiveWindow);
        assert_eq!(mailbox.take(), Some((":1.2".into(), focused)));
    }
}
