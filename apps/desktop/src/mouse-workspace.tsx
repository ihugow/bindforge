import { AppButton } from "./app-button";
import { bindingScope } from "./configured-devices";
import { AppSelect } from "./app-select";
import { useEffect, useState } from "react";
import { MouseArt } from "./device-art";
import { EditorSaveNotice } from "./editor-save-notice";
import { mouseActionDraft, draftMouseAction, mouseDraftBindings, mouseDraftChanged, type MouseActionDraft } from "./mouse-bindings";
import { actionLabel, mouseBinding, mouseButtons, mouseControls } from "./mouse-bindings";
import type { ConfiguredDevice } from "./configured-devices";
import type { Config, Profile } from "./profiles";

export function MouseWorkspace({ profile, revision, device, save, onDirtyChange }: {
  profile: Profile; revision: number; device?: ConfiguredDevice;
  save: (profile: Profile, revision: number) => Promise<Config>;
  onDirtyChange: (dirty: boolean, busy: boolean) => void;
}) {
  const controls = mouseControls(device?.button_count ?? 5), scope = device ? bindingScope(device) : null;
  const [selected, setSelected] = useState(() => controls.find(c => c.code === 8)?.code ?? controls[0].code);
  const [drafts, setDrafts] = useState<Record<number, MouseActionDraft>>({});
  const [busy, setBusy] = useState(false), [error, setError] = useState("");
  const dirty = mouseDraftChanged(profile.bindings, scope, drafts);
  const current = drafts[selected] ?? mouseActionDraft(mouseBinding(profile.bindings, scope, selected)?.action);
  useEffect(() => onDirtyChange(dirty, busy), [dirty, busy, onDirtyChange]);
  function patch(change: Partial<MouseActionDraft>) { setDrafts(values => ({ ...values, [selected]: { ...current, ...change } })); setError(""); }
  function description(code: number) {
    const raw = drafts[code];
    if (raw) { try { const action = draftMouseAction(raw); if (action) return actionLabel(action); } catch { return "Atalho incompleto"; } }
    return actionLabel(raw && scope === null ? undefined : mouseBinding(profile.bindings, raw ? null : scope, code, true)?.action);
  }
  async function persist() {
    setError("");
    let bindings;
    try { bindings = mouseDraftBindings(profile.bindings, scope, drafts); }
    catch (e) { setError(String(e)); return; }
    setBusy(true);
    try { await save({ ...profile, bindings }, revision); setDrafts({}); }
    catch (e) { setError(String(e)); } finally { setBusy(false); }
  }
  return <div className="device-editor-body">
    <div className="device-editor-grid">
      <div className="editor-canvas"><div className="mouse-workspace-stage"><div className="mouse-diagram mouse-assignment-diagram"><MouseArt />
        <svg className="mouse-assignment-lines" viewBox="0 0 100 100" preserveAspectRatio="none" aria-hidden="true">
          {controls.slice(0, 5).map(b => <line key={b.code} x1={25 + b.x / 2} y1={b.y} x2={b.code === 3 || b.code === 2 ? 76 : 24} y2={b.code === 2 ? 45 : b.y} className={selected === b.code ? "is-selected" : undefined} />)}
        </svg>
        {controls.slice(0, 5).map(b => {
          const label = description(b.code), active = selected === b.code;
          return <div key={b.code} className={`mouse-assignment${active ? " is-selected" : ""}`}>
            <AppButton type="button" className={`mouse-hotspot${active ? " is-selected" : ""}`} style={{ left: `${25 + b.x / 2}%`, top: `${b.y}%` }}
              aria-label={`Configurar ${b.name}: ${label}`} aria-pressed={active} disabled={busy} onClick={() => setSelected(b.code)}>{b.code}</AppButton>
            <AppButton type="button" className="mouse-assignment-label" style={{ left: b.code === 3 || b.code === 2 ? "76%" : "0%", top: `${b.code === 2 ? 45 : b.y}%` }}
              aria-label={`Configurar ${b.name}: ${label}`} aria-pressed={active} disabled={busy} onClick={() => setSelected(b.code)}>
              <small>{b.name}</small><strong>{label}</strong>
            </AppButton>
          </div>;
        })}</div></div>
        {controls.length > 5 && <details className="editor-controls-list"><summary>Mais botões ({controls.length - 5})</summary><ul className="mapping-list">{controls.slice(5).map(b => <li key={b.code}><AppButton type="button" disabled={busy} aria-pressed={selected === b.code} onClick={() => setSelected(b.code)}><span><strong>{b.name}</strong><small>{description(b.code)}</small></span><span>{b.code}</span></AppButton></li>)}</ul></details>}
      </div>
      <section className="action-sidebar" aria-labelledby="mouse-action-title"><span className="eyebrow">AÇÃO SELECIONADA</span><h3 id="mouse-action-title">{controls.find(b => b.code === selected)?.name ?? `Botão ${selected}`}</h3>
        <fieldset disabled={busy}><label htmlFor="workspace-action-kind">Comportamento</label><AppSelect id="workspace-action-kind" value={current.kind} onValueChange={value => patch({ kind: value as MouseActionDraft["kind"] })}>
          <option value="original">{scope ? "Original / ação geral" : "Comportamento original"}</option><option value="chord">Tecla ou atalho</option><option value="mouse">Outro botão do mouse</option><option value="disabled">Desativar botão</option>
        </AppSelect>
        {current.kind === "chord" && <><label htmlFor="workspace-action-keys">Tecla ou combinação</label><input id="workspace-action-keys" value={current.keys} onChange={e => patch({ keys: e.target.value })} placeholder="Ctrl + Shift + V" /><p className="hint">Exemplos: V, Esc ou Ctrl + C.</p></>}
        {current.kind === "mouse" && <><label htmlFor="workspace-action-target">Botão de destino</label><AppSelect id="workspace-action-target" value={current.target} onValueChange={value => patch({ target: Number(value) })}>
          {!mouseButtons.some(b => b.code === current.target) && <option value={current.target}>Botão {current.target}</option>}{mouseButtons.map(b => <option key={b.code} value={b.code}>{b.name}</option>)}
        </AppSelect></>}
        {current.kind === "original" && <p className="hint">{scope ? "Remove a personalização deste dispositivo. Uma ação geral existente continua valendo." : "Preserva a função original do botão."}</p>}
        {current.kind === "disabled" && <p className="hint">Este botão ficará sem ação neste perfil.</p>}</fieldset>
        <div className="action-scope">{scope ? "Personalização deste dispositivo." : "Ações gerais para todos os mouses."}<br />Perfil <strong>{profile.name}</strong>.</div>
      </section>
    </div>
    {error && <p role="alert">{error}</p>}
    <EditorSaveNotice dirty={dirty} busy={busy} discard={() => { setDrafts({}); setError(""); }} save={() => void persist()} />
  </div>;
}
