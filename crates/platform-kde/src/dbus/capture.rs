//! One short-lived, caller-owned application-ID capture. No titles or history.
use std::sync::{Arc, Mutex, atomic::{AtomicU64, Ordering}};
use std::time::{Duration, Instant};
use remap_core::WindowEvent;
use zbus::{fdo, message::Header};

const LIFETIME: Duration = Duration::from_secs(30);
struct Request {
    id: u64, caller: String, kwin: String, after: u64,
    baseline_app: Option<String>, result: Option<String>, deadline: Instant,
}
#[derive(Default)]
struct State {
    next: u64,
    latest: Option<(u64, String, Option<String>)>,
    request: Option<Request>,
}
#[derive(Clone, Default)]
pub(super) struct CaptureStore(Arc<Mutex<State>>);
impl CaptureStore {
    fn begin(&self, caller: &str, kwin: &str, after: u64) -> Result<u64, String> {
        let mut s = self.0.lock().map_err(|e| e.to_string())?;
        if s.request.as_ref().is_some_and(|r| Instant::now() < r.deadline) {
            return Err("Já existe uma captura em andamento. Cancele-a ou aguarde até 30 segundos.".into());
        }
        s.next = s.next.checked_add(1).ok_or("Limite de capturas atingido.")?;
        let id = s.next;
        let baseline_app = s.latest.as_ref().filter(|(_, owner, _)| owner == kwin).and_then(|(_, _, app)| app.clone());
        s.request = Some(Request { id, caller: caller.into(), kwin: kwin.into(), after,
            baseline_app, result: None, deadline: Instant::now() + LIFETIME });
        Ok(id)
    }
    fn read(&self, caller: &str, id: u64, kwin: Option<&str>) -> Result<(String, String), String> {
        let s = self.0.lock().map_err(|e| e.to_string())?;
        let r = s.request.as_ref().filter(|r| r.id == id && r.caller == caller).ok_or("Captura encerrada ou indisponível.")?;
        if Instant::now() >= r.deadline { return Err("Tempo esgotado. Inicie a captura novamente e selecione outra janela.".into()); }
        if kwin != Some(r.kwin.as_str()) { return Err("O KDE foi desconectado ou reiniciado. Inicie a captura novamente.".into()); }
        Ok(match &r.result { Some(app) => ("captured".into(), app.clone()), None => ("waiting".into(), String::new()) })
    }
    fn cancel(&self, caller: &str, id: u64) {
        let mut s = self.0.lock().unwrap_or_else(|e| e.into_inner());
        if s.request.as_ref().is_some_and(|r| r.id == id && r.caller == caller) { s.request = None; }
    }
    pub(super) fn observe(&self, sequence: u64, owner: &str, event: &WindowEvent) {
        let app = match event { WindowEvent::Focused(w) => w.app_id.clone(), WindowEvent::NoActiveWindow => None };
        let mut s = self.0.lock().unwrap_or_else(|e| e.into_inner());
        if s.latest.as_ref().is_some_and(|(last, _, _)| *last >= sequence) { return; }
        s.latest = Some((sequence, owner.into(), app.clone()));
        if let Some(r) = &mut s.request {
            if sequence > r.after && owner == r.kwin && Instant::now() < r.deadline && r.result.is_none() {
                if let Some(app) = app.filter(|a| !a.is_empty() && Some(a) != r.baseline_app.as_ref()) { r.result = Some(app); }
            }
        }
    }
}

pub(super) struct CaptureInterface { pub store: CaptureStore, pub sequence: Arc<AtomicU64> }
async fn owner(connection: &zbus::Connection) -> fdo::Result<Option<String>> {
    let bus = fdo::DBusProxy::new(connection).await?;
    match bus.get_name_owner("org.kde.KWin".try_into().expect("valid bus name")).await {
        Ok(owner) => Ok(Some(owner.to_string())),
        Err(fdo::Error::NameHasNoOwner(_)) => Ok(None), Err(e) => Err(e),
    }
}
fn caller<'a>(header: &'a Header<'_>) -> fdo::Result<&'a str> {
    header.sender().map(|s| s.as_str()).ok_or_else(|| fdo::Error::AccessDenied("missing sender".into()))
}
#[zbus::interface(name = "org.bindforge.Daemon.Capture1")]
impl CaptureInterface {
    async fn begin_capture(&self, #[zbus(header)] header: Header<'_>, #[zbus(connection)] connection: &zbus::Connection) -> fdo::Result<u64> {
        let kwin = owner(connection).await?.ok_or_else(|| fdo::Error::Failed("KDE indisponível. Ative a ponte de foco do KWin.".into()))?;
        self.store.begin(caller(&header)?, &kwin, self.sequence.load(Ordering::Relaxed)).map_err(fdo::Error::Failed)
    }
    async fn read_capture(&self, id: u64, #[zbus(header)] header: Header<'_>, #[zbus(connection)] connection: &zbus::Connection) -> fdo::Result<(String, String)> {
        self.store.read(caller(&header)?, id, owner(connection).await?.as_deref()).map_err(fdo::Error::Failed)
    }
    fn cancel_capture(&self, id: u64, #[zbus(header)] header: Header<'_>) -> fdo::Result<()> {
        self.store.cancel(caller(&header)?, id); Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use remap_core::ActiveWindow;
    fn focus(id: &str) -> WindowEvent { WindowEvent::Focused(ActiveWindow { app_id: Some(id.into()), ..Default::default() }) }
    #[test]
    fn captures_only_future_authenticated_different_app_and_never_replaces_result() {
        let store = CaptureStore::default();
        store.observe(1, "kwin", &focus("remap"));
        let id = store.begin("ui", "kwin", 2).unwrap();
        store.observe(2, "kwin", &focus("old"));
        store.observe(3, "kwin", &focus("remap"));
        store.observe(4, "kwin", &WindowEvent::NoActiveWindow);
        store.observe(5, "wrong-owner", &focus("spoofed"));
        assert_eq!(store.read("ui", id, Some("kwin")).unwrap().0, "waiting");
        store.observe(6, "kwin", &focus("firefox"));
        store.observe(7, "kwin", &focus("remap"));
        assert_eq!(store.read("ui", id, Some("kwin")).unwrap().1, "firefox");
        assert!(store.read("other", id, Some("kwin")).is_err());
        assert!(store.read("ui", id, Some("new-owner")).is_err());
        store.cancel("other", id);
        assert!(store.begin("other", "kwin", 7).is_err());
        store.cancel("ui", id);
        assert!(store.read("ui", id, Some("kwin")).is_err());
        assert!(store.begin("ui", "kwin", 7).is_ok());
    }
    #[test]
    fn timeout_releases_the_slot_and_late_events_are_ignored() {
        let store = CaptureStore::default(); let id = store.begin("ui", "kwin", 0).unwrap();
        store.0.lock().unwrap().request.as_mut().unwrap().deadline = Instant::now() - Duration::from_secs(1);
        store.observe(1, "kwin", &focus("firefox"));
        assert!(store.read("ui", id, Some("kwin")).is_err());
        assert!(store.begin("ui", "kwin", 1).unwrap() > id);
    }
}
