use std::{fs, io, os::fd::{AsFd, OwnedFd}, os::unix::fs::MetadataExt, path::{Path, PathBuf}, sync::{Arc, Mutex, atomic::{AtomicBool, Ordering}, mpsc}, thread, time::{Duration, Instant}};
use evdev::{raw_stream::RawDevice, uinput::VirtualDevice, AbsoluteAxisCode, AttributeSet, EventType, InputEvent, KeyCode, RelativeAxisCode, UinputAbsSetup};
use remap_core::{DeviceConnection, DeviceEnumerator, DeviceKind};
use super::{Engine, Plan, DeviceRegistration, NativeStatus, InputKind, CaptureGuard};
use super::engine::EmergencyChord;
const OUTPUT_PREFIX: &str = "BindForge";
const TEST_INPUT: &str = "BindForge Test Input";
const TEST_OUTPUT: &str = "BindForge Test Output";
fn error(context: &str, e: impl std::fmt::Display) -> String { format!("{context}: {e}") }
fn nonblocking(device: &RawDevice) -> Result<(), String> {
    let flags = rustix::fs::fcntl_getfl(device).map_err(|e| e.to_string())?;
    rustix::fs::fcntl_setfl(device, flags | rustix::fs::OFlags::NONBLOCK).map_err(|e| e.to_string())
}
fn open_registered(id: &str, kind: InputKind) -> Result<RawDevice, String> {
    let inventory = crate::LinuxDeviceEnumerator.enumerate()?;
    let metadata = inventory.into_iter().find(|d| d.id == id).ok_or("Interface não encontrada. Atualize os dispositivos e refaça o vínculo se necessário.")?;
    let compatible = match kind {
        InputKind::Mouse => physical_mouse(metadata.kind, metadata.connection, &metadata.name),
        InputKind::Keyboard => physical_keyboard(metadata.kind, metadata.connection, &metadata.name),
    };
    if !compatible { return Err(format!("Selecione uma interface física de {} compatível; saídas virtuais não são capturadas.", kind.name())); }
    let syspath = id.strip_prefix("linux:").ok_or("Identificador de interface incompatível.")?;
    let udev = udev::Device::from_syspath(Path::new(syspath)).map_err(|e| e.to_string())?;
    let path = udev.devnode().ok_or("A interface não tem um nó evdev.")?;
    let device = RawDevice::open(path).map_err(|e| error("Não foi possível abrir o dispositivo. Verifique o acesso específico à interface", e))?;
    let fd = device.as_fd().try_clone_to_owned().map_err(|e| e.to_string())?;
    let rdev = fs::File::from(fd).metadata().map_err(|e| e.to_string())?.rdev();
    let current = udev::Device::from_syspath(Path::new(syspath)).map_err(|e| e.to_string())?;
    if current.devnum() != Some(rdev) || udev.devnum() != Some(rdev) { return Err("O dispositivo mudou durante a abertura. Atualize a lista.".into()); }
    validate_source(&device, kind)?; nonblocking(&device)?; Ok(device)
}
fn physical_mouse(kind: DeviceKind, connection: DeviceConnection, name: &str) -> bool {
    matches!(kind, DeviceKind::Mouse | DeviceKind::Composite)
        && connection != DeviceConnection::Virtual && !name.starts_with("BindForge") && !name.starts_with("App Buttons Remap")
}
fn physical_keyboard(kind: DeviceKind, connection: DeviceConnection, name: &str) -> bool {
    matches!(kind, DeviceKind::Keyboard | DeviceKind::Composite)
        && connection != DeviceConnection::Virtual && !name.starts_with("BindForge") && !name.starts_with("App Buttons Remap")
}
fn validate_keyboard(keys: &[KeyCode], absolute: &[AbsoluteAxisCode], events: &[EventType]) -> Result<(), String> {
    if ![KeyCode::KEY_A, KeyCode::KEY_Z, KeyCode::KEY_ENTER, KeyCode::KEY_SPACE].iter().all(|key| keys.contains(key)) {
        return Err("Esta interface não é um teclado alfanumérico completo. Escolha a interface principal do teclado.".into());
    }
    if absolute.iter().any(|axis| *axis != AbsoluteAxisCode::ABS_VOLUME) || events.iter().any(|t| ![
        EventType::SYNCHRONIZATION, EventType::KEY, EventType::RELATIVE, EventType::MISC,
        EventType::ABSOLUTE, EventType::LED, EventType::REPEAT,
    ].contains(t)) {
        return Err("O teclado possui recursos de entrada ainda não suportados; a captura foi recusada para preservar seu funcionamento.".into());
    }
    Ok(())
}
fn validate_capabilities(keys: &[KeyCode], relative: &[RelativeAxisCode], absolute: &[AbsoluteAxisCode], events: &[EventType]) -> Result<(), String> {
    if !keys.contains(&KeyCode::BTN_LEFT) || !relative.contains(&RelativeAxisCode::REL_X) || !relative.contains(&RelativeAxisCode::REL_Y) {
        return Err("Esta interface não possui os botões e eixos de um mouse relativo.".into());
    }
    // HID++ mice advertise keyboard/media controls alongside their pointer. LED and REP
    // describe host feedback/configuration, not additional physical input controls.
    if absolute.iter().any(|axis| *axis != AbsoluteAxisCode::ABS_VOLUME) || events.iter().any(|t| ![
        EventType::SYNCHRONIZATION, EventType::KEY, EventType::RELATIVE, EventType::MISC,
        EventType::ABSOLUTE, EventType::LED, EventType::REPEAT,
    ].contains(t)) {
        return Err("Esta interface possui recursos de entrada ainda não suportados; a captura foi recusada para preservar seu funcionamento.".into());
    }
    Ok(())
}
fn validate_source(device: &RawDevice, kind: InputKind) -> Result<(), String> {
    if device.name().is_some_and(|n| n.starts_with(OUTPUT_PREFIX)) {
        return Err("A saída virtual do app não pode ser capturada.".into());
    }
    if kind == InputKind::Keyboard {
        return validate_keyboard(
            &device.supported_keys().map(|v| v.iter().collect::<Vec<_>>()).unwrap_or_default(),
            &device.supported_absolute_axes().map(|v| v.iter().collect::<Vec<_>>()).unwrap_or_default(),
            &device.supported_events().iter().collect::<Vec<_>>(),
        );
    }
    validate_capabilities(
        &device.supported_keys().map(|v| v.iter().collect::<Vec<_>>()).unwrap_or_default(),
        &device.supported_relative_axes().map(|v| v.iter().collect::<Vec<_>>()).unwrap_or_default(),
        &device.supported_absolute_axes().map(|v| v.iter().collect::<Vec<_>>()).unwrap_or_default(),
        &device.supported_events().iter().collect::<Vec<_>>(),
    )
}
fn virtual_output(source: &RawDevice, name: &str) -> Result<VirtualDevice, String> {
    let mut keys = AttributeSet::<KeyCode>::new();
    // Advertise all supported keyboard targets before creating the device, allowing profile changes.
    for code in 1..256 { keys.insert(KeyCode(code)); }
    for button in [1, 2, 3, 8, 9, 10, 11, 12] { keys.insert(KeyCode(super::engine::mouse_code(button)?)); }
    if let Some(source_keys) = source.supported_keys() { for k in source_keys.iter() { keys.insert(k); } }
    let mut builder = VirtualDevice::builder().map_err(|e| error("Não foi possível acessar /dev/uinput", e))?.name(name)
        .with_keys(&keys).map_err(|e| e.to_string())?;
    // Mouse-button destinations from a keyboard need a pointer-capable output too.
    let mut axes = AttributeSet::<RelativeAxisCode>::new();
    if let Some(source_axes) = source.supported_relative_axes() { for axis in source_axes.iter() { axes.insert(axis); } }
    axes.insert(RelativeAxisCode::REL_X); axes.insert(RelativeAxisCode::REL_Y);
    builder = builder.with_relative_axes(&axes).map_err(|e| e.to_string())?;
    if let Some(misc) = source.misc_properties() { builder = builder.with_msc(misc).map_err(|e| e.to_string())?; }
    if source.supported_absolute_axes().is_some() {
        for (axis, info) in source.get_absinfo().map_err(|e| error("Não foi possível ler os eixos absolutos", e))? {
            builder = builder.with_absolute_axis(&UinputAbsSetup::new(axis, info)).map_err(|e| e.to_string())?;
        }
    }
    builder.build().map_err(|e| error("Não foi possível criar a saída virtual", e))
}
fn emit(output: &mut VirtualDevice, events: &[InputEvent]) -> Result<(), String> {
    if !events.is_empty() { output.emit(events).map_err(|e| error("Falha na saída virtual; remapeamento interrompido", e))?; } Ok(())
}
// A shared eventfd wakes poll for control changes, even when the input is idle.
struct Wake(OwnedFd);
impl Wake {
    fn new() -> Result<Self, String> {
        rustix::event::eventfd(0, rustix::event::EventfdFlags::CLOEXEC | rustix::event::EventfdFlags::NONBLOCK)
            .map(Self).map_err(|e| e.to_string())
    }
    fn notify(&self) -> Result<(), String> {
        loop {
            match rustix::io::write(&self.0, &1u64.to_ne_bytes()) {
                Ok(_) | Err(rustix::io::Errno::AGAIN) => return Ok(()),
                Err(rustix::io::Errno::INTR) => continue,
                Err(e) => return Err(e.to_string()),
            }
        }
    }
    fn wait(&self, source: &impl AsFd, timeout: Duration) -> Result<(), String> {
        use rustix::event::{poll, PollFd, PollFlags, Timespec};
        let mut fds = [PollFd::new(source, PollFlags::IN), PollFd::new(&self.0, PollFlags::IN)];
        let timeout = Timespec::try_from(timeout).map_err(|e| e.to_string())?;
        match poll(&mut fds, Some(&timeout)) {
            Ok(_) => {},
            Err(rustix::io::Errno::INTR) => return Ok(()),
            Err(e) => return Err(error("Falha aguardando entrada", e)),
        }
        if fds.iter().any(|fd| fd.revents().intersects(PollFlags::NVAL)) {
            return Err("Descritor de entrada inválido.".into());
        }
        if fds[1].revents().contains(PollFlags::IN) {
            let mut bytes = [0u8; 8];
            loop {
                match rustix::io::read(&self.0, &mut bytes) {
                    Ok(_) | Err(rustix::io::Errno::AGAIN) => break,
                    Err(rustix::io::Errno::INTR) => continue,
                    Err(e) => return Err(e.to_string()),
                }
            }
        }
        Ok(())
    }
}
enum Control { Apply(Plan, mpsc::SyncSender<Result<(),String>>) }
pub(super) struct Worker {
    device: DeviceRegistration, control: mpsc::SyncSender<Control>, stopping: Arc<AtomicBool>,
    disconnected: Arc<AtomicBool>, wake: Arc<Wake>, health: Arc<Mutex<NativeStatus>>, thread: Option<thread::JoinHandle<()>>,
}
impl Worker {
    pub fn start(device: DeviceRegistration, plan: Plan, guard: CaptureGuard) -> Result<Self,String> {
        guard()?;
        let mut source = open_registered(&device.interface_id, device.kind)?;
        let output_name = match device.kind {
            InputKind::Mouse => "BindForge Mouse Output",
            InputKind::Keyboard => "BindForge Keyboard Output",
        };
        let mut output = virtual_output(&source, output_name)?;
        let output_node = node(&mut output)?;
        wait_initialized(&output_node)?;
        source.grab().map_err(|e| error("Não foi possível capturar exclusivamente o dispositivo. Pause outros remapeadores para essa interface", e))?;
        // Discard events from before acquisition, and require all buttons released.
        match source.fetch_events() { Ok(events) => { let _ = events.count(); }, Err(e) if e.kind() == io::ErrorKind::WouldBlock => {}, Err(e) => return Err(e.to_string()) }
        if source.get_key_state().map_err(|e| e.to_string())?.iter().next().is_some() { return Err("Solte todas as teclas e botões do dispositivo e tente ativar novamente.".into()); }
        guard()?;
        let (control, commands) = mpsc::sync_channel(2); let stopping = Arc::new(AtomicBool::new(false));
        let health = Arc::new(Mutex::new(NativeStatus { active: true, device_id: device.id.clone(), message: String::new() }));
        let stop = stopping.clone(); let state = health.clone();
        let disconnected = Arc::new(AtomicBool::new(false)); let removed = disconnected.clone();
        let kind = device.kind;
        let wake = Arc::new(Wake::new()?); let worker_wake = wake.clone();
        let thread = thread::Builder::new().name(format!("{}-remapper", kind.name())).spawn(move || {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| pump(&mut source, output, plan, &commands, &stop, kind, guard.as_ref(), &worker_wake, &removed)))
                .unwrap_or_else(|_| { removed.store(false, Ordering::Relaxed); Err("A tarefa de entrada falhou e foi encerrada.".into()) });
            // Closing RawDevice releases EVIOCGRAB even on errors/panic.
            drop(source);
            let mut state = state.lock().unwrap_or_else(|e| e.into_inner()); state.active = false;
            state.message = result.err().unwrap_or_default();
        }).map_err(|e| e.to_string())?;
        Ok(Self { device, control, stopping, disconnected, wake, health, thread: Some(thread) })
    }
    #[cfg(test)]
    pub(super) fn fixture(device: DeviceRegistration, applied: Arc<Mutex<Vec<Plan>>>, fail: bool) -> Self {
        let (control, commands) = mpsc::sync_channel(2);
        let stopping = Arc::new(AtomicBool::new(false)); let stop = stopping.clone();
        let health = Arc::new(Mutex::new(NativeStatus { active: true, device_id: device.id.clone(), message: String::new() }));
        let state = health.clone();
        let wake = Arc::new(Wake::new().unwrap());
        let thread = thread::spawn(move || {
            while !stop.load(Ordering::Relaxed) {
                if let Ok(Control::Apply(plan, ack)) = commands.recv_timeout(Duration::from_millis(2)) {
                    let result = if fail { Err("fixture I/O failure".into()) } else { applied.lock().unwrap().push(plan); Ok(()) };
                    let _ = ack.send(result);
                }
            }
            state.lock().unwrap().active = false;
        });
        Self { device, control, stopping, disconnected: Arc::new(AtomicBool::new(false)), wake, health, thread: Some(thread) }
    }
    #[cfg(test)]
    pub(super) fn fail_for_test(&self, message: &str, disconnected: bool) {
        self.disconnected.store(disconnected, Ordering::Relaxed);
        let mut status = self.health.lock().unwrap(); status.active = false; status.message = message.into();
    }
    pub fn disconnected(&self) -> bool { self.disconnected.load(Ordering::Relaxed) }
    pub fn binding_id(&self) -> &str { &self.device.binding_id }
    pub fn interface_id(&self) -> &str { &self.device.interface_id }
    pub fn status(&self) -> NativeStatus { self.health.lock().unwrap_or_else(|e| e.into_inner()).clone() }
    pub fn apply(&self, plan: Plan) -> Result<(),String> {
        let (send, recv) = mpsc::sync_channel(1);
        self.control.send(Control::Apply(plan, send)).map_err(|_| "O leitor do dispositivo encerrou. Ative novamente.".to_string())?;
        self.wake.notify()?;
        recv.recv().map_err(|_| "O leitor do dispositivo encerrou durante a troca de perfil.".to_string())?
    }
    pub fn stop(mut self) { self.stop_inner(); }
    fn stop_inner(&mut self) {
        self.stopping.store(true, Ordering::Relaxed);
        let _ = self.wake.notify();
        if let Some(handle) = self.thread.take() { let _ = handle.join(); }
    }
}
impl Drop for Worker { fn drop(&mut self) { self.stop_inner(); } }
struct CaptureAuthorization<'a> {
    guard: &'a (dyn Fn() -> Result<(), String> + Send + Sync),
    last_check: Option<Instant>,
}
impl CaptureAuthorization<'_> {
    fn remaining(&self) -> Duration {
        self.last_check.map_or(Duration::ZERO, |at| Duration::from_millis(100).saturating_sub(at.elapsed()))
    }
    fn check(&mut self) -> Result<(), String> {
        if self.last_check.is_none_or(|at| at.elapsed() >= Duration::from_millis(100)) {
            (self.guard)()?; self.last_check = Some(Instant::now());
        }
        Ok(())
    }
}
fn pump(source: &mut RawDevice, mut output: VirtualDevice, plan: Plan, commands: &mpsc::Receiver<Control>, stop: &AtomicBool, kind: InputKind, guard: &(dyn Fn() -> Result<(), String> + Send + Sync), wake: &Wake, disconnected: &AtomicBool) -> Result<(),String> {
    let mut engine = Engine::new(plan); let mut frame = Vec::with_capacity(32); let mut out = Vec::with_capacity(32); let mut emergency = EmergencyChord::default();
    let result = (|| {
        let input_fd = source.as_fd().try_clone_to_owned().map_err(|e| e.to_string())?;
        let mut authorization = CaptureAuthorization { guard, last_check: None };
        while !stop.load(Ordering::Relaxed) {
            authorization.check()?;
            while let Ok(Control::Apply(plan, ack)) = commands.try_recv() {
                let result = emit(&mut output, &engine.replace(plan));
                let _ = ack.send(result.clone()); result?;
            }
            let mut events = match source.fetch_events() {
                Ok(events) => events.peekable(), Err(e) if e.kind() == io::ErrorKind::WouldBlock => { wake.wait(&input_fd, authorization.remaining())?; continue; },
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                Err(e) if [rustix::io::Errno::NODEV.raw_os_error(), rustix::io::Errno::NXIO.raw_os_error()].into_iter().any(|code| e.raw_os_error() == Some(code)) => {
                    disconnected.store(true, Ordering::Relaxed);
                    return Err(error("Dispositivo desconectado ou leitura interrompida", e));
                },
                Err(e) => return Err(error("Falha lendo o dispositivo; ative novamente", e)),
            };
            if events.peek().is_none() { disconnected.store(true, Ordering::Relaxed); return Err("O dispositivo encerrou o fluxo de eventos.".into()); }
            for event in events {
                if event.event_type() == EventType::SYNCHRONIZATION {
                    if event.code() == 3 { return Err("O fluxo do dispositivo perdeu eventos. Remapeamento pausado; ative novamente.".into()); }
                    if event.code() == 0 {
                        out.clear();
                        for input in frame.drain(..) {
                            if kind == InputKind::Keyboard && emergency.event(input) {
                                return Err("Teclado pausado pelo atalho físico de emergência Ctrl + Alt + Pause.".into());
                            }
                            engine.event_into(input, &mut out);
                        }
                        emit(&mut output, &out)?;
                    }
                } else if ![EventType::LED, EventType::REPEAT].contains(&event.event_type()) {
                    // Host LED/repeat configuration is not mirrored to the virtual pointer.
                    // Actual key repeat is EV_KEY value 2 and remains in the input stream.
                    if frame.len() >= 4096 { return Err("Quadro de entrada excedeu o limite. Remapeamento pausado.".into()); }
                    frame.push(event);
                }
            }
        }
        Ok(())
    })();
    let release = emit(&mut output, &engine.release_all());
    if release.is_err() { disconnected.store(false, Ordering::Relaxed); }
    // Destroy synthetic state before returning to original physical behavior.
    drop(output);
    let ungrab = source.ungrab().map_err(|e| e.to_string());
    result.and(release).and(ungrab)
}

