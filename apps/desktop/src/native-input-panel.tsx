import { AppButton } from "./app-button";
import { AppSelect } from "./app-select";
import { createPortal } from "react-dom";
import { useEffect, useState, type ReactNode } from "react";
import { invoke, isTauri } from "@tauri-apps/api/core";
import { DeviceAccessButton } from "./device-access-panel";
import { listen } from "@tauri-apps/api/event";
import type { Config } from "./profiles";
import type { ConfiguredDevice } from "./configured-devices";
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
export function NativeInputPanel({ native, connected, simulation, config, focused = true, version = "", kind = "mouse", api = kind === "keyboard" ? keyboardInputApi : inputApi, device, actions, statusContainer, connectionLabel, available = true }: {
  kind?: "mouse" | "keyboard"; focused?: boolean; version?: string; native: boolean; connected: boolean; simulation: boolean; config: Config | null | undefined; api?: typeof inputApi;
  device?: ConfiguredDevice; actions?: ReactNode; statusContainer?: HTMLElement | null; connectionLabel?: string; available?: boolean;
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
  const selectedId = device?.id ?? (devices.some(d => d.id === selected) ? selected : devices[0]?.id ?? "");
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
  if (device) {
    const ownActive = !!status?.active && status.device_id === device.id;
    const otherActive = !!status?.active && !ownActive;
    const label = !native || !connected ? "Serviço indisponível" : !status ? "Verificando…" : ownActive ? (status.message.startsWith("Aguardando uma conexão") ? "Aguardando conexão" : "Remapeamento ativo") : !available ? "Desconectado" : otherActive ? `Outro ${name} ativo` : "Pausado";
    const indicator = <span className="status-chip" data-state={ownActive ? "active" : "paused"} role="status">{label}</span>;
    return <div className="device-remapping">
      {connectionLabel !== undefined ? <div className="device-connection-row"><span className="device-card-caption">{connectionLabel}</span>{indicator}</div> : statusContainer ? createPortal(indicator, statusContainer) : indicator}
      <div className="device-card-actions">
        <AppButton type="button" className="secondary-button" aria-label={`${ownActive ? "Pausar" : "Ativar"} ${device.name}`} disabled={!native || !connected || !status || busy || otherActive || !ownActive && (!focused || !available)} onClick={() => void act(!ownActive)}>{busy ? "Aguarde…" : ownActive ? "Pausar" : "Ativar"}</AppButton>
        <DeviceAccessButton device={device} compact disabled={!native || !connected || busy} onChanged={() => setRefresh(n => n + 1)} />
        {actions}
      </div>
      {!focused && !ownActive && connected && <p className="hint">Aguardando a identificação da janela em foco.</p>}
      {status?.message && <p className="hint" role="status">{status.message}</p>}{error && <p role="alert">{error}</p>}
    </div>;
  }
  return <section className="native-input-panel" aria-labelledby={headingId}>
    <div className="section-heading"><h2 id={headingId}>Remapeamento do {name}</h2><span className="status-chip" data-state={status?.active ? "active" : "paused"}>{!native || !connected ? "Indisponível" : !status ? "Verificando…" : status.active ? "Ativo" : "Pausado"}</span></div>
    <p role="status">{status?.active ? `${activeName} · ações do perfil ativo aplicadas ao dispositivo` : `Ative um ${name} para aplicar seus mapeamentos. O daemon inicia pausado e exige nova ativação após bloqueio da sessão ou suspensão.`}</p>
    {!focused && <p className="hint">Aguarde a ponte KDE identificar a janela em foco antes de ativar.</p>}
    <label htmlFor={`native-${kind}`}>{title} cadastrado</label><AppSelect id={`native-${kind}`} className="app-select" value={selectedId} disabled={busy || !!status?.active} onValueChange={value => setSelected(value)}>
      {!devices.length && <option value="">Adicione um {name} em Cadastros</option>}{devices.map(d => <option key={d.id} value={d.id}>{d.name}</option>)}
    </AppSelect>
    <details className="input-details"><summary>Compatibilidade e segurança</summary><p className="hint">{kind === "keyboard" ? "Uma interface de teclado alfanumérico por vez. Solte todas as teclas antes de ativar. Para pausar por emergência, pressione Ctrl + Alt + Pause no teclado físico, mesmo que essas teclas estejam remapeadas." : "Interfaces físicas de mouse, incluindo modelos compostos compatíveis, com até oito botões suportados."} Pause outros remapeadores nessa interface antes de ativar. Fechar a janela mantém o serviço funcionando.</p></details>
    <div className="button-row"><AppButton type="button" disabled={!native || !connected || !status || !selectedId || !focused || busy || !!status.active} onClick={() => void act(true)}>{busy && !status?.active ? "Ativando…" : "Ativar remapeamento"}</AppButton>
      <AppButton type="button" className="secondary-button" disabled={!native || !connected || busy || !status?.active} onClick={() => void act(false)}>{busy && status?.active ? "Pausando…" : "Pausar remapeamento"}</AppButton>
      <AppButton type="button" className="secondary-button" disabled={!native || !connected || busy} onClick={() => { setError(""); setRefresh(n => n + 1); }}>Atualizar estado</AppButton></div>
    <div className="native-access-entry"><DeviceAccessButton device={devices.find(d => d.id === selectedId)} disabled={!native || !connected || busy} onChanged={() => setRefresh(n => n + 1)} /></div>
    {status?.message && <p role="status">{status.message}</p>}{error && <p role="alert">{error}</p>}
    {!native && <p className="hint">Disponível no aplicativo nativo.</p>}
  </section>;
}
