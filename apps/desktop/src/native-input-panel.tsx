import { useEffect, useState } from "react";
import { invoke, isTauri } from "@tauri-apps/api/core";
import { DeviceAccessButton } from "./device-access-panel";
import { listen } from "@tauri-apps/api/event";
import type { Config } from "./profiles";
export type InputStatus = { active: boolean; device_id: string; message: string };
export const inputApi = {
  status: () => invoke<InputStatus>("get_input_status"),
  start: (deviceId: string) => invoke<InputStatus>("start_mouse", { deviceId }),
  pause: () => invoke<InputStatus>("pause_mouse"),
};
export const keyboardInputApi = {
  status: () => invoke<InputStatus>("get_keyboard_status"),
  start: (deviceId: string) => invoke<InputStatus>("start_keyboard", { deviceId }),
  pause: () => invoke<InputStatus>("pause_keyboard"),
};
export function NativeInputPanel({ native, connected, simulation, config, focused = true, version = "", kind = "mouse", api = kind === "keyboard" ? keyboardInputApi : inputApi }: {
  kind?: "mouse" | "keyboard"; focused?: boolean; version?: string; native: boolean; connected: boolean; simulation: boolean; config: Config | null | undefined; api?: typeof inputApi;
}) {
  const [status, setStatus] = useState<InputStatus | null>(null);
  const [selected, setSelected] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [refresh, setRefresh] = useState(0);
  const devices = config?.devices?.filter(d => d.kind === kind) ?? [];
  const name = kind === "keyboard" ? "teclado" : "mouse";
  const title = kind === "keyboard" ? "Teclado" : "Mouse";
  const statusEvent = kind === "keyboard" ? "bindforge-keyboard-status" : "bindforge-input-status";
  const headingId = `native-${kind}-heading`;
  const selectedId = devices.some(d => d.id === selected) ? selected : devices[0]?.id ?? "";
  useEffect(() => {
    if (!native || !isTauri()) return;
    let cancelled = false;
    const subscription = listen<InputStatus>(statusEvent, event => {
      if (!cancelled) setStatus(event.payload);
    });
    void subscription.catch(e => { if (!cancelled) setError(String(e)); });
    return () => { cancelled = true; void subscription.then(stop => stop()).catch(() => {}); };
  }, [native, statusEvent]);
  useEffect(() => {
    if (!native || !connected) { setStatus(null); return; }
    if (busy) return;
    let cancelled = false;
    void api.status().then(s => { if (!cancelled) { setStatus(s); } }).catch(e => { if (!cancelled) { setStatus(null); setError(String(e)); } });
    return () => { cancelled = true; };
  }, [native, connected, simulation, config?.revision, api, refresh, version, busy]);
  async function act(start: boolean) {
    setBusy(true); setError("");
    try { setStatus(await (start ? api.start(selectedId) : api.pause())); }
    catch (e) { setError(String(e)); try { setStatus(await api.status()); } catch {} }
    finally { setBusy(false); }
  }
  const activeName = devices.find(d => d.id === status?.device_id)?.name ?? `${title} selecionado`;
  return <section aria-labelledby={headingId}>
    <div className="section-heading"><h2 id={headingId}>Remapeamento do {name}</h2><span className="status-chip">{status?.active ? "Ativo" : "Pausado"}</span></div>
    <p role="status">{status?.active ? `${activeName} · ações do perfil ativo aplicadas ao dispositivo` : `Ative um ${name} para aplicar seus mapeamentos. O daemon inicia pausado e exige nova ativação após bloqueio da sessão ou suspensão.`}</p>
    {!focused && <p className="hint">Aguarde a ponte KDE identificar a janela em foco antes de ativar.</p>}
    <label htmlFor={`native-${kind}`}>{title} cadastrado</label><select id={`native-${kind}`} className="app-select" value={selectedId} disabled={busy || !!status?.active} onChange={e => setSelected(e.target.value)}>
      {!devices.length && <option value="">Cadastre um {name} na aba Dispositivos</option>}{devices.map(d => <option key={d.id} value={d.id}>{d.name}</option>)}
    </select>
    <p className="hint">{kind === "keyboard" ? "Backend experimental: uma interface de teclado alfanumérico por vez. Solte todas as teclas antes de ativar. Para pausar por emergência, pressione Ctrl + Alt + Pause no teclado físico, mesmo que essas teclas estejam remapeadas." : "Backend experimental: interfaces físicas de mouse, incluindo modelos compostos compatíveis, com até oito botões suportados."} Pause outros remapeadores nessa interface antes de ativar. Fechar a janela mantém o daemon funcionando.</p>
    <div className="button-row"><button type="button" disabled={!native || !connected || !status || !selectedId || !focused || busy || !!status.active} onClick={() => void act(true)}>{busy && !status?.active ? "Ativando…" : "Ativar remapeamento"}</button>
      <button type="button" disabled={!native || !connected || busy || !status?.active} onClick={() => void act(false)}>{busy && status?.active ? "Pausando…" : "Pausar remapeamento"}</button>
      <button type="button" className="secondary-button" disabled={!native || !connected || busy} onClick={() => { setError(""); setRefresh(n => n + 1); }}>Atualizar estado</button></div>
    <div className="native-access-entry"><DeviceAccessButton device={devices.find(d => d.id === selectedId)} disabled={!native || !connected || busy} onChanged={() => setRefresh(n => n + 1)} /></div>
    {status?.message && <p role="status">{status.message}</p>}{error && <p role="alert">{error}</p>}
    {!native && <p className="hint">Disponível no aplicativo nativo.</p>}
  </section>;
}
