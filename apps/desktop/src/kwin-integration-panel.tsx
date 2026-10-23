import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Modal } from "./modal";
import { integrationSummary, integrationAction, integrationReady, type IntegrationStatus } from "./kwin-integration";
export const integrationApi = {
  status: () => invoke<IntegrationStatus>("get_kwin_integration"),
  configure: () => invoke<IntegrationStatus>("configure_kwin_integration"),
};
export function KwinIntegrationPanel({ native, active, daemonState = "", api = integrationApi }: {
  native: boolean; active: boolean; daemonState?: string; api?: typeof integrationApi;
}) {
  const [status, setStatus] = useState<IntegrationStatus | null>(null);
  const [loading, setLoading] = useState(false);
  const [busy, setBusy] = useState(false);
  const [open, setOpen] = useState(false);
  const [error, setError] = useState("");
  const [message, setMessage] = useState("");
  const [refresh, setRefresh] = useState(0);
  const trigger = useRef<HTMLElement | null>(null);
  // Recheck when returning to Home or when the daemon reconnects, not on every focus event.
  useEffect(() => {
    if (!native || !active || busy) return;
    let cancelled = false; setLoading(true);
    void api.status().then(result => { if (!cancelled) setStatus(result); })
      .catch(e => { if (!cancelled) { setStatus(null); setError(String(e)); } }).finally(() => { if (!cancelled) setLoading(false); });
    return () => { cancelled = true; };
  }, [native, active, daemonState, refresh, api, busy]);
  async function configure() {
    setBusy(true); setError(""); setMessage("");
    try { const result = await api.configure(); setStatus(result); setMessage("Reconexão concluída. Alterne para outra janela e use “Verificar novamente” para conferir a comunicação com o daemon."); }
    catch (e) { setError(String(e)); try { setStatus(await api.status()); } catch {} }
    finally { setBusy(false); }
  }
  return <section className="integration-panel" aria-labelledby="integration-heading">
    <div className="section-heading"><div><span className="eyebrow">DETECÇÃO DE JANELAS</span><h2 id="integration-heading">Integração com KDE</h2></div>
      <span className="status-chip">{status && integrationReady(status) ? "Ativa" : "KWin"}</span></div>
    <p role="status">{!native ? "Disponível no aplicativo nativo, no KDE Plasma 6." : loading && !status ? "Verificando integração…" : status ? integrationSummary(status) : "Não foi possível verificar a integração."}</p>
    <p className="hint">A ponte permite detectar a janela em foco e trocar o perfil automaticamente.</p>
    <button type="button" disabled={!native || busy} onClick={() => { trigger.current = document.activeElement as HTMLElement; setOpen(true); setRefresh(n => n + 1); }}>Ver integração com KDE</button>
    {error && !open && <p role="alert">{error}</p>}
    {active && open && <Modal title="Ver integração com KDE" busy={busy} onDismiss={() => setOpen(false)} restoreFocus={trigger}>
      <p>O daemon carrega a ponte temporária ao iniciar e a descarrega ao encerrar. A conexão é recuperada automaticamente se o KWin reiniciar. Não é necessário instalar um script no KDE.</p>
      {loading && <p role="status">Verificando estado…</p>}
      {status && <><dl className="integration-checklist">
        <div><dt>Sessão KDE / KWin</dt><dd>{status.kwin_available ? "KWin disponível" : "KWin não encontrado"}</dd></div>
        <div><dt>Modo de integração</dt><dd>Temporária · gerenciada pelo daemon</dd></div>
        <div><dt>Carregada no KWin</dt><dd>{status.loaded ? "Sim" : "Aguardando conexão automática"}</dd></div>
        <div><dt>Migração anterior</dt><dd>{status.legacy_disabled ? "Script persistente desabilitado" : "Nenhuma migração registrada"}</dd></div>
        <div><dt>Daemon</dt><dd>{status.daemon_connected ? "Conectado" : "Inicie o daemon com --kde"}</dd></div>
        <div><dt>Comunicação de foco</dt><dd>{status.loaded && status.daemon_connected && status.focus_received ? "Evento de foco recebido pelo daemon" : "Aguardando evento de foco"}</dd></div>
      </dl>
      {!status.kwin_available && <p role="status">Esta ponte funciona no KDE Plasma 6. Outros ambientes precisam de outra integração.</p>}
      {status.loaded && status.daemon_connected && !status.focus_received && <p className="hint">Alterne para outra janela e clique em Verificar novamente. Se continuar aguardando, use Reconectar ao KDE.</p>}
      </>}
      <p className="hint">A ponte acompanha o daemon, sem sudo e sem instalação permanente. O script antigo, se encontrado, é desabilitado para evitar eventos duplicados. O remapeamento do mouse é ativado separadamente.</p>
      {status?.message && <p role="status">{status.message}</p>}
      {message && <p role="status">{message}</p>}{error && <p role="alert">{error}</p>}
      <div className="button-row modal-actions">
        <button type="button" disabled={busy} onClick={() => setOpen(false)}>Fechar</button>
        <button type="button" disabled={busy || loading} onClick={() => { setError(""); setRefresh(n => n + 1); }}>Verificar novamente</button>
        <button type="button" disabled={busy || loading || !status?.kwin_available || !status?.daemon_connected} onClick={configure}>{busy ? "Reconectando…" : status ? integrationAction() : "Reconectar ao KDE"}</button>
      </div>
    </Modal>}
  </section>;
}
