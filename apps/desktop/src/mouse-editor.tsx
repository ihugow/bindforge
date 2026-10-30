import { AppButton } from "./app-button";
import { bindingScope } from "./configured-devices";
import { AppSelect } from "./app-select";
import { useRef, useState } from "react";
import { Modal } from "./modal";
import { MouseArt } from "./device-art";
import { actionLabel, keyChord, mouseBinding, mouseButtons, mouseControls, setMouseBinding, type MouseAction } from "./mouse-bindings";
import type { Binding, Config, Profile } from "./profiles";
import type { ConfiguredDevice } from "./configured-devices";

export function MouseEditor({ profile, revision, device, simulation = true, save, close, restoreFocus }: {
  simulation?: boolean; device?: ConfiguredDevice;
  profile: Profile; revision: number; save: (profile: Profile, revision: number) => Promise<Config>;
  close: () => void; restoreFocus: React.RefObject<HTMLElement | null>;
}) {
  const [bindings, setBindings] = useState<Binding[]>(() => structuredClone(profile.bindings));
  const [control, setControl] = useState<number | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [discard, setDiscard] = useState(false);
  const buttonTrigger = useRef<HTMLElement | null>(null);
  const scopeId = device ? bindingScope(device) : null;
  const dirty = JSON.stringify(bindings) !== JSON.stringify(profile.bindings);
  const controls = mouseControls(device?.button_count ?? 5);
  function dismiss() { if (dirty) setDiscard(true); else close(); }
  function openControl(code: number) { buttonTrigger.current = document.activeElement as HTMLElement; setControl(code); }
  async function persist() {
    setBusy(true); setError("");
    try { await save({ ...profile, bindings }, revision); close(); }
    catch (e) { setError(String(e)); } finally { setBusy(false); }
  }
  return <Modal title={`${device?.name ?? "Ações gerais do mouse"} · ${profile.name}`} onDismiss={dismiss} busy={busy} restoreFocus={restoreFocus} initialFocus=".mouse-hotspot">
    <div className="editor-summary"><span className="status-chip">{simulation ? "Remapeamento pausado" : "Remapeamento físico ativo"}</span><span>{dirty ? "Alterações não salvas" : "Configuração salva"}</span></div>
    <p className="hint">Clique em um controle no desenho ou na lista. Ações específicas substituem as gerais para o mesmo botão.</p>
    <div className="mouse-workspace">
      <div className="mouse-diagram"><MouseArt />{controls.slice(0, 5).map(b => {
        const binding = mouseBinding(bindings, scopeId, b.code, true);
        return <AppButton key={b.code} type="button" className={`mouse-hotspot${binding ? " is-mapped" : ""}`} style={{ left: `${b.x}%`, top: `${b.y}%` }}
          aria-label={`Configurar ${b.name}`} title={`${b.name}: ${actionLabel(binding?.action)}`} disabled={busy} onClick={() => openControl(b.code)}>{b.code}</AppButton>;
      })}</div>
      <ul className="mapping-list">{controls.map(b => {
        const exact = mouseBinding(bindings, scopeId, b.code);
        const effective = mouseBinding(bindings, scopeId, b.code, true);
        return <li key={b.code}><AppButton type="button" disabled={busy} onClick={() => openControl(b.code)}>
          <span><strong>{b.name}</strong><small>{actionLabel(effective?.action)}{scopeId && !exact && effective ? " · ação geral" : ""}</small></span><span className="control-number">{b.code}</span>
        </AppButton></li>;
      })}</ul>
    </div>
    <p className="hint">Ilustração genérica. A lista apresenta {controls.length} controles; a posição real varia conforme o modelo. Ações de outros botões já salvas são preservadas.</p>
    {error && <p role="alert">{error}</p>}
    <div className="button-row modal-actions"><AppButton type="button" disabled={busy} onClick={dismiss}>Fechar</AppButton>
      <AppButton type="button" disabled={busy || !dirty} onClick={persist}>{busy ? "Salvando…" : "Salvar alterações"}</AppButton></div>
    {control !== null && <ActionEditor key={`${scopeId}-${control}`} name={controls.find(b => b.code === control)?.name ?? `Botão ${control}`} scope={scopeId} action={mouseBinding(bindings, scopeId, control)?.action}
      close={() => setControl(null)} restoreFocus={buttonTrigger} apply={action => { setBindings(current => setMouseBinding(current, scopeId, control, action)); setControl(null); setError(""); }} />}
    {discard && <Modal title="Descartar alterações?" compact onDismiss={() => setDiscard(false)} initialFocus="[data-keep-editing]">
      <p>Os mapeamentos deste perfil ainda não foram salvos.</p><div className="button-row modal-actions">
        <AppButton type="button" data-keep-editing onClick={() => setDiscard(false)}>Continuar editando</AppButton><AppButton type="button" className="danger-button" onClick={close}>Descartar e fechar</AppButton>
      </div>
    </Modal>}
  </Modal>;
}
function ActionEditor({ name, scope, action, apply, close, restoreFocus }: {
  name: string; scope: string | null; action?: MouseAction; apply: (action: MouseAction | null) => void; close: () => void; restoreFocus: React.RefObject<HTMLElement | null>;
}) {
  const [kind, setKind] = useState(action === undefined ? "original" : action === "Disabled" ? "disabled" : "KeyChord" in action ? "chord" : "mouse");
  const [keys, setKeys] = useState(action && action !== "Disabled" && "KeyChord" in action ? action.KeyChord.join(" + ") : "");
  const [target, setTarget] = useState(action && action !== "Disabled" && "MouseButton" in action ? action.MouseButton : 1);
  const [error, setError] = useState("");
  return <Modal title={name} onDismiss={close} compact restoreFocus={restoreFocus} initialFocus="#action-kind">
    <form onSubmit={event => { event.preventDefault(); try { apply(kind === "original" ? null : kind === "disabled" ? "Disabled" : kind === "mouse" ? { MouseButton: target } : keyChord(keys)); } catch (e) { setError(String(e)); } }}>
      <label htmlFor="action-kind">Ação</label><AppSelect id="action-kind" className="app-select" value={kind} onValueChange={value => setKind(value)}>
        <option value="original">{scope ? "Usar comportamento geral" : "Comportamento original"}</option><option value="chord">Tecla ou atalho de teclado</option><option value="mouse">Outro botão do mouse</option><option value="disabled">Desativar botão</option>
      </AppSelect>
      {kind === "chord" && <><label htmlFor="action-keys">Tecla ou combinação</label><input id="action-keys" value={keys} required placeholder="V ou Ctrl + Shift + V" onChange={e => setKeys(e.target.value)} /><p className="hint">Separe as teclas com +. Exemplos: V, Ctrl + W, Alt + Left.</p></>}
      {kind === "mouse" && <><label htmlFor="action-target">Botão de destino</label><AppSelect id="action-target" className="app-select" value={target} onValueChange={value => setTarget(Number(value))}>
        {!mouseButtons.some(b => b.code === target) && <option value={target}>Botão {target}</option>}{mouseButtons.map(b => <option key={b.code} value={b.code}>{b.name}</option>)}
      </AppSelect></>}
      {kind === "original" && <p className="hint">{scope ? "Remove a personalização deste dispositivo. Uma ação geral, se existir, continua valendo." : "Remove o mapeamento e preserva a função original do botão."}</p>}
      {error && <p role="alert">{error}</p>}
      <div className="button-row modal-actions"><AppButton type="button" onClick={close}>Cancelar</AppButton><AppButton type="submit">Aplicar ao rascunho</AppButton></div>
    </form>
  </Modal>;
}
