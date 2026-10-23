import React, { useEffect, useState } from "react";
import { createRoot } from "react-dom/client";
import { Channel, invoke, isTauri } from "@tauri-apps/api/core";
import "./style.css";
import { DaemonServicePanel } from "./daemon-service-panel";
import { NativeInputPanel } from "./native-input-panel";
import { KwinIntegrationPanel } from "./kwin-integration-panel";
import { HomePanel } from "./home-panel";
import { WorkspaceTabs } from "./workspace-tabs";
import { ProfileHistory } from "./profile-history";
import { ProfileName } from "./profile-name";
import type { Config } from "./profiles";
import { ProfilesPanel } from "./profiles-panel";
import { DevicesPanel } from "./devices-panel";
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
  return <main>
    <header><span className="eyebrow">FEDORA KDE · EM DESENVOLVIMENTO</span>
      <h1>App Buttons Remap</h1><p>Seus dispositivos. Um perfil para cada aplicativo.</p></header>
    <aside><strong>{simulation ? "Remapeamento pausado" : "Remapeamento físico ativo"}</strong><p>{simulation ? "A seleção de perfis continua sendo acompanhada. Ative um mouse cadastrado para executar suas ações." : "O daemon está remapeando o mouse selecionado conforme a janela em foco. Use Pausar remapeamento para restaurar o funcionamento original."}</p></aside>
    <WorkspaceTabs home={visible => <>
      <HomePanel config={config} native={native} daemon={daemon} active={visible} onConfigChange={setConfig} onSaved={() => setSelection(null)} read={readDevices} />
    <DaemonServicePanel native={native} active={visible} connected={daemon.phase === "connected"} />
    <NativeInputPanel focused={daemon.phase === "connected" && daemon.status.focus_state === "focused"} version={daemon.phase === "connected" ? daemon.status.revision : ""} native={native} connected={daemon.phase === "connected"} simulation={simulation} config={config} />
    <KwinIntegrationPanel native={native} active={visible} daemonState={daemon.phase === "connected" ? daemon.status.focus_state : daemon.phase} />
    <section aria-labelledby="daemon-heading">
      <h2 id="daemon-heading">Perfil no daemon</h2>
      <p className="connection" role="status">
        {daemon.phase === "browser" && "Consulta disponível no aplicativo nativo."}
        {daemon.phase === "loading" && "Consultando o daemon…"}
        {daemon.phase === "unavailable" && daemon.message}
        {daemon.phase === "connected" && `Conectado ao daemon · por eventos · ${simulation ? "entrada pausada" : "entrada física ativa"}`}
      </p>
      {daemon.phase === "connected" && <dl className="daemon-details">
        <div><dt>Perfil selecionado</dt><dd><ProfileName id={daemon.status.active_profile_id} config={config} /></dd></div>
        <div><dt>Monitor de foco</dt><dd>{focusLabels[daemon.status.focus_state]}</dd></div>
        <div><dt>Atualização</dt><dd>{daemon.status.revision}</dd></div>
      </dl>}
      {daemon.phase === "connected" && <ProfileHistory status={daemon.status} config={config} />}
      {daemon.phase === "unavailable" && <p className="hint">Use Iniciar serviço no painel acima e confira o diagnóstico. O painel reconecta quando o daemon volta a ficar disponível.</p>}
      {daemon.phase === "browser" && <p className="hint">Execute <code>npm run tauri dev</code> para acompanhar o processo local. O navegador não acessa o daemon.</p>}
    </section>
    </>} profiles={visible => <>
    <ProfilesPanel currentConfig={config} active={visible} onConfigChange={setConfig} native={native} connected={daemon.phase === "connected"} onSaved={() => setSelection(null)} />
    <section aria-labelledby="preview-heading">
      <h2 id="preview-heading">Explorar a seleção de perfis</h2>
      <p>Confira qual dos seus perfis salvos corresponde a um aplicativo, sem alterar o daemon.</p>
      <form onSubmit={preview}>
        <label htmlFor="app-id">ID do aplicativo</label>
        <input id="app-id" value={appId} maxLength={4096}
          onChange={e => { setAppId(e.target.value); setSelection(null); }}
          placeholder="Deixe vazio para simular ausência de foco" />
        <p className="hint">Exemplos: firefox, org.kde.konsole ou unknown.app.</p>
        <button disabled={!native || busy} type="submit">{busy ? "Consultando…" : "Simular seleção"}</button>
      </form>
      {!native && <p role="status">Prévia no navegador. Execute <code>npm run tauri dev</code> para consultar o core Rust.</p>}
      {error && <p role="alert">{error}</p>}
      {selection && <output aria-live="polite"><strong>{selection.name}</strong>
        <span>{selection.bindings} ação(ões) · somente simulação</span></output>}
    </section>
    </>} devices={<DevicesPanel config={config} onConfigChange={setConfig} native={native} connected={daemon.phase === "connected"} read={readDevices} />} />
    <details className="implementation-details"><summary>Sobre esta versão</summary><section><h2>Estado da implementação</h2><ul>
      <li>Core Rust: regras, prioridades e troca de perfil.</li>
      <li>Daemon: processo separado, com consulta de estado pela interface nativa.</li>
      <li>KDE Wayland: ponte D-Bus de foco opcional no daemon.</li>
      <li>Dispositivos: identificação disponível no Linux. Remapeamento de um mouse físico disponível por ativação explícita.</li>
    </ul></section></details>
  </main>;
}
createRoot(document.getElementById("root")!).render(<React.StrictMode><App /></React.StrictMode>);
