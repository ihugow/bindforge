import { AppSelect } from "./app-select";
import { useCallback, useEffect, useRef, useState } from "react";
import { MouseArt, KeyboardArt } from "./device-art";
import { DeviceBattery } from "./device-battery";
import { DeviceForm } from "./device-form";
import { MouseWorkspace } from "./mouse-workspace";
import { KeyboardEditor } from "./keyboard-editor";
import { NativeInputPanel, type inputApi } from "./native-input-panel";
import { Modal } from "./modal";
import { ProfileName } from "./profile-name";
import { profileApi } from "./profile-api";
import { configWithBindings } from "./mouse-bindings";
import { watchInventory, type DetectedDevice, type InventoryState } from "./devices";
import { layouts, withDevice, type ConfiguredDevice } from "./configured-devices";
import { useWorkspaceNavigation } from "./workspace-navigation";
import type { Config, Profile } from "./profiles";
import type { DaemonConnection } from "./daemon-status";

type Editor = { profile: Profile; revision: number; device?: ConfiguredDevice };
export function HomePanel({ config, native, daemon, active, onConfigChange, onSaved, read, api = profileApi, onManageDevices, inputApis }: {
  config: Config | null | undefined; native: boolean; daemon: DaemonConnection; active: boolean;
  onConfigChange: (config: Config) => void; onSaved: () => void; read: () => Promise<DetectedDevice[]>; api?: typeof profileApi;
  onManageDevices?: () => void; inputApis?: { mouse: typeof inputApi; keyboard: typeof inputApi };
}) {
  const [selectedId, setSelectedId] = useState("");
  const [inventory, setInventory] = useState<InventoryState>({ phase: "idle" });
  const [refresh, setRefresh] = useState(0);
  const [editor, setEditor] = useState<Editor | null>(null);
  const [draft, setDraft] = useState({ dirty: false, busy: false });
  const [pending, setPending] = useState<(() => void) | null>(null);
  const [adding, setAdding] = useState(false);
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
    const selector = editor?.device ? `button[aria-label="Configurar ${CSS.escape(editor.device.name)}"]` : "[data-general-mouse]";
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
  const remapping = (device: ConfiguredDevice, configure?: () => void) => <NativeInputPanel
    device={device} kind={device.kind} configure={configure} native={native} connected={connected} focused={connected && daemon.status.focus_state === "focused"}
    version={connected ? daemon.status.revision : ""} simulation={simulation} config={config} api={inputApis?.[device.kind]}
    available={inventory.phase !== "ready" || devices.some(d => d.id === device.interface_id)} />;
  const confirmNavigation = pending && <Modal title="Alterações não salvas" compact onDismiss={() => setPending(null)} initialFocus="[data-keep-editing]">
    <p>Você alterou as ações deste perfil. Continue editando para salvar ou descarte as alterações antes de sair.</p><div className="button-row modal-actions">
      <button type="button" data-keep-editing onClick={() => setPending(null)}>Continuar editando</button>
      <button type="button" className="danger-button" onClick={() => { const proceed = pending; setPending(null); setDraft({ dirty: false, busy: false }); proceed(); }}>Descartar e continuar</button>
    </div></Modal>;
  if (editor) return <div className="device-editor-page">
    <button className="editor-back" type="button" disabled={draft.busy} onClick={() => leave(closeEditor)}>← Dispositivos</button>
    <div className="editor-page-heading"><div><h2 ref={heading} tabIndex={-1}>{editor.device?.name ?? "Ações gerais do mouse"}</h2><p>{editor.device?.kind === "keyboard" ? layouts[editor.device.layout!] : `${editor.device?.button_count ?? 5} botões · mouse`}</p></div>
      {editor.device && remapping(editor.device)}
    </div>
    <div className="editor-profile-context"><div><label htmlFor="editing-profile">Perfil em edição</label><AppSelect id="editing-profile" value={editor.profile.id} disabled={draft.busy || !config} onValueChange={value => {
      const id = value;
      leave(() => { const next = config?.profiles.find(p => p.id === id); if (!next || !config) return; setSelectedId(id); setDraft({ dirty: false, busy: false }); setMessage(""); setEditor({ ...editor, profile: structuredClone(next), revision: config.revision }); });
    }}>{config?.profiles.map(p => <option key={p.id} value={p.id}>{p.name}{p.enabled ? "" : " (desativado)"}</option>)}</AppSelect></div>
      <div className="editor-live-profile"><small>Perfil em uso</small><strong>{connected ? <ProfileName id={daemon.status.active_profile_id} config={config} /> : "Serviço indisponível"}</strong></div></div>
    {message && <p className="editor-saved-message" role="status">{message}</p>}
    {editor.device?.kind === "keyboard" ? <KeyboardEditor key={`${editor.device.id}:${editor.profile.id}`} inline profile={editor.profile} revision={editor.revision} device={editor.device} save={persist} close={closeEditor} restoreFocus={trigger} onDirtyChange={onDirtyChange} />
      : <MouseWorkspace key={`${editor.device?.id ?? "general"}:${editor.profile.id}`} profile={editor.profile} revision={editor.revision} device={editor.device} save={persist} onDirtyChange={onDirtyChange} />}
    {confirmNavigation}
  </div>;
  return <section className="home-panel" aria-labelledby="home-heading">
    <div className="home-heading"><div><h2 id="home-heading">Seus dispositivos</h2><p>Escolha um dispositivo para configurar suas ações.</p></div><button type="button" className="secondary-button" disabled={!native || !connected || !config || inventory.phase !== "ready"} onClick={() => setAdding(true)}>+ Adicionar dispositivo</button></div>
    {config === null && <><p role="alert">Não foi possível carregar seus perfis.</p><button type="button" onClick={() => { setError(""); void api.load().then(onConfigChange).catch(e => setError(String(e))); }}>Tentar novamente</button></>}
    {!connected && <p className="hint" role="status">Inicie o serviço em Configurações para ativar o remapeamento. Seus perfis salvos continuam disponíveis para edição.</p>}
    <div className="device-showcase device-overview">
      {(config?.devices ?? []).map(device => <article key={device.id} className="device-card device-overview-card" aria-label={device.name}>
        <div className="device-card-top"><span>{device.kind === "mouse" ? "MOUSE" : "TECLADO"}</span><DeviceBattery device={devices.find(d => d.id === device.interface_id)} /></div>
        <button className="device-art-button" type="button" disabled={!profile} aria-label={`Abrir configuração de ${device.name}`} onClick={() => open(device)}><span className="device-card-art">{device.kind === "mouse" ? <MouseArt /> : <KeyboardArt />}</span></button>
        <h3 className="device-card-name">{device.name}</h3><span className="device-card-caption">{device.kind === "mouse" ? `${device.button_count} botões` : layouts[device.layout!]}</span>
        <span className="device-card-caption">{inventory.phase !== "ready" ? "Conexão não verificada" : devices.some(d => d.id === device.interface_id) ? "Conectado" : "Desconectado"}</span>
        {remapping(device, profile ? () => open(device) : undefined)}
      </article>)}
    </div>
    {!config?.devices?.length && <p className="hint">Adicione seu mouse ou teclado para começar.</p>}
    <div className="device-overview-footer"><small>Perfil em uso: <strong>{connected ? <ProfileName id={daemon.status.active_profile_id} config={config} /> : "—"}</strong> · A seleção acompanha a janela em foco.</small></div>
    <details className="device-overview-options"><summary>Opções dos dispositivos</summary><div className="button-row">
      <button type="button" data-general-mouse className="secondary-button" disabled={!profile} onClick={() => open()}>Ações gerais do mouse</button>
      {onManageDevices && <button type="button" className="secondary-button" onClick={onManageDevices}>Gerenciar dispositivos e interfaces</button>}
      <button type="button" className="secondary-button" disabled={!native || !connected || inventory.phase === "loading"} onClick={() => setRefresh(n => n + 1)}>Atualizar dispositivos</button>
    </div></details>
    {inventory.phase === "error" && <p role="alert">{inventory.message}</p>}{error && <p role="alert">{error}</p>}
    {adding && config && <DeviceForm inventory={devices} close={() => setAdding(false)} save={async device => { const saved = await api.save(withDevice(config, device, devices), config.revision); onConfigChange(saved); onSaved(); }} />}
  </section>;
}
