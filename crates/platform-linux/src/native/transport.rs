use std::{fs, io, os::fd::AsFd, os::unix::fs::MetadataExt, path::{Path, PathBuf}, sync::{Arc, Mutex, atomic::{AtomicBool, Ordering}, mpsc}, thread, time::{Duration, Instant}};
use evdev::{raw_stream::RawDevice, uinput::VirtualDevice, AbsoluteAxisCode, AttributeSet, EventType, InputEvent, KeyCode, RelativeAxisCode, UinputAbsSetup};
use remap_core::{DeviceConnection, DeviceEnumerator, DeviceKind};
use super::{Engine, Plan, MouseRegistration, NativeStatus};
const OUTPUT_NAME: &str = "App Buttons Remap Output";
const TEST_INPUT: &str = "App Buttons Remap Test Input";
const TEST_OUTPUT: &str = "App Buttons Remap Test Output";
fn error(context: &str, e: impl std::fmt::Display) -> String { format!("{context}: {e}") }
fn nonblocking(device: &RawDevice) -> Result<(), String> {
    let flags = rustix::fs::fcntl_getfl(device).map_err(|e| e.to_string())?;
    rustix::fs::fcntl_setfl(device, flags | rustix::fs::OFlags::NONBLOCK).map_err(|e| e.to_string())
}
fn open_registered(id: &str) -> Result<RawDevice, String> {
    let inventory = crate::LinuxDeviceEnumerator.enumerate()?;
    let metadata = inventory.into_iter().find(|d| d.id == id).ok_or("Interface não encontrada. Atualize os dispositivos e refaça o vínculo se necessário.")?;
    if !physical_mouse(metadata.kind, metadata.connection, &metadata.name) {
        return Err("Selecione uma interface física com funções de mouse. Teclados sem ponteiro e saídas virtuais não são capturados.".into());
    }
    let syspath = id.strip_prefix("linux:").ok_or("Identificador de interface incompatível.")?;
    let udev = udev::Device::from_syspath(Path::new(syspath)).map_err(|e| e.to_string())?;
    let path = udev.devnode().ok_or("A interface não tem um nó evdev.")?;
    let device = RawDevice::open(path).map_err(|e| error("Não foi possível abrir o mouse. Verifique o acesso específico à interface", e))?;
    let fd = device.as_fd().try_clone_to_owned().map_err(|e| e.to_string())?;
    let rdev = fs::File::from(fd).metadata().map_err(|e| e.to_string())?.rdev();
    let current = udev::Device::from_syspath(Path::new(syspath)).map_err(|e| e.to_string())?;
    if current.devnum() != Some(rdev) || udev.devnum() != Some(rdev) { return Err("O dispositivo mudou durante a abertura. Atualize a lista.".into()); }
    validate_source(&device)?; nonblocking(&device)?; Ok(device)
}
fn physical_mouse(kind: DeviceKind, connection: DeviceConnection, name: &str) -> bool {
    matches!(kind, DeviceKind::Mouse | DeviceKind::Composite)
        && connection != DeviceConnection::Virtual && !name.starts_with("App Buttons Remap")
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
fn validate_source(device: &RawDevice) -> Result<(), String> {
    if device.name().is_some_and(|n| n.starts_with(OUTPUT_NAME)) {
        return Err("A saída virtual do app não pode ser capturada.".into());
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
    if let Some(source_keys) = source.supported_keys() { for k in source_keys.iter() { keys.insert(k); } }
    let mut builder = VirtualDevice::builder().map_err(|e| error("Não foi possível acessar /dev/uinput", e))?.name(name)
        .with_keys(&keys).map_err(|e| e.to_string())?;
    if let Some(axes) = source.supported_relative_axes() { builder = builder.with_relative_axes(axes).map_err(|e| e.to_string())?; }
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
enum Control { Apply(Plan, mpsc::SyncSender<Result<(),String>>) }
pub(super) struct Worker {
    device: MouseRegistration, control: mpsc::SyncSender<Control>, stopping: Arc<AtomicBool>,
    health: Arc<Mutex<NativeStatus>>, thread: Option<thread::JoinHandle<()>>,
}
impl Worker {
    pub fn start(device: MouseRegistration, plan: Plan) -> Result<Self,String> {
        let mut source = open_registered(&device.interface_id)?;
        let mut output = virtual_output(&source, OUTPUT_NAME)?;
        let output_node = node(&mut output)?;
        wait_initialized(&output_node)?;
        source.grab().map_err(|e| error("Não foi possível capturar exclusivamente o mouse. Pause outros remapeadores, como o Keymasq, para esse dispositivo", e))?;
        // Discard events from before acquisition, and require all buttons released.
        match source.fetch_events() { Ok(events) => { let _ = events.count(); }, Err(e) if e.kind() == io::ErrorKind::WouldBlock => {}, Err(e) => return Err(e.to_string()) }
        if source.get_key_state().map_err(|e| e.to_string())?.iter().next().is_some() { return Err("Solte todos os botões do mouse e tente ativar novamente.".into()); }
        let (control, commands) = mpsc::sync_channel(2); let stopping = Arc::new(AtomicBool::new(false));
        let health = Arc::new(Mutex::new(NativeStatus { active: true, device_id: device.id.clone(), message: String::new() }));
        let stop = stopping.clone(); let state = health.clone();
        let thread = thread::Builder::new().name("mouse-remapper".into()).spawn(move || {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| pump(&mut source, output, plan, &commands, &stop)))
                .unwrap_or_else(|_| Err("A tarefa de entrada falhou e foi encerrada.".into()));
            // Closing RawDevice releases EVIOCGRAB even on errors/panic.
            drop(source);
            let mut state = state.lock().unwrap_or_else(|e| e.into_inner()); state.active = false;
            state.message = result.err().unwrap_or_default();
        }).map_err(|e| e.to_string())?;
        Ok(Self { device, control, stopping, health, thread: Some(thread) })
    }
    pub fn interface_id(&self) -> &str { &self.device.interface_id }
    pub fn status(&self) -> NativeStatus { self.health.lock().unwrap_or_else(|e| e.into_inner()).clone() }
    pub fn apply(&self, plan: Plan) -> Result<(),String> {
        let (send, recv) = mpsc::sync_channel(1);
        self.control.send(Control::Apply(plan, send)).map_err(|_| "O leitor do mouse encerrou. Ative novamente.".to_string())?;
        recv.recv().map_err(|_| "O leitor do mouse encerrou durante a troca de perfil.".to_string())?
    }
    pub fn stop(mut self) { self.stop_inner(); }
    fn stop_inner(&mut self) {
        self.stopping.store(true, Ordering::Relaxed);
        if let Some(handle) = self.thread.take() { let _ = handle.join(); }
    }
}
impl Drop for Worker { fn drop(&mut self) { self.stop_inner(); } }
fn pump(source: &mut RawDevice, mut output: VirtualDevice, plan: Plan, commands: &mpsc::Receiver<Control>, stop: &AtomicBool) -> Result<(),String> {
    let mut engine = Engine::new(plan); let mut frame = vec![];
    let result = (|| {
        while !stop.load(Ordering::Relaxed) {
            while let Ok(Control::Apply(plan, ack)) = commands.try_recv() {
                let result = emit(&mut output, &engine.replace(plan));
                let _ = ack.send(result.clone()); result?;
            }
            let events: Vec<_> = match source.fetch_events() {
                Ok(events) => events.collect(), Err(e) if e.kind() == io::ErrorKind::WouldBlock => { thread::sleep(Duration::from_millis(2)); continue; },
                Err(e) => return Err(error("Mouse desconectado ou leitura interrompida", e)),
            };
            if events.is_empty() { return Err("O mouse encerrou o fluxo de eventos.".into()); }
            for event in events {
                if event.event_type() == EventType::SYNCHRONIZATION {
                    if event.code() == 3 { return Err("O fluxo do mouse perdeu eventos. Remapeamento pausado; ative novamente.".into()); }
                    if event.code() == 0 {
                        let mut out = vec![];
                        for input in frame.drain(..) { out.extend(engine.event(input)); }
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
pub fn isolated_self_test() -> Result<(),String> {
    let mut keys = AttributeSet::<KeyCode>::new(); for code in [KeyCode::BTN_LEFT,KeyCode::BTN_RIGHT,KeyCode::BTN_MIDDLE,KeyCode::BTN_SIDE,KeyCode::BTN_EXTRA] { keys.insert(code); }
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
    let plan = Plan(std::collections::BTreeMap::from([(KeyCode::BTN_SIDE.0,vec![KeyCode::KEY_V.0])]));
    let (send, commands) = mpsc::sync_channel(2); let stop = Arc::new(AtomicBool::new(false)); let worker_stop = stop.clone();
    let worker = thread::spawn(move || pump(&mut source, output, plan, &commands, &worker_stop));
    let result = (|| {
        let button = |value| InputEvent::new(EventType::KEY.0,KeyCode::BTN_SIDE.0,value);
        input.emit(&[button(1),button(0)]).map_err(|e| e.to_string())?;
        if read_keys(&mut reader,2)? != vec![(KeyCode::KEY_V.0,1),(KeyCode::KEY_V.0,0)] { return Err("A ação V não foi emitida corretamente.".into()); }
        input.emit(&[button(1)]).map_err(|e| e.to_string())?;
        if read_keys(&mut reader,1)? != vec![(KeyCode::KEY_V.0,1)] { return Err("O pressionamento não foi mantido.".into()); }
        let (ack, recv) = mpsc::sync_channel(1);
        send.send(Control::Apply(Plan(std::collections::BTreeMap::from([(KeyCode::BTN_SIDE.0,vec![KeyCode::KEY_C.0])])),ack)).map_err(|e| e.to_string())?;
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
    drop(input); stop.store(true,Ordering::Relaxed); drop(send);
    let cleanup = worker.join().map_err(|_| "Falha na tarefa de teste.".to_string())?;
    if let Err(e) = cleanup {
        // The fixture was deliberately disconnected before releasing its grab.
        if !e.contains("os error 19") && !e.starts_with("Mouse desconectado") { return Err(e); }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
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
        assert!(!physical_mouse(DeviceKind::Mouse, DeviceConnection::Usb, OUTPUT_NAME));
        assert!(physical_mouse(DeviceKind::Mouse, DeviceConnection::Usb, "Mouse"));
    }
}
