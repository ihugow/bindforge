import { AppButton } from "./app-button";
import { invoke } from "@tauri-apps/api/core";
import { ProfileCover } from "./profile-cover";
import { steamApps, coverFor, type FindSteamCovers, type SteamCover } from "./steam-covers";
import { AppSelect } from "./app-select";
import React, { useCallback, useEffect, useRef, useState } from "react";
import { profileApi as nativeApi } from "./profile-api";
import { ProfileIcon, readProfileIcon } from "./profile-icon";
import { ApplicationIconPicker } from "./application-icon-picker";
import type { FindApplicationIcons } from "./application-icons";
import { ProfileCard } from "./profile-card";
import { Modal } from "./modal";
import { WindowCapturePanel } from "./window-capture-panel";
import type { CaptureStart } from "./window-capture";
import { appIds, conflictingProfiles, removeProfile, updateProfile, type Config, type Draft, type Profile } from "./profiles";

const findLocalCovers: FindSteamCovers = appIds => invoke("find_steam_covers", { appIds });

export function ProfilesPanel({ native, connected, onSaved, api = nativeApi, captureStart, onConfigChange, active = true, currentConfig, findIcons, findCovers = findLocalCovers }: {
  findIcons?: FindApplicationIcons; findCovers?: FindSteamCovers;
  currentConfig?: Config | null; active?: boolean; native: boolean; connected: boolean; onSaved: () => void; api?: typeof nativeApi; captureStart?: CaptureStart; onConfigChange?: (config: Config | null) => void;
}) {
  const editorTrigger = useRef<HTMLElement | null>(null);
  const captureTrigger = useRef<HTMLElement | null>(null);
  const deleteTrigger = useRef<HTMLElement | null>(null);
  const [config, setConfig] = useState<Config | null>(null);
  useEffect(() => { if (currentConfig) setConfig(currentConfig); }, [currentConfig]);
  const [draft, setDraft] = useState<Draft | null>(null);
  const [coverRefresh, setCoverRefresh] = useState(0);
  const [covers, setCovers] = useState<SteamCover[]>([]);
  const coverIds = JSON.stringify(steamApps([
    ...(config?.profiles.flatMap(profile => profile.id === config.fallback_id ? [] : profile.rules) ?? []),
    ...appIds(draft?.apps ?? "").map(app_id => ({ app_id })),
  ]));
  useEffect(() => {
    if (!active) return;
    let current = true;
    const ids: string[] = JSON.parse(coverIds);
    if (!ids.length) { setCovers([]); return; }
    // Batch IPC requests and ignore responses from an obsolete association/editor.
    const timer = setTimeout(() => {
      const batches = Array.from({ length: Math.ceil(ids.length / 256) }, (_, i) => ids.slice(i * 256, (i + 1) * 256));
      Promise.all(batches.map(ids => findCovers(ids))).then(results => {
        if (current) setCovers(results.flat());
      }).catch(() => { if (current) setCovers([]); });
    }, 150);
    return () => { current = false; clearTimeout(timer); };
  }, [coverIds, findCovers, active, coverRefresh]);
  const [deleteId, setDeleteId] = useState<string | null>(null);
  const [captureOpen, setCaptureOpen] = useState(false);
  const [capturePending, setCapturePending] = useState(false);
  // Preserve the draft across tabs, but never keep a hidden window capture armed.
  useEffect(() => { if (!active) { setCaptureOpen(false); setDeleteId(null); } }, [active]);
  const [automaticIconApp, setAutomaticIconApp] = useState<string | null>(null);
  const [iconSearching, setIconSearching] = useState(false);
  const detected = useCallback((appId: string) => {
    setAutomaticIconApp(appId);
    setDraft(d => d ? { ...d, apps: [...new Set([...appIds(d.apps), appId])].join("\n") } : d);
  }, []);
  const [busy, setBusy] = useState(false);
  const [iconLoading, setIconLoading] = useState(false);
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
    try { const loaded = await api.load(); setConfig(loaded); onConfigChange?.(loaded); setDraft(null); setCoverRefresh(n => n + 1); }
    catch (e) { setError(String(e)); } finally { setBusy(false); }
  }
  function edit(profile?: Profile) {
    if (!config) return;
    editorTrigger.current = document.activeElement as HTMLElement;
    setError(""); setMessage(""); setCaptureOpen(false); setAutomaticIconApp(null);
    setDraft({ icon: profile?.icon ?? null, id: profile?.id ?? null, name: profile?.name ?? "", apps: profile?.rules.map(r => r.app_id).filter(Boolean).join("\n") ?? "", priority: String(profile?.priority ?? 10), enabled: profile?.enabled ?? true, source: config.fallback_id });
  }
  function patch(value: Partial<Draft>) { setDraft(d => d ? { ...d, ...value } : d); }
  async function chooseIcon(event: React.ChangeEvent<HTMLInputElement>) {
    const file = event.target.files?.[0];
    event.target.value = "";
    if (!file) return;
    setIconLoading(true); setError("");
    try { patch({ icon: await readProfileIcon(file) }); }
    catch (error) { setError(error instanceof Error ? error.message : String(error)); }
    finally { setIconLoading(false); }
  }
  async function save(event: React.FormEvent) {
    event.preventDefault(); if (!config || !draft || capturePending || iconLoading || iconSearching || deleteId) return;
    setBusy(true); setError(""); setMessage("");
    try {
      const next = updateProfile(config, draft, `profile-${crypto.randomUUID()}`);
      const saved = await api.save(next, config.revision);
      setConfig(saved); onConfigChange?.(saved); setDraft(null); onSaved();
      setMessage(connected ? "Perfis salvos. O daemon recarrega as alterações automaticamente." : "Perfis salvos. Serão carregados quando o daemon iniciar.");
    } catch (e) { setError(String(e)); } finally { setBusy(false); }
  }
  async function confirmDelete(id: string) {
    if (!config || busy || !draft || draft.id !== id || deleteId !== id || id === config.fallback_id) return;
    setBusy(true); setError(""); setMessage("");
    try {
      const name = config.profiles.find(p => p.id === id)?.name;
      const saved = await api.save(removeProfile(config, id), config.revision);
      setConfig(saved); onConfigChange?.(saved); onSaved();
      editorTrigger.current = document.getElementById("profiles-heading");
      deleteTrigger.current = editorTrigger.current;
      setDeleteId(null); setDraft(null);
      setMessage(`Perfil “${name}” excluído.${connected ? " O daemon reavalia automaticamente o perfil selecionado." : " A alteração será usada quando o daemon iniciar."}`);
    } catch (e) { setError(String(e)); } finally { setBusy(false); }
  }
  const deleteTarget = config?.profiles.find(p => p.id === deleteId);
  const fallback = !!draft?.id && draft.id === config?.fallback_id;
  const conflicts = config && draft ? conflictingProfiles(config, draft) : [];
  const selected = config?.profiles.find(p => p.id === draft?.id);
  const advanced = selected?.rules.some(r => r.executable !== null || r.title_contains !== null) ?? false;
  return <section className="profiles-panel" aria-labelledby="profiles-heading">
    <div className="workspace-page-heading"><div><h2 id="profiles-heading" tabIndex={-1}>Seus perfis</h2><p>Crie perfis para personalizar suas ações em cada aplicativo.</p></div>
      {native && <div className="workspace-page-actions"><AppButton type="button" className="secondary-button" disabled={busy || !!draft || !!deleteId} onClick={reload}>Recarregar lista</AppButton>
        <AppButton type="button" disabled={!config || busy || !!draft || !!deleteId} onClick={() => edit()}>+ Criar perfil</AppButton></div>}
    </div>
    {!native && <p>Abra o aplicativo nativo para criar e salvar perfis.</p>}
    {native && !config && !error && <p role="status">Carregando perfis…</p>}
    {config && <>
      <ul className="profile-list">{config.profiles.map(p => <li key={p.id}>
        <ProfileCard profile={p} image={p.id === config.fallback_id ? undefined : coverFor(p.rules, covers)}
          disabled={busy || !!draft || !!deleteId} onEdit={() => edit(p)} />
      </li>)}</ul>
      <p className="hint">O perfil Padrão é usado quando nenhum aplicativo corresponde. Cada perfil tem suas próprias ações, sem herança automática.</p></>}
    {active && draft && config && <Modal title={draft.id ? "Editar perfil" : "Criar perfil"} busy={busy || iconLoading || iconSearching || !!deleteId} initialFocus="#profile-name" restoreFocus={editorTrigger}
      onDismiss={() => { setDraft(null); setCaptureOpen(false); setError(""); }}>
    <form className="profile-form" onSubmit={save}>
      <fieldset disabled={busy || iconLoading || iconSearching || !!deleteId}>
        <label htmlFor="profile-name">Nome do perfil</label>
        <input id="profile-name" autoFocus required maxLength={128} value={draft.name} onChange={e => patch({ name: e.target.value })} />
        {!fallback && steamApps(appIds(draft.apps).map(app_id => ({ app_id }))).length > 0 && <div className="profile-cover-editor">
          <ProfileCover image={coverFor(appIds(draft.apps).map(app_id => ({ app_id })), covers)} icon={draft.icon} />
          <div><strong>Capa da Steam</strong><p className="hint">Usamos automaticamente a capa disponível na biblioteca local da Steam. Sem capa, exibimos o ícone do perfil.</p></div>
        </div>}
        <div className="profile-icon-editor"><ProfileIcon icon={draft.icon} /><div>
          <label htmlFor="profile-icon-file">Ícone do perfil</label>
          <input id="profile-icon-file" type="file" accept="image/png,image/jpeg,image/webp,image/x-icon,image/vnd.microsoft.icon,.ico" aria-describedby="profile-icon-hint" onChange={chooseIcon} />
          <p className="hint" id="profile-icon-hint">PNG, JPG, WebP ou ICO de até 5 MB. Sem imagem, usamos o ícone padrão.</p>
          {!fallback && <ApplicationIconPicker apps={draft.apps} automaticAppId={automaticIconApp} hasIcon={!!draft.icon || iconLoading}
            onSelect={icon => patch({ icon })} onPending={setIconSearching} find={findIcons} />}
          {draft.icon && <AppButton type="button" className="secondary-button" onClick={() => patch({ icon: null })}>Usar ícone padrão</AppButton>}
          {iconLoading && <p role="status">Preparando ícone…</p>}
        </div></div>
        {!fallback && <><label htmlFor="profile-apps">IDs dos aplicativos — um por linha</label>
          <textarea id="profile-apps" required={!advanced} rows={3} value={draft.apps} onChange={e => patch({ apps: e.target.value })} placeholder={"firefox\norg.kde.konsole"} disabled={advanced || busy} />
          <p className="hint">O ID deve corresponder exatamente ao informado pelo KDE. {advanced && "Este perfil tem regras avançadas; suas associações são preservadas nesta tela."}</p>
          {appIds(draft.apps).some(id => id.toLowerCase() === "gamescope") && <p role="status">A associação “gamescope” é genérica e não identifica um jogo específico. Use a detecção atualizada para obter um ID como steam:1808500.</p>}
          {!advanced && <><AppButton type="button" disabled={!connected || captureOpen} onClick={() => { captureTrigger.current = document.activeElement as HTMLElement; setCaptureOpen(true); }}>Detectar pela janela</AppButton>
            {!connected && <p className="hint">Conecte o daemon KDE para usar a detecção automática.</p>}
          </>}</>}
        {!draft.id && <><label htmlFor="profile-source">Como começar</label>
          <AppSelect id="profile-source" value={draft.source} onValueChange={value => patch({ source: value })}>
            <option value="">Sem remapeamentos</option>
            {config.profiles.map(p => <option key={p.id} value={p.id}>Copiar ações de {p.name}</option>)}
          </AppSelect><p className="hint">A cópia é independente. Alterações na origem não mudam este perfil.</p></>}
        {!fallback && <><label className="checkbox-label"><input type="checkbox" checked={draft.enabled} onChange={e => patch({ enabled: e.target.checked })} /> Perfil habilitado</label>
          <details open={conflicts.length > 0}><summary>Prioridade e conflitos</summary>
            <label htmlFor="profile-priority">Prioridade — o maior número vence</label>
            <input id="profile-priority" type="number" step={1} min={-2147483648} max={2147483647} required value={draft.priority} onChange={e => patch({ priority: e.target.value })} />
            {conflicts.length > 0 && <p role="status">Também associado a: {conflicts.map(p => `${p.name} (prioridade ${p.priority})`).join(", ")}. Em empate, vence a regra mais específica e depois o menor ID.</p>}
          </details></>}
        <p className="hint">As ações existentes são preservadas. Configure os botões e teclas em Dispositivos. Salvar um perfil não ativa o remapeamento.</p>
        {error && <p role="alert">{error}</p>}
        <div className="profile-editor-actions">
          <div className="button-row"><AppButton type="submit" disabled={capturePending}>{busy ? "Salvando…" : "Salvar alterações"}</AppButton><AppButton type="button" className="secondary-button" onClick={() => { setDraft(null); setCaptureOpen(false); setError(""); }}>Cancelar</AppButton></div>
          {draft.id && !fallback && <AppButton type="button" className="danger-button" disabled={capturePending || captureOpen} onClick={() => {
            deleteTrigger.current = document.activeElement as HTMLElement;
            setDeleteId(draft.id); setError(""); setMessage("");
          }}>Excluir perfil</AppButton>}
        </div>
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
        <AppButton type="button" data-cancel-delete disabled={busy} onClick={() => { setDeleteId(null); setError(""); }}>Cancelar</AppButton>
        <AppButton type="button" className="danger-button" disabled={busy} onClick={() => confirmDelete(deleteTarget.id)}>{busy ? "Excluindo…" : "Confirmar exclusão"}</AppButton>
      </div>
    </Modal>}
    {message && <p role="status">{message}</p>}
    {error && !draft && !deleteId && <p role="alert">{error}</p>}
  </section>;
}
