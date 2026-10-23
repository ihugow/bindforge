import React, { useCallback, useEffect, useRef, useState } from "react";
import { profileApi as nativeApi } from "./profile-api";
import { Modal } from "./modal";
import { WindowCapturePanel } from "./window-capture-panel";
import type { CaptureStart } from "./window-capture";
import { appIds, conflictingProfiles, removeProfile, updateProfile, type Config, type Draft, type Profile } from "./profiles";

export function ProfilesPanel({ native, connected, onSaved, api = nativeApi, captureStart, onConfigChange, active = true, currentConfig }: {
  currentConfig?: Config | null; active?: boolean; native: boolean; connected: boolean; onSaved: () => void; api?: typeof nativeApi; captureStart?: CaptureStart; onConfigChange?: (config: Config | null) => void;
}) {
  const editorTrigger = useRef<HTMLElement | null>(null);
  const captureTrigger = useRef<HTMLElement | null>(null);
  const deleteTrigger = useRef<HTMLElement | null>(null);
  const [config, setConfig] = useState<Config | null>(null);
  useEffect(() => { if (currentConfig) setConfig(currentConfig); }, [currentConfig]);
  const [draft, setDraft] = useState<Draft | null>(null);
  const [deleteId, setDeleteId] = useState<string | null>(null);
  const [captureOpen, setCaptureOpen] = useState(false);
  const [capturePending, setCapturePending] = useState(false);
  // Preserve the draft across tabs, but never keep a hidden window capture armed.
  useEffect(() => { if (!active) { setCaptureOpen(false); setDeleteId(null); } }, [active]);
  const detected = useCallback((appId: string) => {
    setDraft(d => d ? { ...d, apps: [...new Set([...appIds(d.apps), appId])].join("\n") } : d);
  }, []);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [message, setMessage] = useState("");
  useEffect(() => {
    if (!native) return;
    let active = true;
    api.load().then(c => { if (active) { setConfig(c); onConfigChange?.(c); } }).catch(e => { if (active) { setError(String(e)); onConfigChange?.(null); } });
    return () => { active = false; };
  }, [native, api, onConfigChange]);
  async function reload() {
    setBusy(true); setError(""); setMessage("");
    try { const loaded = await api.load(); setConfig(loaded); onConfigChange?.(loaded); setDraft(null); }
    catch (e) { setError(String(e)); } finally { setBusy(false); }
  }
  function edit(profile?: Profile) {
    if (!config) return;
    editorTrigger.current = document.activeElement as HTMLElement;
    setError(""); setMessage(""); setCaptureOpen(false);
    setDraft({ id: profile?.id ?? null, name: profile?.name ?? "", apps: profile?.rules.map(r => r.app_id).filter(Boolean).join("\n") ?? "", priority: String(profile?.priority ?? 10), enabled: profile?.enabled ?? true, source: config.fallback_id });
  }
  function patch(value: Partial<Draft>) { setDraft(d => d ? { ...d, ...value } : d); }
  async function save(event: React.FormEvent) {
    event.preventDefault(); if (!config || !draft || capturePending) return;
    setBusy(true); setError(""); setMessage("");
    try {
      const next = updateProfile(config, draft, `profile-${crypto.randomUUID()}`);
      const saved = await api.save(next, config.revision);
      setConfig(saved); onConfigChange?.(saved); setDraft(null); onSaved();
      setMessage(connected ? "Perfis salvos. O daemon recarrega as alterações automaticamente." : "Perfis salvos. Serão carregados quando o daemon iniciar.");
    } catch (e) { setError(String(e)); } finally { setBusy(false); }
  }
  async function confirmDelete(id: string) {
    if (!config || busy || draft || deleteId !== id) return;
    setBusy(true); setError(""); setMessage("");
    try {
      const name = config.profiles.find(p => p.id === id)?.name;
      const saved = await api.save(removeProfile(config, id), config.revision);
      setConfig(saved); onConfigChange?.(saved); onSaved(); setDeleteId(null);
      setMessage(`Perfil “${name}” excluído.${connected ? " O daemon reavalia automaticamente o perfil selecionado." : " A alteração será usada quando o daemon iniciar."}`);
    } catch (e) { setError(String(e)); } finally { setBusy(false); }
  }
  const deleteTarget = config?.profiles.find(p => p.id === deleteId);
  const fallback = !!draft?.id && draft.id === config?.fallback_id;
  const conflicts = config && draft ? conflictingProfiles(config, draft) : [];
  const selected = config?.profiles.find(p => p.id === draft?.id);
  const advanced = selected?.rules.some(r => r.executable !== null || r.title_contains !== null) ?? false;
  return <section aria-labelledby="profiles-heading">
    <div className="section-heading"><h2 id="profiles-heading">Seus perfis</h2>
      {native && <div className="button-row"><button type="button" disabled={busy || !!draft || !!deleteId} onClick={reload}>Recarregar lista</button>
        <button type="button" disabled={!config || busy || !!draft || !!deleteId} onClick={() => edit()}>Criar perfil</button></div>}
    </div>
    {!native && <p>Abra o aplicativo nativo para criar e salvar perfis.</p>}
    {native && !config && !error && <p role="status">Carregando perfis…</p>}
    {config && <><p className="hint">O perfil Padrão é usado quando nenhum aplicativo corresponde. Cada perfil tem suas próprias ações, sem herança automática.</p>
      <ul className="profile-list">{config.profiles.map(p => <li key={p.id}>
        <div><strong>{p.name}</strong><span>{p.id === config.fallback_id ? "Padrão · todos os demais aplicativos" : `${p.enabled ? "Ativo" : "Desativado"} · ${p.rules.map(r => r.app_id ?? "Regra avançada").join(", ")}`}</span>
        <span>{p.bindings.length} ação(ões) · prioridade {p.priority}</span></div>
        <div className="button-row profile-actions">
          <button type="button" disabled={busy || !!draft || !!deleteId} onClick={() => edit(p)}>Editar</button>
          {p.id !== config.fallback_id && <button type="button" className="danger-button" aria-label={`Excluir perfil ${p.name}`}
            disabled={busy || !!draft || !!deleteId} onClick={() => { deleteTrigger.current = document.activeElement as HTMLElement; setDeleteId(p.id); setError(""); setMessage(""); }}>Excluir</button>}
        </div>

      </li>)}</ul></>}
    {active && draft && config && <Modal title={draft.id ? "Editar perfil" : "Criar perfil"} busy={busy} initialFocus="#profile-name" restoreFocus={editorTrigger}
      onDismiss={() => { setDraft(null); setCaptureOpen(false); setError(""); }}>
    <form className="profile-form" onSubmit={save}>
      <fieldset disabled={busy}>
        <label htmlFor="profile-name">Nome do perfil</label>
        <input id="profile-name" autoFocus required maxLength={128} value={draft.name} onChange={e => patch({ name: e.target.value })} />
        {!fallback && <><label htmlFor="profile-apps">IDs dos aplicativos — um por linha</label>
          <textarea id="profile-apps" required={!advanced} rows={3} value={draft.apps} onChange={e => patch({ apps: e.target.value })} placeholder={"firefox\norg.kde.konsole"} disabled={advanced || busy} />
          <p className="hint">O ID deve corresponder exatamente ao informado pelo KDE. {advanced && "Este perfil tem regras avançadas; suas associações são preservadas nesta tela."}</p>
          {appIds(draft.apps).some(id => id.toLowerCase() === "gamescope") && <p role="status">A associação “gamescope” é genérica e não identifica um jogo específico. Use a detecção atualizada para obter um ID como steam:1808500.</p>}
          {!advanced && <><button type="button" disabled={!connected || captureOpen} onClick={() => { captureTrigger.current = document.activeElement as HTMLElement; setCaptureOpen(true); }}>Detectar pela janela</button>
            {!connected && <p className="hint">Conecte o daemon KDE para usar a detecção automática.</p>}
          </>}</>}
        {!draft.id && <><label htmlFor="profile-source">Como começar</label>
          <select id="profile-source" value={draft.source} onChange={e => patch({ source: e.target.value })}>
            <option value="">Sem remapeamentos</option>
            {config.profiles.map(p => <option key={p.id} value={p.id}>Copiar ações de {p.name}</option>)}
          </select><p className="hint">A cópia é independente. Alterações na origem não mudam este perfil.</p></>}
        {!fallback && <><label className="checkbox-label"><input type="checkbox" checked={draft.enabled} onChange={e => patch({ enabled: e.target.checked })} /> Perfil habilitado</label>
          <details open={conflicts.length > 0}><summary>Prioridade e conflitos</summary>
            <label htmlFor="profile-priority">Prioridade — o maior número vence</label>
            <input id="profile-priority" type="number" step={1} min={-2147483648} max={2147483647} required value={draft.priority} onChange={e => patch({ priority: e.target.value })} />
            {conflicts.length > 0 && <p role="status">Também associado a: {conflicts.map(p => `${p.name} (prioridade ${p.priority})`).join(", ")}. Em empate, vence a regra mais específica e depois o menor ID.</p>}
          </details></>}
        <p className="hint">As ações existentes são preservadas. Configure os botões do mouse na aba Início. Nenhuma entrada é remapeada nesta versão.</p>
        {error && <p role="alert">{error}</p>}
        <div className="button-row"><button type="submit" disabled={capturePending}>{busy ? "Salvando…" : "Salvar alterações"}</button><button type="button" onClick={() => { setDraft(null); setCaptureOpen(false); setError(""); }}>Cancelar</button></div>
      </fieldset>
    </form>
    {captureOpen && <Modal title="Detectar aplicativo pela janela" compact restoreFocus={captureTrigger} onDismiss={() => setCaptureOpen(false)}>
      <WindowCapturePanel connected={connected} detected={detected} pending={setCapturePending} close={() => setCaptureOpen(false)} start={captureStart} />
    </Modal>}
    </Modal>}
    {active && deleteTarget && <Modal title={`Excluir perfil ${deleteTarget.name}`} compact busy={busy} initialFocus="[data-cancel-delete]" restoreFocus={deleteTrigger}
      onDismiss={() => { setDeleteId(null); setError(""); }}>
      <p>Excluir o perfil <strong>{deleteTarget.name}</strong>? As associações e ações deste perfil serão removidas.</p>
      {error && <p role="alert">{error}</p>}
      <div className="button-row modal-actions">
        <button type="button" data-cancel-delete disabled={busy} onClick={() => { setDeleteId(null); setError(""); }}>Cancelar</button>
        <button type="button" className="danger-button" disabled={busy} onClick={() => confirmDelete(deleteTarget.id)}>{busy ? "Excluindo…" : "Confirmar exclusão"}</button>
      </div>
    </Modal>}
    {message && <p role="status">{message}</p>}
    {error && !draft && !deleteId && <p role="alert">{error}</p>}
  </section>;
}
