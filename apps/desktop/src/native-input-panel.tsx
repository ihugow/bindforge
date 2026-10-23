import { useEffect, useState } from "react";
import { invoke, isTauri } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { Config } from "./profiles";
export type InputStatus = { active: boolean; device_id: string; message: string };
export const inputApi = {
  status: () => invoke<InputStatus>("get_input_status"),
  start: (deviceId: string) => invoke<InputStatus>("start_mouse", { deviceId }),
  pause: () => invoke<InputStatus>("pause_mouse"),
};
export function NativeInputPanel({ native, connected, simulation, config, focused = true, version = "", api = inputApi }: {
  focused?: boolean; version?: string; native: boolean; connected: boolean; simulation: boolean; config: Config | null | undefined; api?: typeof inputApi;
}) {
  const [status, setStatus] = useState<InputStatus | null>(null);
  const [selected, setSelected] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [refresh, setRefresh] = useState(0);
  const mice = config?.devices?.filter(d => d.kind === "mouse") ?? [];
  const selectedId = mice.some(d => d.id === selected) ? selected : mice[0]?.id ?? "";
  useEffect(() => {
    if (!native || !isTauri()) return;
    let cancelled = false;
    const subscription = listen<InputStatus>("bindforge-input-status", event => {
      if (!cancelled) setStatus(event.payload);
    });
    void subscription.catch(e => { if (!cancelled) setError(String(e)); });
    return () => { cancelled = true; void subscription.then(stop => stop()).catch(() => {}); };
  }, [native]);
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
  const activeName = mice.find(d => d.id === status?.device_id)?.name ?? "Mouse selecionado";
  return <section aria-labelledby="native-input-heading">
    <div className="section-heading"><h2 id="native-input-heading">Remapeamento do mouse</h2><span className="status-chip">{status?.active ? "Ativo" : "Pausado"}</span></div>
    <p role="status">{status?.active ? `${activeName} · ações do perfil ativo aplicadas ao dispositivo` : "Ative um mouse para aplicar seus mapeamentos. O daemon inicia pausado e exige nova ativação após bloqueio da sessão ou suspensão."}</p>
    {!focused && <p className="hint">Aguarde a ponte KDE identificar a janela em foco antes de ativar.</p>}
    <label htmlFor="native-mouse">Mouse cadastrado</label><select id="native-mouse" className="app-select" value={selectedId} disabled={busy || !!status?.active} onChange={e => setSelected(e.target.value)}>
      {!mice.length && <option value="">Cadastre um mouse na aba Dispositivos</option>}{mice.map(d => <option key={d.id} value={d.id}>{d.name}</option>)}
    </select>
    <p className="hint">Backend experimental: interfaces físicas de mouse, incluindo modelos compostos compatíveis, com até oito botões suportados. Pause o Keymasq ou outro remapeador nesse mouse antes de ativar. Fechar apenas a interface mantém o daemon funcionando; use Pausar ou encerre o daemon com Ctrl+C.</p>
    <div className="button-row"><button type="button" disabled={!native || !connected || !status || !selectedId || !focused || busy || !!status.active} onClick={() => void act(true)}>{busy && !status?.active ? "Ativando…" : "Ativar remapeamento"}</button>
      <button type="button" disabled={!native || !connected || busy || !status?.active} onClick={() => void act(false)}>{busy && status?.active ? "Pausando…" : "Pausar remapeamento"}</button>
      <button type="button" className="secondary-button" disabled={!native || !connected || busy} onClick={() => { setError(""); setRefresh(n => n + 1); }}>Atualizar estado</button></div>
    {status?.message && <p role="status">{status.message}</p>}{error && <p role="alert">{error}</p>}
    {!native && <p className="hint">Disponível no aplicativo nativo.</p>}
  </section>;
}
