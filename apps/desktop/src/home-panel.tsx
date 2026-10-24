import { useEffect, useRef, useState } from "react";
import { MouseArt, KeyboardArt } from "./device-art";
import { DeviceBattery } from "./device-battery";
import { MouseEditor } from "./mouse-editor";
import { KeyboardEditor } from "./keyboard-editor";
import { ProfileName } from "./profile-name";
import { profileApi } from "./profile-api";
import { configWithBindings } from "./mouse-bindings";
import { watchInventory, type DetectedDevice, type InventoryState } from "./devices";
import { layouts } from "./configured-devices";
import type { ConfiguredDevice } from "./configured-devices";
import type { Config, Profile } from "./profiles";
import type { DaemonConnection } from "./daemon-status";

export function HomePanel({ config, native, daemon, active, onConfigChange, onSaved, read, api = profileApi }: {
  config: Config | null | undefined; native: boolean; daemon: DaemonConnection; active: boolean;
  onConfigChange: (config: Config) => void; onSaved: () => void; read: () => Promise<DetectedDevice[]>; api?: typeof profileApi;
}) {
  const [selectedId, setSelectedId] = useState("");
  const [inventory, setInventory] = useState<InventoryState>({ phase: "idle" });
  const [refresh, setRefresh] = useState(0);
  const [editor, setEditor] = useState<{ profile: Profile; revision: number; device?: ConfiguredDevice } | null>(null);
  const [keyboard, setKeyboard] = useState<{ profile: Profile; revision: number; device: ConfiguredDevice } | null>(null);
  const [message, setMessage] = useState("");
  const [error, setError] = useState("");
  const trigger = useRef<HTMLElement | null>(null);
  const simulation = daemon.phase !== "connected" || daemon.status.simulation;
  const connected = daemon.phase === "connected";
  const profile = config?.profiles.find(p => p.id === selectedId) ?? config?.profiles.find(p => p.id === config.fallback_id);
  const devices = inventory.phase === "ready" ? inventory.devices : [];
  useEffect(() => {
    if (!native || !connected) { setInventory({ phase: "idle" }); return; }
    if (!active) return;
    return watchInventory(read, setInventory);
  }, [native, connected, read, refresh, active]);
  async function persist(next: Profile, revision: number, keyboard = false) {
    if (!config) throw new Error("Recarregue seus perfis antes de salvar.");
    const saved = await api.save(configWithBindings(config, next, revision), revision);
    onConfigChange(saved); onSaved(); setMessage(`Mapeamentos de “${next.name}” salvos. ${keyboard ? "Ative este teclado no daemon para aplicar suas ações conforme o perfil em foco." : simulation ? "Ative o remapeamento para aplicar ao mouse físico." : "O daemon aplica as ações conforme o perfil ativo."}`);
    return saved;
  }
  return <section className="home-panel" aria-labelledby="home-heading">
    <div className="home-heading"><div><span className="eyebrow">SEU ESPAÇO DE CONFIGURAÇÃO</span><h2 id="home-heading">Um perfil. Seus controles.</h2></div><span className="status-chip">{simulation ? "Entrada pausada" : "Entrada ativa"}</span></div>
    <div className="profile-toolbar"><div><label htmlFor="editing-profile">Editar perfil</label>
      <select id="editing-profile" className="app-select" value={profile?.id ?? ""} disabled={!config} onChange={e => { setSelectedId(e.target.value); setMessage(""); }}>
        {!config && <option value="">{native ? "Carregando perfis…" : "Disponível no aplicativo nativo"}</option>}
        {config?.profiles.map(p => <option key={p.id} value={p.id}>{p.name}{p.enabled ? "" : " (desativado)"}</option>)}
      </select></div>
      <div className="live-profile"><small>Ativo no daemon</small><strong>{daemon.phase === "connected" ? <ProfileName id={daemon.status.active_profile_id} config={config} /> : daemon.phase === "loading" ? "Conectando…" : "Daemon desconectado"}</strong></div>
    </div>
    <p className="hint">O perfil em edição é independente do perfil ativo. Trocar de janela não muda o que você está configurando.</p>
    {config === null && <><p role="alert">Não foi possível carregar seus perfis.</p><button type="button" onClick={() => { setError(""); void api.load().then(onConfigChange).catch(e => setError(String(e))); }}>Tentar novamente</button></>}
    <div className="device-showcase">
      {(config?.devices ?? []).map(device => <button key={device.id} type="button" className="device-card" disabled={!profile} onClick={() => {
        trigger.current = document.activeElement as HTMLElement;
        if (device.kind === "mouse" && profile && config) { setEditor({ profile: structuredClone(profile), revision: config.revision, device: structuredClone(device) }); setMessage(""); }
        else if (profile && config) { setKeyboard({ profile: structuredClone(profile), revision: config.revision, device: structuredClone(device) }); setMessage(""); }
      }}>
        <span className="device-card-art">{device.kind === "mouse" ? <MouseArt /> : <KeyboardArt />}</span><span className="device-card-name">{device.name}</span>
        <span className="device-card-caption">{device.kind === "mouse" ? `${device.button_count} botões` : layouts[device.layout!]}</span>
        <span className="device-card-caption">{inventory.phase !== "ready" ? "Conexão não verificada" : devices.some(d => d.id === device.interface_id) ? "Interface conectada" : "Interface indisponível"}</span>
        <DeviceBattery device={devices.find(d => d.id === device.interface_id)} />
        <span className="device-card-link">{device.kind === "mouse" ? "Configurar botões ↗" : "Configurar teclas ↗"}</span>
      </button>)}
    </div>
    {!config?.devices?.length && <p className="hint">Abra a aba Dispositivos e clique em + para cadastrar seu mouse ou teclado. Eles aparecerão aqui.</p>}
    <div className="home-device-note"><button type="button" className="secondary-button" disabled={!profile} onClick={() => { if (profile && config) { trigger.current = document.activeElement as HTMLElement; setEditor({ profile: structuredClone(profile), revision: config.revision }); } }}>Ações gerais do mouse</button>
      <button type="button" className="secondary-button" disabled={!native || !connected || inventory.phase === "loading"} onClick={() => setRefresh(n => n + 1)}>Atualizar dispositivos</button>
    </div>
    {inventory.phase === "error" && <p role="alert">{inventory.message}</p>}
    {message && <p role="status">{message}</p>}{error && <p role="alert">{error}</p>}
    {active && editor && <MouseEditor simulation={simulation} profile={editor.profile} revision={editor.revision} device={editor.device} save={persist} close={() => setEditor(null)} restoreFocus={trigger} />}
    {active && keyboard && <KeyboardEditor profile={keyboard.profile} revision={keyboard.revision} device={keyboard.device} save={(next, revision) => persist(next, revision, true)} close={() => setKeyboard(null)} restoreFocus={trigger} />}
  </section>;
}