fn node(device: &mut VirtualDevice) -> Result<PathBuf,String> {
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        if let Some(path) = device.enumerate_dev_nodes_blocking().map_err(|e| e.to_string())?.next() {
            let path = path.map_err(|e| e.to_string())?; if path.exists() { return Ok(path); }
        }
        if Instant::now() >= deadline { return Err("O nó do dispositivo virtual não apareceu.".into()); }
        thread::sleep(Duration::from_millis(20));
    }
}
fn wait_initialized(path: &Path) -> Result<(),String> {
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        if let Ok(meta) = fs::metadata(path) {
            if udev::Device::from_devnum(udev::DeviceType::Character, meta.rdev()).is_ok_and(|d| d.is_initialized()) { return Ok(()); }
        }
        if Instant::now() >= deadline { return Err("O dispositivo virtual não foi inicializado pelo udev.".into()); }
        thread::sleep(Duration::from_millis(20));
    }
}
fn read_keys(reader: &mut RawDevice, count: usize) -> Result<Vec<(u16,i32)>,String> {
    let deadline = Instant::now()+Duration::from_secs(3); let mut got = vec![];
    while Instant::now() < deadline {
        match reader.fetch_events() { Ok(events) => got.extend(events.filter(|e| e.event_type()==EventType::KEY).map(|e|(e.code(),e.value()))), Err(e) if e.kind()==io::ErrorKind::WouldBlock => {}, Err(e)=>return Err(e.to_string()) }
        if got.len() >= count { return Ok(got); } thread::sleep(Duration::from_millis(2));
    }
    Err(format!("Tempo esgotado lendo saída virtual: {got:?}"))
}
/// Opt-in kernel harness. BOTH fixtures are exclusively grabbed BEFORE any emission.
/// Observer handles verify that no test events escape to other evdev consumers.
pub fn isolated_self_test() -> Result<(),String> { isolated_test(InputKind::Mouse) }
pub fn isolated_keyboard_self_test() -> Result<(),String> { isolated_test(InputKind::Keyboard) }
fn isolated_test(kind: InputKind) -> Result<(),String> {
    let mut keys = AttributeSet::<KeyCode>::new(); for code in [KeyCode::BTN_LEFT,KeyCode::BTN_RIGHT,KeyCode::BTN_MIDDLE,KeyCode::BTN_SIDE,KeyCode::BTN_EXTRA] { keys.insert(code); }
    if kind == InputKind::Keyboard {
        for code in [KeyCode::KEY_A, KeyCode::KEY_Z, KeyCode::KEY_ENTER, KeyCode::KEY_SPACE, KeyCode::KEY_CAPSLOCK] { keys.insert(code); }
    }
    let mut axes = AttributeSet::<RelativeAxisCode>::new(); axes.insert(RelativeAxisCode::REL_X); axes.insert(RelativeAxisCode::REL_Y);
    let mut input = VirtualDevice::builder().map_err(|e| e.to_string())?.name(TEST_INPUT).with_keys(&keys).map_err(|e| e.to_string())?.with_relative_axes(&axes).map_err(|e| e.to_string())?.build().map_err(|e| e.to_string())?;
    let input_node = node(&mut input)?; wait_initialized(&input_node)?;
    let mut source = RawDevice::open(&input_node).map_err(|e| error(&format!("Sem acesso de leitura ao nó virtual de teste {}. Nenhum evento emitido", input_node.display()),e))?; nonblocking(&source)?;
    let mut output = virtual_output(&source, TEST_OUTPUT)?; let output_node = node(&mut output)?; wait_initialized(&output_node)?;
    let mut reader = RawDevice::open(&output_node).map_err(|e| error(&format!("Sem acesso de leitura à saída virtual de teste {}. Nenhum evento emitido", output_node.display()),e))?; nonblocking(&reader)?;
    let mut input_observer = RawDevice::open(&input_node).map_err(|e| e.to_string())?; nonblocking(&input_observer)?;
    let mut output_observer = RawDevice::open(&output_node).map_err(|e| e.to_string())?; nonblocking(&output_observer)?;
    source.grab().map_err(|e| error("Não foi possível isolar a entrada de teste; nenhum evento emitido",e))?;
    reader.grab().map_err(|e| error("Não foi possível isolar a saída de teste; nenhum evento emitido",e))?;
    if !source.is_grabbed() || !reader.is_grabbed() { return Err("O isolamento dos dispositivos de teste falhou; nenhum evento emitido.".into()); }
    let source_code = if kind == InputKind::Mouse { KeyCode::BTN_SIDE.0 } else { KeyCode::KEY_CAPSLOCK.0 };
    let plan = Plan(std::collections::BTreeMap::from([(source_code,vec![KeyCode::KEY_V.0])]));
    let (send, commands) = mpsc::sync_channel(2); let stop = Arc::new(AtomicBool::new(false)); let worker_stop = stop.clone();
    let wake = Arc::new(Wake::new()?); let worker_wake = wake.clone();
    let worker = thread::spawn(move || pump(&mut source, output, plan, &commands, &worker_stop, kind, &|| Ok(()), &worker_wake, &AtomicBool::new(false)));
    let result = (|| {
        let button = |value| InputEvent::new(EventType::KEY.0,source_code,value);
        input.emit(&[button(1),button(0)]).map_err(|e| e.to_string())?;
        if read_keys(&mut reader,2)? != vec![(KeyCode::KEY_V.0,1),(KeyCode::KEY_V.0,0)] { return Err("A ação V não foi emitida corretamente.".into()); }
        input.emit(&[button(1)]).map_err(|e| e.to_string())?;
        if read_keys(&mut reader,1)? != vec![(KeyCode::KEY_V.0,1)] { return Err("O pressionamento não foi mantido.".into()); }
        if kind == InputKind::Keyboard {
            input.emit(&[button(2)]).map_err(|e| e.to_string())?;
            if read_keys(&mut reader, 1)? != vec![(KeyCode::KEY_V.0, 2)] { return Err("A repetição da tecla remapeada falhou.".into()); }
        }
        let (ack, recv) = mpsc::sync_channel(1);
        send.send(Control::Apply(Plan(std::collections::BTreeMap::from([(source_code,vec![KeyCode::KEY_C.0])])),ack)).map_err(|e| e.to_string())?;
        wake.notify()?;
        recv.recv_timeout(Duration::from_secs(3)).map_err(|e| e.to_string())??;
        if read_keys(&mut reader,1)? != vec![(KeyCode::KEY_V.0,0)] { return Err("A troca de perfil não liberou a tecla anterior.".into()); }
        input.emit(&[button(0),button(1),button(0)]).map_err(|e| e.to_string())?;
        if read_keys(&mut reader,2)? != vec![(KeyCode::KEY_C.0,1),(KeyCode::KEY_C.0,0)] { return Err("O novo perfil não foi aplicado corretamente.".into()); }
        for observer in [&mut input_observer, &mut output_observer] {
            match observer.fetch_events() {
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {},
                Ok(events) => { if events.count() != 0 { return Err("Um evento escapou dos leitores exclusivos de teste.".into()); } },
                Err(e) => return Err(e.to_string()),
            }
        }
        Ok(())
    })();
    // Destroy input while still grabbed, even on errors with a held test button.
    // Keep the output reader grabbed until the worker destroys its uinput output.
    drop(input); stop.store(true,Ordering::Relaxed); let _ = wake.notify(); drop(send);
    let cleanup = worker.join().map_err(|_| "Falha na tarefa de teste.".to_string())?;
    if let Err(e) = cleanup {
        // The fixture was deliberately disconnected before releasing its grab.
        if !e.contains("os error 19") && !e.starts_with("Dispositivo desconectado") { return Err(e); }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{io::Write, os::unix::net::UnixStream};

    // No uinput permissions or physical input needed: exercise the same wait path.
    #[test]
    fn idle_wait_wakes_for_input_and_disconnect() {
        let (source, mut producer) = UnixStream::pair().unwrap();
        let wake = Arc::new(Wake::new().unwrap());
        let waiting = wake.clone();
        let (done, result) = mpsc::channel();
        let worker = thread::spawn(move || {
            waiting.wait(&source, Duration::from_secs(10)).unwrap();
            done.send(()).unwrap();
            source
        });
        producer.write_all(&[1]).unwrap();
        result.recv_timeout(Duration::from_secs(2)).expect("input failed to wake reader");
        let source = worker.join().unwrap();
        let mut byte = [0u8; 1]; rustix::io::read(&source, &mut byte).unwrap();
        drop(producer);
        let started = Instant::now();
        wake.wait(&source, Duration::from_secs(10)).unwrap();
        assert!(started.elapsed() < Duration::from_secs(2), "disconnect failed to wake reader");
    }

    #[test]
    fn idle_wait_observes_queued_and_concurrent_control_notifications() {
        let (source, _producer) = UnixStream::pair().unwrap();
        let wake = Arc::new(Wake::new().unwrap());
        wake.notify().unwrap(); wake.notify().unwrap();
        wake.wait(&source, Duration::from_secs(10)).unwrap();
        // The aggregated counter was drained; a fresh notification must also wake it.
        let waiting = wake.clone(); let (done, result) = mpsc::channel();
        let worker = thread::spawn(move || {
            waiting.wait(&source, Duration::from_secs(10)).unwrap();
            done.send(()).unwrap();
        });
        wake.notify().unwrap();
        result.recv_timeout(Duration::from_secs(2)).expect("control failed to wake idle reader");
        worker.join().unwrap();
    }

    #[test]
    fn idle_wait_retains_authorization_deadline() {
        let (source, _producer) = UnixStream::pair().unwrap();
        let wake = Wake::new().unwrap();
        let guard = || Ok(());
        let mut authorization = CaptureAuthorization { guard: &guard, last_check: None };
        authorization.check().unwrap();
        let deadline = authorization.last_check.unwrap() + Duration::from_millis(100);
        wake.wait(&source, authorization.remaining()).unwrap();
        assert!(Instant::now() >= deadline);
        assert_eq!(authorization.remaining(), Duration::ZERO);
    }

    #[test]
    fn logitech_composite_mouse_accepts_keyboard_media_and_volume_capabilities() {
        assert!(physical_mouse(DeviceKind::Composite, DeviceConnection::Usb, "Logitech G Pro"));
        let keys = [KeyCode::BTN_LEFT, KeyCode::KEY_A, KeyCode::KEY_VOLUMEUP];
        let relative = [RelativeAxisCode::REL_X, RelativeAxisCode::REL_Y, RelativeAxisCode::REL_WHEEL];
        let events = [EventType::SYNCHRONIZATION, EventType::KEY, EventType::RELATIVE,
            EventType::ABSOLUTE, EventType::MISC, EventType::LED, EventType::REPEAT];
        assert!(validate_capabilities(&keys, &relative, &[AbsoluteAxisCode::ABS_VOLUME], &events).is_ok());
        assert!(validate_capabilities(&keys, &[], &[], &events).is_err());
        assert!(validate_capabilities(&[KeyCode::KEY_A], &relative, &[], &events).is_err());
        assert!(validate_capabilities(&keys, &relative, &[AbsoluteAxisCode::ABS_X], &events).is_err());
        assert!(validate_capabilities(&keys, &relative, &[], &[EventType::SWITCH]).is_err());
    }
    #[test]
    fn virtual_and_keyboard_only_registrations_remain_rejected() {
        assert!(!physical_mouse(DeviceKind::Keyboard, DeviceConnection::Usb, "Keyboard"));
        assert!(!physical_mouse(DeviceKind::Composite, DeviceConnection::Virtual, "keymasq-mouse"));
        for name in ["BindForge Output", "BindForge Mouse Output", "BindForge Keyboard Output"] {
            assert!(!physical_mouse(DeviceKind::Mouse, DeviceConnection::Usb, name));
        }
        assert!(physical_mouse(DeviceKind::Mouse, DeviceConnection::Usb, "Mouse"));
    }
    #[test]
    fn keyboard_requires_physical_alphanumeric_input_and_refuses_unsupported_capabilities() {
        let keys = [KeyCode::KEY_A, KeyCode::KEY_Z, KeyCode::KEY_ENTER, KeyCode::KEY_SPACE];
        let events = [EventType::SYNCHRONIZATION, EventType::KEY, EventType::LED, EventType::REPEAT, EventType::MISC];
        assert!(physical_keyboard(DeviceKind::Keyboard, DeviceConnection::Usb, "Keyboard"));
        assert!(physical_keyboard(DeviceKind::Composite, DeviceConnection::Bluetooth, "Keyboard"));
        assert!(!physical_keyboard(DeviceKind::Mouse, DeviceConnection::Usb, "Mouse"));
        assert!(!physical_keyboard(DeviceKind::Keyboard, DeviceConnection::Virtual, "Keyboard"));
        for name in ["BindForge Output", "BindForge Mouse Output", "BindForge Keyboard Output"] {
            assert!(!physical_keyboard(DeviceKind::Keyboard, DeviceConnection::Usb, name));
        }
        assert!(validate_keyboard(&keys, &[], &events).is_ok());
        assert!(validate_keyboard(&[KeyCode::KEY_VOLUMEUP], &[], &events).is_err());
        assert!(validate_keyboard(&keys, &[AbsoluteAxisCode::ABS_X], &events).is_err());
        assert!(validate_keyboard(&keys, &[], &[EventType::SWITCH]).is_err());
    }

    #[test]
    fn capture_observes_revocation_after_start_without_needing_a_key_event() {
        let revoked = AtomicBool::new(false);
        let guard = || if revoked.load(Ordering::Relaxed) { Err("Acesso revogado".into()) } else { Ok(()) };
        let mut authorization = CaptureAuthorization { guard: &guard, last_check: None };
        authorization.check().unwrap();
        revoked.store(true, Ordering::Relaxed);
        authorization.last_check = Some(Instant::now() - Duration::from_millis(101));
        assert_eq!(authorization.check().unwrap_err(), "Acesso revogado");
        revoked.store(false, Ordering::Relaxed);
        authorization.check().unwrap();
    }

}
