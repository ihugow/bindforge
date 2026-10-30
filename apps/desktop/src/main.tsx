import { AppButton } from "./app-button";
import React, { useEffect, useState } from "react";
import { createRoot } from "react-dom/client";
import { Channel, invoke, isTauri } from "@tauri-apps/api/core";
import "./style.css";
import { DaemonServicePanel } from "./daemon-service-panel";
import { KwinIntegrationPanel } from "./kwin-integration-panel";
import { BackupPanel } from "./backup-panel";
import { HomePanel } from "./home-panel";
import { WorkspaceTabs } from "./workspace-tabs";
import { ProfileHistory } from "./profile-history";
import { ProfileName } from "./profile-name";
import type { Config } from "./profiles";
import { ProfilesPanel } from "./profiles-panel";
import type { DetectedDevice } from "./devices";
import { watchDaemon, type DaemonConnection, type DaemonEvent } from "./daemon-status";

const readDevices = () => invoke<DetectedDevice[]>("list_devices");

type Selection = { id: string; name: string; bindings: number };
function App() {
  const native = isTauri();
  const [config, setConfig] = useState<Config | null>();
  const [daemon, setDaemon] = useState<DaemonConnection>({ phase: native ? "loading" : "browser" });
  useEffect(() => {
    if (!native) return;
    return watchDaemon(async receive => {
      const events = new Channel<DaemonEvent>();
      events.onmessage = receive;
      const id = await invoke<number>("start_daemon_status", { events });
      return () => {
        events.onmessage = () => {};
        void invoke("stop_daemon_status", { id }).catch(() => {});
      };
    }, setDaemon);
  }, [native]);
  const focusLabels = {
    waiting: "Aguardando foco do KDE",
    focused: "Janela em foco",
    no_active_window: "Sem janela ativa",
    stopped: "Monitor encerrado",
  };
  const simulation = daemon.phase !== "connected" || daemon.status.simulation;
  const [appId, setAppId] = useState("firefox");
  const [selection, setSelection] = useState<Selection | null>(null);
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
  async function preview(event: React.FormEvent) {
    event.preventDefault(); setError(""); setSelection(null); setBusy(true);
    try {
      setSelection(await invoke<Selection>("preview_profile", { appId }));
    } catch (e) { setError(String(e)); }
    finally { setBusy(false); }
  }
  const connected = daemon.phase === "connected";
  return <main className="app-root">
    <WorkspaceTabs status={<><span className={`connection-dot${connected ? " is-connected" : ""}`} aria-hidden="true" />
      {connected ? "Serviço conectado" : daemon.phase === "loading" ? "Conectando…" : daemon.phase === "browser" ? "Prévia no navegador" : "Serviço indisponível"}</>}
      home={visible => <HomePanel config={config} native={native} daemon={daemon} active={visible} onConfigChange={setConfig} onSaved={() => setSelection(null)} read={readDevices} />}
      profiles={visible => <ProfilesPanel currentConfig={config} active={visible} onConfigChange={setConfig} native={native} connected={connected} onSaved={() => setSelection(null)} />}
      settings={visible => <>
        <div className="page-heading"><span className="eyebrow">DO SEU JEITO</span><h2>Configurações</h2><p>Backup, inicialização, integração com o KDE e diagnóstico.</p></div>
        <BackupPanel native={native} onRestored={restored => { setConfig(restored); setSelection(null); }} />
        <DaemonServicePanel native={native} active={visible} connected={connected} />
        <KwinIntegrationPanel native={native} active={visible} daemonState={connected ? daemon.status.focus_state : daemon.phase} />
        <section aria-labelledby="daemon-heading">
          <h2 id="daemon-heading">Diagnóstico de perfis</h2>
          <p className="connection" role="status">
            {daemon.phase === "browser" && "Consulta disponível no aplicativo nativo."}
            {daemon.phase === "loading" && "Consultando o serviço…"}
            {daemon.phase === "unavailable" && daemon.message}
            {connected && `Conectado · ${simulation ? "remapeamento pausado" : "há dispositivos ativos"}`}
          </p>
          {connected && <dl className="daemon-details">
            <div><dt>Perfil em uso</dt><dd><ProfileName id={daemon.status.active_profile_id} config={config} /></dd></div>
            <div><dt>Monitor de foco</dt><dd>{focusLabels[daemon.status.focus_state]}</dd></div>
            <div><dt>Atualização</dt><dd>{daemon.status.revision}</dd></div>
          </dl>}
          {connected && <ProfileHistory status={daemon.status} config={config} />}
          {daemon.phase === "unavailable" && <p className="hint">Inicie o serviço no painel acima. A interface reconecta automaticamente.</p>}
          <details className="selection-preview"><summary>Testar a seleção de um perfil</summary>
            <p className="hint">Confira qual perfil salvo corresponde a um aplicativo.</p>
            <form onSubmit={preview}>
              <label htmlFor="app-id">ID do aplicativo</label>
              <input id="app-id" value={appId} maxLength={4096} onChange={e => { setAppId(e.target.value); setSelection(null); }} placeholder="Deixe vazio para simular ausência de foco" />
              <p className="hint">Exemplos: firefox, org.kde.konsole ou unknown.app.</p>
              <AppButton disabled={!native || busy} type="submit">{busy ? "Consultando…" : "Simular seleção"}</AppButton>
            </form>
            {error && <p role="alert">{error}</p>}
            {selection && <output aria-live="polite"><strong>{selection.name}</strong><span>{selection.bindings} ação(ões) · somente simulação</span></output>}
          </details>
        </section>
        <details className="implementation-details"><summary>Sobre esta versão</summary><section><h2>BindForge para Fedora e KDE</h2><p>Perfis por aplicativo, configuração de mouse e teclado e remapeamento por ativação explícita.</p></section></details>
      </>}
    />
  </main>;
}
createRoot(document.getElementById("root")!).render(<React.StrictMode><App /></React.StrictMode>);
