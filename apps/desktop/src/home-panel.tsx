import { AppButton } from "./app-button";
import { connectedInterface } from "./configured-devices";
import { AppSelect } from "./app-select";
import { useCallback, useEffect, useRef, useState } from "react";
import { MouseArt, KeyboardArt } from "./device-art";
import { DeviceBattery } from "./device-battery";
import { DeviceForm } from "./device-form";
import { AppIcon } from "./app-icon";
import { MouseWorkspace } from "./mouse-workspace";
import { KeyboardEditor } from "./keyboard-editor";
import { NativeInputPanel, type inputApi } from "./native-input-panel";
import { Modal } from "./modal";
import { ProfileName } from "./profile-name";
import { profileApi } from "./profile-api";
import { configWithBindings } from "./mouse-bindings";
import { watchInventory, type DetectedDevice, type InventoryState } from "./devices";
import { withDevice, withoutDevice, type ConfiguredDevice } from "./configured-devices";
import { useWorkspaceNavigation } from "./workspace-navigation";
import type { Config, Profile } from "./profiles";
import type { DaemonConnection } from "./daemon-status";

type Editor = { profile: Profile; revision: number; device?: ConfiguredDevice };
export function HomePanel({ config, native, daemon, active, onConfigChange, onSaved, read, api = profileApi, inputApis }: {
  config: Config | null | undefined; native: boolean; daemon: DaemonConnection; active: boolean;
  onConfigChange: (config: Config) => void; onSaved: () => void; read: () => Promise<DetectedDevice[]>; api?: typeof profileApi;
  inputApis?: { mouse: typeof inputApi; keyboard: typeof inputApi };
}) {
  const [selectedId, setSelectedId] = useState("");
  const [inventory, setInventory] = useState<InventoryState>({ phase: "idle" });
  const [refresh, setRefresh] = useState(0);
  const [editor, setEditor] = useState<Editor | null>(null);
  const [remappingStatusTarget, setRemappingStatusTarget] = useState<HTMLDivElement | null>(null);
  const [draft, setDraft] = useState({ dirty: false, busy: false });
  const [pending, setPending] = useState<(() => void) | null>(null);
  const [adding, setAdding] = useState(false);
  const [editingDevice, setEditingDevice] = useState<ConfiguredDevice | null>(null);
  const [editorVersion, setEditorVersion] = useState(0);
  const [message, setMessage] = useState("");
  const [error, setError] = useState("");
  const trigger = useRef<HTMLElement | null>(null), heading = useRef<HTMLHeadingElement>(null);
  const registerNavigation = useWorkspaceNavigation();
  const simulation = daemon.phase !== "connected" || daemon.status.simulation;
  const connected = daemon.phase === "connected";
  const profile = config?.profiles.find(p => p.id === selectedId) ?? config?.profiles.find(p => p.id === config.fallback_id);
  const devices = inventory.phase === "ready" ? inventory.devices : [];
  const onDirtyChange = useCallback((dirty: boolean, busy: boolean) => setDraft(previous => previous.dirty === dirty && previous.busy === busy ? previous : { dirty, busy }), []);
  useEffect(() => {
    if (!native || !connected) { setInventory({ phase: "idle" }); return; }
    if (!active) return;
    return watchInventory(read, setInventory);
  }, [native, connected, read, refresh, active]);
  useEffect(() => {
    if (!active || !editor || !draft.dirty && !draft.busy) return;
    return registerNavigation(proceed => {
      if (!draft.busy) setPending(() => () => { setEditor(null); setDraft({ dirty: false, busy: false }); proceed(); });
    });
  }, [active, editor, draft.dirty, draft.busy, registerNavigation]);
  useEffect(() => {
    if (!draft.dirty) return;
    const warn = (event: BeforeUnloadEvent) => { event.preventDefault(); event.returnValue = ""; };
    window.addEventListener("beforeunload", warn);
    return () => window.removeEventListener("beforeunload", warn);
  }, [draft.dirty]);
  useEffect(() => { if (editor) heading.current?.focus(); }, [editor?.device?.id, editor?.profile.id]);
  function leave(proceed: () => void) {
    if (draft.busy) return;
    if (draft.dirty) setPending(() => proceed); else proceed();
  }
  function closeEditor() {
    const selector = editor?.device ? `button[aria-label="Abrir remapeamento de ${CSS.escape(editor.device.name)}"]` : "[data-general-mouse]";
    setEditor(null); setDraft({ dirty: false, busy: false }); setMessage("");
    queueMicrotask(() => document.querySelector<HTMLButtonElement>(selector)?.focus());
  }
  function open(device?: ConfiguredDevice) {
    if (!config || !profile) return;
    trigger.current = document.activeElement as HTMLElement;
    setEditor({ profile: structuredClone(profile), revision: config.revision, device: device && structuredClone(device) });
    setDraft({ dirty: false, busy: false }); setMessage("");
  }
  async function persist(next: Profile, revision: number) {
    if (!config) throw new Error("Recarregue seus perfis antes de salvar.");
    const saved = await api.save(configWithBindings(config, next, revision), revision);
    onConfigChange(saved); onSaved();
    setEditor(current => current && { ...current, profile: structuredClone(saved.profiles.find(p => p.id === next.id)!), revision: saved.revision });
    setMessage("Ações salvas neste perfil. O estado do remapeamento foi mantido.");
    return saved;
  }
  function configureDevice(device: ConfiguredDevice) {
    leave(() => {
      setDraft({ dirty: false, busy: false });
      setEditorVersion(n => n + 1);
      setEditingDevice(device);
    });
  }
  const deviceSettings = editingDevice && config && <DeviceForm device={editingDevice} inventory={devices} refreshInventory={native && connected ? read : undefined}
    close={() => setEditingDevice(null)} save={async (device, snapshot) => {
      const saved = await api.save(withDevice(config, device, snapshot ?? devices), config.revision);
      onConfigChange(saved); onSaved();
      setEditor(current => current && { ...current, device: saved.devices?.find(d => d.id === device.id),
        profile: structuredClone(saved.profiles.find(p => p.id === current.profile.id)!), revision: saved.revision });
      setEditorVersion(n => n + 1);
      setMessage(`Configurações de “${device.name.trim()}” salvas.`);
    }} remove={async () => {
      const saved = await api.save(withoutDevice(config, editingDevice), config.revision);
      onConfigChange(saved); onSaved();
      if (editor?.device?.id === editingDevice.id) closeEditor();
      setMessage(`Dispositivo “${editingDevice.name}” removido.`);
    }} />;
  const remapping = (device: ConfiguredDevice, showSettings = false) => <NativeInputPanel
    connectionLabel={showSettings ? (inventory.phase !== "ready" ? "Conexão não verificada" : connectedInterface(device, devices) ? "Conectado" : "Desconectado") : undefined}
    device={device} kind={device.kind} statusContainer={showSettings ? undefined : remappingStatusTarget} actions={showSettings && <AppButton type="button" className="secondary-button device-settings-button" disabled={!native || !config}
      onClick={() => configureDevice(device)} aria-label={`Configurações de ${device.name}`} title="Configurações do dispositivo"><AppIcon name="gear" /></AppButton>}
    native={native} connected={connected} focused={connected && daemon.status.focus_state === "focused"}
    version={connected ? daemon.status.revision : ""} simulation={simulation} config={config} api={inputApis?.[device.kind]}
    available={inventory.phase !== "ready" || !!connectedInterface(device, devices)} />;
  const confirmNavigation = pending && <Modal title="Alterações não salvas" compact onDismiss={() => setPending(null)} initialFocus="[data-keep-editing]">
    <p>Você alterou as ações deste perfil. Continue editando para salvar ou descarte as alterações antes de sair.</p><div className="button-row modal-actions">
      <AppButton type="button" data-keep-editing onClick={() => setPending(null)}>Continuar editando</AppButton>
      <AppButton type="button" className="danger-button" onClick={() => { const proceed = pending; setPending(null); setDraft({ dirty: false, busy: false }); proceed(); }}>Descartar e continuar</AppButton>
    </div></Modal>;
  if (editor) return <div className="device-editor-page">
    <div className="editor-page-heading"><div className="editor-device-identity"><div className="editor-device-title"><AppButton className="editor-back" type="button" aria-label="Voltar para dispositivos" title="Voltar para dispositivos" disabled={draft.busy} onClick={() => leave(closeEditor)}><AppIcon name="back" /></AppButton><h2 ref={heading} tabIndex={-1}>{editor.device?.name ?? "Ações gerais do mouse"}</h2></div></div>
      {editor.device && <div className="editor-device-options">
        <AppButton type="button" className="secondary-button" disabled={!native || !config || draft.busy} onClick={() => configureDevice(editor.device!)}><AppIcon name="gear" />Configurações</AppButton>
        {remapping(editor.device)}
      </div>}
    </div>
    <div className="editor-profile-context"><div><label htmlFor="editing-profile">Perfil em edição</label><AppSelect id="editing-profile" value={editor.profile.id} disabled={draft.busy || !config} onValueChange={value => {
      const id = value;
      leave(() => { const next = config?.profiles.find(p => p.id === id); if (!next || !config) return; setSelectedId(id); setDraft({ dirty: false, busy: false }); setMessage(""); setEditor({ ...editor, profile: structuredClone(next), revision: config.revision }); });
    }}>{config?.profiles.map(p => <option key={p.id} value={p.id}>{p.name}{p.enabled ? "" : " (desativado)"}</option>)}</AppSelect></div>
      {editor.device && <div className="editor-remapping-status" ref={setRemappingStatusTarget} />}</div>
    {message && <p className="editor-saved-message" role="status">{message}</p>}
    {editor.device?.kind === "keyboard" ? <KeyboardEditor key={`${editor.device.id}:${editor.profile.id}:${editorVersion}`} inline profile={editor.profile} revision={editor.revision} device={editor.device} save={persist} close={closeEditor} restoreFocus={trigger} onDirtyChange={onDirtyChange} />
      : <MouseWorkspace key={`${editor.device?.id ?? "general"}:${editor.profile.id}:${editorVersion}`} profile={editor.profile} revision={editor.revision} device={editor.device} save={persist} onDirtyChange={onDirtyChange} />}
    {deviceSettings}
    {confirmNavigation}
  </div>;
  return <section className="home-panel" aria-labelledby="home-heading">
    <div className="workspace-page-heading"><div><h2 id="home-heading">Seus dispositivos</h2><p>Escolha um dispositivo para configurar suas ações.</p></div><div className="workspace-page-actions"><AppButton type="button" disabled={!native || !connected || !config || inventory.phase !== "ready"} onClick={() => setAdding(true)}><AppIcon name="plus" />Adicionar dispositivo</AppButton></div></div>
    {config === null && <><p role="alert">Não foi possível carregar seus perfis.</p><AppButton type="button" onClick={() => { setError(""); void api.load().then(onConfigChange).catch(e => setError(String(e))); }}>Tentar novamente</AppButton></>}
    {!connected && <p className="hint" role="status">Inicie o serviço em Configurações para ativar o remapeamento. Seus perfis salvos continuam disponíveis para edição.</p>}
    <div className="device-showcase device-overview">
      {(config?.devices ?? []).map(device => <article key={device.id} className="device-card device-overview-card" aria-label={device.name}>
        <div className="device-card-top"><span>{device.kind === "mouse" ? "MOUSE" : "TECLADO"}</span><DeviceBattery device={connectedInterface(device, devices)} /></div>
        <AppButton className="device-art-button" type="button" disabled={!profile} aria-label={`Abrir remapeamento de ${device.name}`} onClick={() => open(device)}><span className="device-card-art">{device.kind === "mouse" ? <MouseArt /> : <KeyboardArt />}</span></AppButton>
        <h3 className="device-card-name">{device.name}</h3>
        {remapping(device, true)}
      </article>)}
    </div>
    {!config?.devices?.length && <p className="hint">Adicione seu mouse ou teclado para começar.</p>}
    <div className="device-overview-footer"><small>Perfil em uso: <strong>{connected ? <ProfileName id={daemon.status.active_profile_id} config={config} /> : "—"}</strong> · A seleção acompanha a janela em foco.</small></div>
    <details className="device-overview-options"><summary>Opções dos dispositivos</summary><div className="button-row">
      <AppButton type="button" data-general-mouse className="secondary-button" disabled={!profile} onClick={() => open()}>Ações gerais do mouse</AppButton>
      <AppButton type="button" className="secondary-button" disabled={!native || !connected || inventory.phase === "loading"} onClick={() => setRefresh(n => n + 1)}>Atualizar dispositivos</AppButton>
    </div></details>
    {inventory.phase === "error" && <p role="alert">{inventory.message}</p>}{error && <p role="alert">{error}</p>}
    {message && <p role="status">{message}</p>}
    {deviceSettings}
    {adding && config && <DeviceForm refreshInventory={read} inventory={devices} close={() => setAdding(false)} save={async (device, snapshot) => { const saved = await api.save(withDevice(config, device, snapshot ?? devices), config.revision); onConfigChange(saved); onSaved(); }} />}
  </section>;
}
