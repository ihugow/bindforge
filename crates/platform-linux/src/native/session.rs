//! logind observer. A delay inhibitor is released only after stopping native input.
use std::{sync::{Arc, Condvar, Mutex}, thread, time::{Duration, Instant}};
use futures_lite::{future::block_on, StreamExt};
use zbus::{blocking::{connection::Builder, Connection, Proxy}, message::Type, zvariant::{OwnedFd, OwnedObjectPath}, MatchRule, MessageStream};
use super::{InputSession, SessionRestriction};

const SERVICE: &str = "org.freedesktop.login1";
const PATH: &str = "/org/freedesktop/login1";
const MANAGER: &str = "org.freedesktop.login1.Manager";
const SESSION: &str = "org.freedesktop.login1.Session";
type Stop = Arc<(Mutex<bool>, Condvar)>;

pub struct SessionGuard { stop: Stop, worker: Option<thread::JoinHandle<()>> }
impl SessionGuard {
    pub fn system(input: InputSession) -> Result<Self, String> { Self::start(input, None) }
    /// Explicit system-bus replacement for isolated tests; no session-bus overrides.
    pub fn at_address(input: InputSession, address: &str) -> Result<Self, String> { Self::start(input, Some(address.into())) }
    fn start(input: InputSession, address: Option<String>) -> Result<Self, String> {
        input.restrict_session(Some(SessionRestriction::Checking));
        let stop: Stop = Arc::new((Mutex::new(false), Condvar::new()));
        let stopping = stop.clone();
        let worker = thread::Builder::new().name("session-monitor".into()).spawn(move || {
            let mut last_error = None;
            while !stopped(&stopping) {
                if let Err(error) = observe(&input, &stopping, address.as_deref()) {
                    input.restrict_session(Some(SessionRestriction::Unavailable));
                    if last_error.as_ref() != Some(&error) { eprintln!("Session monitor unavailable: {error}"); }
                    last_error = Some(error);
                }
                wait(&stopping, Duration::from_secs(1));
            }
        }).map_err(|e| e.to_string())?;
        Ok(Self { stop, worker: Some(worker) })
    }
}
impl Drop for SessionGuard {
    fn drop(&mut self) {
        *self.stop.0.lock().unwrap_or_else(|e| e.into_inner()) = true;
        self.stop.1.notify_all();
        if let Some(worker) = self.worker.take() { let _ = worker.join(); }
    }
}
fn stopped(stop: &Stop) -> bool { *stop.0.lock().unwrap_or_else(|e| e.into_inner()) }
fn wait(stop: &Stop, duration: Duration) {
    let lock = stop.0.lock().unwrap_or_else(|e| e.into_inner());
    let _ = stop.1.wait_timeout_while(lock, duration, |v| !*v).unwrap_or_else(|e| e.into_inner());
}
// This guard also pauses on panic, before the inhibitor/connection are dropped.
struct PauseOnDrop(InputSession);
impl Drop for PauseOnDrop { fn drop(&mut self) { self.0.restrict_session(Some(SessionRestriction::Unavailable)); } }
fn proxy<'a>(connection: &'a Connection, owner: &'a str, path: &'a str, interface: &'a str) -> zbus::Result<Proxy<'a>> {
    zbus::blocking::proxy::Builder::new(connection).destination(owner)?.path(path)?.interface(interface)?
        .cache_properties(zbus::proxy::CacheProperties::No).build()
}
fn owner(connection: &Connection) -> zbus::Result<String> {
    Ok(zbus::blocking::fdo::DBusProxy::new(connection)?.get_name_owner(SERVICE.try_into().expect("valid name"))?.to_string())
}
fn inhibit(manager: &Proxy<'_>) -> zbus::Result<OwnedFd> {
    manager.call("Inhibit", &( "sleep", "BindForge", "Liberar as ações e a captura de mouse e teclado antes de suspender", "delay" ))
}
fn refresh(input: &InputSession, manager: &Proxy<'_>, session: &Proxy<'_>, sleeping: &mut bool) -> zbus::Result<()> {
    if owner(manager.connection())? != manager.destination().as_str() {
        return Err(zbus::Error::Failure("logind owner changed".into()));
    }
    // PreparingForSleep does not emit property changes; the signal clears this latch.
    *sleeping |= manager.get_property::<bool>("PreparingForSleep")?;
    let locked = session.get_property::<bool>("LockedHint")?;
    let active = session.get_property::<bool>("Active")?;
    input.restrict_session(if *sleeping { Some(SessionRestriction::Sleeping) }
        else if locked { Some(SessionRestriction::Locked) }
        else if !active { Some(SessionRestriction::Inactive) } else { None });
    Ok(())
}
fn observe(input: &InputSession, stop: &Stop, address: Option<&str>) -> Result<(), String> {
    let result = (|| -> zbus::Result<()> {
        let builder = match address { Some(address) => Builder::address(address)?, None => Builder::system()? };
        let connection = builder.method_timeout(Duration::from_secs(2)).build()?;
        let name = owner(&connection)?;
        let manager = proxy(&connection, &name, PATH, MANAGER)?;
        let path: OwnedObjectPath = manager.call("GetSessionByPID", &(std::process::id(),))
            .or_else(|_| manager.call("GetSession", &("auto",)))?;
        let session = proxy(&connection, &name, path.as_str(), SESSION)?;
        // Subscribe before reading the snapshot, pin the unique owner, and bound the queue.
        let rule = MatchRule::builder().msg_type(Type::Signal).sender(name.as_str())?.build();
        let mut signals = block_on(MessageStream::for_match_rule(rule, connection.inner(), Some(64)))?;
        let mut inhibitor = Some(inhibit(&manager)?);
        let _pause_before_inhibitor = PauseOnDrop(input.clone());
        let mut sleeping = false;
        refresh(input, &manager, &session, &mut sleeping)?;
        if sleeping { inhibitor.take(); }
        let mut health = Instant::now();
        while !stopped(stop) {
            if let Some(message) = block_on(futures_lite::future::poll_once(signals.next())) {
                let message = message.ok_or_else(|| zbus::Error::Failure("system bus disconnected".into()))??;
                let header = message.header();
                if header.sender().map(|s| s.as_str()) != Some(name.as_str()) { continue; }
                let is_sleep = header.path().map(|p| p.as_str()) == Some(PATH)
                    && header.interface().map(|v| v.as_str()) == Some(MANAGER)
                    && header.member().map(|v| v.as_str()) == Some("PrepareForSleep");
                if is_sleep {
                    let (preparing,): (bool,) = message.body().deserialize()?;
                    if preparing {
                        sleeping = true;
                        input.restrict_session(Some(SessionRestriction::Sleeping));
                        // restrict_session joins the input worker, emitting releases and ungrabbing.
                        inhibitor.take();
                    } else {
                        // Acquire the next delay lock before allowing activation again.
                        inhibitor = Some(inhibit(&manager)?);
                        sleeping = false;
                        refresh(input, &manager, &session, &mut sleeping)?;
                    }
                } else if header.path().map(|p| p.as_str()) == Some(path.as_str())
                    && header.interface().map(|v| v.as_str()) == Some("org.freedesktop.DBus.Properties")
                    && header.member().map(|v| v.as_str()) == Some("PropertiesChanged") {
                    let (interface, changed, _): (String, std::collections::HashMap<String, zbus::zvariant::OwnedValue>, Vec<String>) = message.body().deserialize()?;
                    if interface == SESSION {
                        // Preserve a brief lock/inactive transition even if the fresh snapshot
                        // already says unlocked. Pausing must never be coalesced away.
                        if changed.get("LockedHint").is_some_and(|v| bool::try_from(v).ok() == Some(true)) {
                            input.restrict_session(Some(SessionRestriction::Locked));
                        } else if changed.get("Active").is_some_and(|v| bool::try_from(v).ok() == Some(false)) {
                            input.restrict_session(Some(SessionRestriction::Inactive));
                        }
                        refresh(input, &manager, &session, &mut sleeping)?;
                    }
                }
                // Drain pending signals before waiting or running health checks.
                continue;
            }
            if health.elapsed() >= Duration::from_millis(500) {
                refresh(input, &manager, &session, &mut sleeping)?;
                if sleeping { inhibitor.take(); }
                health = Instant::now();
            }
            wait(stop, Duration::from_millis(10));
        }
        Ok(())
    })();
    result.map_err(|e| e.to_string())
}
