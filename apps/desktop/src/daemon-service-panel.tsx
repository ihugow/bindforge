import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

export type ServiceStatus = {
  active_state: string; autostart: boolean; connected: boolean; external: boolean;
  binary_available: boolean; result: string; exit_status: number;
};
type ServiceAction = "start" | "stop" | "restart" | "enable_autostart" | "disable_autostart";
export const serviceApi = {
  status: () => invoke<ServiceStatus>("get_daemon_service"),
  control: (action: ServiceAction) => invoke<ServiceStatus>("control_daemon_service", { action }),
};
export function DaemonServicePanel({ native, active, connected, api = serviceApi }: {
  native: boolean; active: boolean; connected: boolean; api?: typeof serviceApi;
}) {
  const [status, setStatus] = useState<ServiceStatus | null>(null);
  const [busy, setBusy] = useState(false);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");
  const [message, setMessage] = useState("");
  const [refresh, setRefresh] = useState(0);
  useEffect(() => {
    if (!native || !active || busy) return;
    let cancelled = false;
    setLoading(true);
    void api.status().then(value => { if (!cancelled) { setStatus(value); } })
      .catch(e => { if (!cancelled) { setStatus(null); setError(String(e)); } })
      .finally(() => { if (!cancelled) setLoading(false); });
    return () => { cancelled = true; };
  }, [native, active, connected, refresh, busy, api]);
  async function control(action: ServiceAction) {
    setBusy(true); setError(""); setMessage("");
    try {
      setStatus(await api.control(action));
      setMessage(action === "stop" ? "Serviço encerrado. O mouse voltou ao funcionamento original."
        : action === "enable_autostart" ? "Inicialização no login ativada. O remapeamento continuará exigindo ativação."
        : action === "disable_autostart" ? "Inicialização no login desativada. O serviço atual permanece como está."
        : "Serviço iniciado. Ative o remapeamento quando desejar.");
    } catch (e) { setError(String(e)); }
    finally { setBusy(false); }
  }
  const running = status?.active_state === "active";
  const pending = ["activating", "deactivating", "reloading"].includes(status?.active_state ?? "");
  const disabled = !native || busy || loading || !status;
  return <section className="integration-panel" aria-labelledby="service-heading">
    <div className="section-heading"><div><span className="eyebrow">EXECUÇÃO EM SEGUNDO PLANO</span><h2 id="service-heading">Serviço do BindForge</h2></div>
      <span className="status-chip">{status?.external ? "Execução externa" : running ? "Em execução" : status?.active_state === "failed" ? "Falha" : "Parado"}</span></div>
    <p className="hint">O serviço mantém seus perfis funcionando quando você fecha a janela. Iniciar ou reiniciar deixa o remapeamento pausado até você ativá-lo.</p>
    {!native ? <p>Disponível no aplicativo nativo para Fedora/KDE.</p> : loading && !status ? <p role="status">Verificando serviço…</p> : null}
    {status?.external && <p role="status">O daemon foi iniciado fora do aplicativo. Encerre-o no terminal de origem e use Iniciar serviço para passar a gerenciá-lo aqui.</p>}
    {status && !status.binary_available && <p role="alert">Executável do daemon não encontrado. Instale o pacote completo do BindForge.</p>}
    <div className="button-row">
      <button disabled={disabled || running || pending || status?.external || !status?.binary_available} onClick={() => void control("start")}>Iniciar serviço</button>
      <button className="secondary-button" disabled={disabled || !running || status?.external} onClick={() => void control("stop")}>Parar serviço</button>
      <button className="secondary-button" disabled={disabled || !running || status?.external || !status?.binary_available} onClick={() => void control("restart")}>Reiniciar serviço</button>
      <button className="secondary-button" disabled={!native || busy || loading} onClick={() => { setError(""); setRefresh(n => n + 1); }}>Atualizar diagnóstico</button>
    </div>
    <label className="service-autostart"><input type="checkbox" checked={status?.autostart ?? false}
      disabled={disabled || (!status?.autostart && !status?.binary_available)}
      onChange={event => void control(event.target.checked ? "enable_autostart" : "disable_autostart")} />Iniciar serviço ao entrar no KDE</label>
    <p className="hint">Essa opção inicia apenas o serviço, com a captura pausada. Desmarcar não interrompe o serviço em execução.</p>
    {status && <details><summary>Diagnóstico do serviço</summary><dl className="daemon-details">
      <div><dt>Estado</dt><dd>{status.active_state}</dd></div>
      <div><dt>Comunicação</dt><dd>{status.connected ? "Conectado" : "Desconectado"}</dd></div>
      <div><dt>Resultado da última execução</dt><dd>{status.result} · código {status.exit_status}</dd></div>
    </dl></details>}
    {busy && <p role="status">Aplicando alteração…</p>}{message && <p role="status">{message}</p>}{error && <p role="alert">{error}</p>}
  </section>;
}
