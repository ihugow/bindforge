//! Subscribe before reading the initial snapshot; all subsequent updates are signals.
use std::time::Duration;
use futures_lite::{future, StreamExt};
use zbus::{message::Type, MatchRule, MessageStream};
use super::{decode_history, DaemonStatus, WireHistory, SERVICE, PATH, STATUS_INTERFACE};

#[derive(Debug, Clone)]
pub enum WatchEvent {
    Status(DaemonStatus),
    Unavailable { restart_required: bool },
}

pub struct WatchCancel(async_channel::Sender<()>);
pub type WatchReceiver = async_channel::Receiver<()>;
impl Drop for WatchCancel {
    fn drop(&mut self) { let _ = self.0.try_send(()); }
}
pub fn watch_cancel() -> (WatchCancel, WatchReceiver) {
    let (sender, receiver) = async_channel::bounded(1);
    (WatchCancel(sender), receiver)
}

async fn owner(connection: &zbus::Connection) -> Result<Option<String>, String> {
    let proxy = zbus::fdo::DBusProxy::new(connection).await.map_err(|e| e.to_string())?;
    match proxy.get_name_owner(SERVICE.try_into().expect("valid name")).await {
        Ok(name) => Ok(Some(name.to_string())),
        Err(zbus::fdo::Error::NameHasNoOwner(_)) => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}

async fn initial(connection: &zbus::Connection, owner: &str) -> WatchEvent {
    let result = async {
        let proxy = zbus::Proxy::new(connection, owner, PATH, STATUS_INTERFACE).await?;
        let version: u32 = proxy.call("GetStatusWatchVersion", &()).await?;
        if version != 1 { return Ok(None); }
        proxy.call::<_, _, WireHistory>("GetStatusWithHistory", &()).await.map(Some)
    }.await;
    match result {
        Ok(Some(wire)) => match decode_history(wire) {
            Ok(state) => WatchEvent::Status(state),
            Err(_) => WatchEvent::Unavailable { restart_required: true },
        },
        Ok(None) => WatchEvent::Unavailable { restart_required: true },
        Err(zbus::Error::MethodError(name, _, _)) if name.as_str() == "org.freedesktop.DBus.Error.UnknownMethod" =>
            WatchEvent::Unavailable { restart_required: true },
        Err(_) => WatchEvent::Unavailable { restart_required: false },
    }
}

async fn connected(
    connection: zbus::Connection,
    cancelled: &WatchReceiver,
    publish: &mut impl FnMut(WatchEvent) -> bool,
) -> Result<(), String> {
    let status_rule = MatchRule::builder().msg_type(Type::Signal)
        .path(PATH).map_err(|e| e.to_string())?
        .interface(STATUS_INTERFACE).map_err(|e| e.to_string())?
        .member("StatusChanged").map_err(|e| e.to_string())?.build();
    let owner_rule = MatchRule::builder().msg_type(Type::Signal)
        .sender("org.freedesktop.DBus").map_err(|e| e.to_string())?
        .interface("org.freedesktop.DBus").map_err(|e| e.to_string())?
        .member("NameOwnerChanged").map_err(|e| e.to_string())?
        .add_arg(SERVICE).map_err(|e| e.to_string())?.build();
    // Both subscriptions precede the owner lookup and snapshot: no startup gap.
    let statuses = MessageStream::for_match_rule(status_rule, &connection, Some(32)).await.map_err(|e| e.to_string())?;
    let owners = MessageStream::for_match_rule(owner_rule, &connection, Some(8)).await.map_err(|e| e.to_string())?;
    let mut messages = futures_lite::stream::or(statuses, owners);
    let mut current_owner = owner(&connection).await?;
    let mut revision = None;
    let first = if let Some(name) = &current_owner { initial(&connection, name).await }
        else { WatchEvent::Unavailable { restart_required: false } };
    let mut supported = matches!(&first, WatchEvent::Status(_));
    if let WatchEvent::Status(state) = &first { revision = Some(state.revision); }
    if !publish(first) { return Ok(()); }
    loop {
        let message = future::race(async { Some(messages.next().await) }, async { let _ = cancelled.recv().await; None }).await;
        let Some(message) = message else { return Ok(()); };
        let message = message.ok_or("Session bus disconnected")?.map_err(|e| e.to_string())?;
        let header = message.header();
        if header.interface().is_some_and(|i| i.as_str() == "org.freedesktop.DBus") {
            let (_, _, new_owner): (String, String, String) = message.body().deserialize().map_err(|e| e.to_string())?;
            let new_owner = if new_owner.is_empty() { None } else { Some(new_owner) };
            if new_owner == current_owner { continue; }
            current_owner = new_owner;
            revision = None;
            supported = false;
            if !publish(WatchEvent::Unavailable { restart_required: false }) { return Ok(()); }
            if let Some(name) = &current_owner {
                let event = initial(&connection, name).await;
                supported = matches!(&event, WatchEvent::Status(_));
                if let WatchEvent::Status(state) = &event { revision = Some(state.revision); }
                if !publish(event) { return Ok(()); }
            }
        } else if supported && header.sender().map(|s| s.as_str()) == current_owner.as_deref() && current_owner.is_some() {
            // Signals from any other session process (including the old daemon) are ignored.
            let wire: WireHistory = message.body().deserialize().map_err(|e| e.to_string())?;
            let state = decode_history(wire)?;
            if revision.is_none_or(|previous| state.revision > previous) {
                revision = Some(state.revision);
                if !publish(WatchEvent::Status(state)) { return Ok(()); }
            }
        }
    }
}

async fn watch(address: Option<&str>, cancelled: WatchReceiver, mut publish: impl FnMut(WatchEvent) -> bool) {
    loop {
        let session = async {
            let builder = if let Some(address) = address { zbus::connection::Builder::address(address) }
                else { zbus::connection::Builder::session() }.map_err(|e| e.to_string())?;
            let connection = builder.method_timeout(Duration::from_secs(2)).build().await.map_err(|e| e.to_string())?;
            connected(connection, &cancelled, &mut publish).await
        };
        let outcome = future::race(async { Some(session.await) }, async { let _ = cancelled.recv().await; None }).await;
        match outcome {
            None | Some(Ok(())) => return,
            Some(Err(_)) => {
                if !publish(WatchEvent::Unavailable { restart_required: false }) { return; }
                // Only retry a broken bus connection. An absent daemon is handled
                // by NameOwnerChanged; there is no periodic status polling.
                let retry = future::race(async { async_io::Timer::after(Duration::from_secs(1)).await; true },
                    async { let _ = cancelled.recv().await; false }).await;
                if !retry { return; }
            }
        }
    }
}

pub async fn watch_session_status(cancelled: WatchReceiver, publish: impl FnMut(WatchEvent) -> bool) {
    watch(None, cancelled, publish).await;
}
/// Isolated-bus variant used by integration tests.
pub async fn watch_status_at(address: &str, cancelled: WatchReceiver, publish: impl FnMut(WatchEvent) -> bool) {
    watch(Some(address), cancelled, publish).await;
}
