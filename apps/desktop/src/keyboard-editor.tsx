import { AppButton } from "./app-button";
import { bindingScope } from "./configured-devices";
import { AppSelect } from "./app-select";
import { useEffect, useState, type KeyboardEvent } from "react";
import { Modal } from "./modal";
import { EditorSaveNotice } from "./editor-save-notice";
import { KeyboardViewport } from "./keyboard-viewport";
import { layouts, type ConfiguredDevice } from "./configured-devices";
import { keyboardSections, keyboardKeyGroups, keyName, type KeyboardKey } from "./keyboard-layouts";
import { keyboardBinding, physicalKey, setKeyboardBinding } from "./keyboard-bindings";
import { actionLabel, mouseButtons } from "./mouse-bindings";
import type { Binding, Config, Profile } from "./profiles";

const modifiers = [
  ["KEY_LEFTCTRL", "Ctrl esquerdo"], ["KEY_RIGHTCTRL", "Ctrl direito"],
  ["KEY_LEFTSHIFT", "Shift esquerdo"], ["KEY_RIGHTSHIFT", "Shift direito"],
  ["KEY_LEFTALT", "Alt esquerdo"], ["KEY_RIGHTALT", "AltGr / Alt direito"],
  ["KEY_LEFTMETA", "Super esquerdo"], ["KEY_RIGHTMETA", "Super direito"],
];

export function KeyboardEditor({ profile, revision, device, save, close, restoreFocus, inline = false, onDirtyChange }: {
  profile: Profile; revision: number; device: ConfiguredDevice;
  save: (profile: Profile, revision: number) => Promise<Config>;
  close: () => void; restoreFocus: React.RefObject<HTMLElement | null>;
  inline?: boolean; onDirtyChange?: (dirty: boolean, busy: boolean) => void;
}) {
  const [bindings, setBindings] = useState(() => structuredClone(profile.bindings));
  const [selected, setSelected] = useState("KEY_CAPSLOCK");
  const [focused, setFocused] = useState("KEY_CAPSLOCK");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [discard, setDiscard] = useState(false);
  const layout = device.layout ?? "abnt2";
  const sections = keyboardSections(layout);
  const keys = sections.flatMap(section => section.keys);
  const scope = bindingScope(device);
  const exact = keyboardBinding(bindings, scope, selected);
  const effective = keyboardBinding(bindings, scope, selected, true);
  const dirty = JSON.stringify(bindings) !== JSON.stringify(profile.bindings);
  useEffect(() => { onDirtyChange?.(dirty, busy); }, [dirty, busy, onDirtyChange]);
  const selectedKey = keys.find(key => key.code === selected);
  const label = (code: string) => {
    const key = keys.find(key => key.code === physicalKey(code));
    return key ? keyName(key) : code;
  };
  const describe = (action?: Binding["action"]) => action && action !== "Disabled" && "KeyChord" in action ? action.KeyChord.map(label).join(" + ") : actionLabel(action);
  const kind = !exact ? "original" : exact.action === "Disabled" ? "disabled" : "KeyChord" in exact.action ? "chord" : "mouse";
  const chord = exact && exact.action !== "Disabled" && "KeyChord" in exact.action ? exact.action.KeyChord : [];
  const target = chord.at(-1) ?? "KEY_ESC";
  const prefixes = chord.slice(0, -1);
  const mouseTarget = exact && exact.action !== "Disabled" && "MouseButton" in exact.action ? exact.action.MouseButton : 1;
  function update(action: Binding["action"] | null) {
    setBindings(current => setKeyboardBinding(current, scope, selected, action)); setError("");
  }
  function renderModifier(code: string, name: string) {
    return <label key={code}>
      <input type="checkbox" disabled={physicalKey(target) === code} checked={prefixes.some(k => physicalKey(k) === code)} onChange={e => update({ KeyChord: [...prefixes.filter(k => physicalKey(k) !== code), ...(e.target.checked ? [code] : []), target] })} />{name}
    </label>;
  }
  function dismiss() { if (dirty) setDiscard(true); else close(); }
  async function persist() {
    setBusy(true); setError("");
    try { await save({ ...profile, bindings }, revision); if (!inline) close(); }
    catch (e) { setError(String(e)); } finally { setBusy(false); }
  }
  function navigate(event: KeyboardEvent<HTMLButtonElement>) {
    if (!["ArrowLeft", "ArrowRight", "ArrowUp", "ArrowDown"].includes(event.key)) return;
    event.preventDefault();
    const origin = event.currentTarget.getBoundingClientRect();
    const horizontal = event.key === "ArrowLeft" || event.key === "ArrowRight";
    const sign = event.key === "ArrowLeft" || event.key === "ArrowUp" ? -1 : 1;
    const buttons = event.currentTarget.closest(".keyboard-board")?.querySelectorAll<HTMLButtonElement>(".keyboard-key:enabled");
    let best: HTMLButtonElement | undefined, score = Infinity;
    buttons?.forEach(button => {
      const rect = button.getBoundingClientRect();
      const dx = rect.x + rect.width / 2 - origin.x - origin.width / 2;
      const dy = rect.y + rect.height / 2 - origin.y - origin.height / 2;
      const along = (horizontal ? dx : dy) * sign;
      const across = Math.abs(horizontal ? dy : dx);
      if (along > 1 && along + across * 4 < score) { best = button; score = along + across * 4; }
    });
    best?.focus();
  }
  function renderKey(key: KeyboardKey) {
    const binding = keyboardBinding(bindings, scope, key.code, true);
    return <AppButton type="button" key={key.code} disabled={busy} data-key-code={key.code}
      className={`keyboard-key${binding ? " is-mapped" : ""}${selected === key.code ? " is-selected" : ""}${key.isoEnter ? " keyboard-enter-iso" : ""}`}
      style={{ gridColumn: `${key.x * 4 + 1} / span ${key.width * 4}`, gridRow: `${key.y + 1} / span ${key.height}` }}
      tabIndex={focused === key.code ? 0 : -1} onFocus={() => setFocused(key.code)} onKeyDown={navigate}
      aria-pressed={selected === key.code} aria-label={`${keyName(key)}: ${describe(binding?.action)}`}
      title={`${keyName(key)} · ${describe(binding?.action)}`} onClick={() => setSelected(key.code)}>
      <span>{key.label}</span>{binding && <span className="key-mapping-dot" aria-hidden="true" />}
    </AppButton>;
  }
  const content = <>
    <div className={inline ? "device-editor-grid keyboard-inline-grid" : ""}><div className="editor-canvas">
    {!inline && <div className="editor-summary keyboard-summary"><span className="status-chip">Editor de teclado</span><span role="status">{dirty ? "Alterações não salvas" : "Configuração salva"}</span></div>}
    <div className="keyboard-heading"><div><span className="eyebrow">LAYOUT FÍSICO</span><h3>{layouts[layout]}</h3></div><span className="keyboard-key-count">{keys.length} teclas · modelo completo</span></div>
    {layout === "iso" && <p className="hint">ISO define o formato físico; as legendas variam conforme o idioma. O layout do sistema não é alterado.</p>}
    {!inline && <p className="hint" id="keyboard-instructions">Selecione uma tecla no desenho e escolha sua ação no painel. Use também as setas e Enter para selecionar.</p>}
    <KeyboardViewport label={`Teclado ${layout.toUpperCase()}`} descriptionId={inline ? undefined : "keyboard-instructions"}>
      <div className="keyboard-chassis"><div className="keyboard-board">{sections.map(section => <div key={section.id} className={`keyboard-section keyboard-section-${section.id}`} role="group" aria-label={section.name}
        style={{ gridTemplateColumns: `repeat(${section.width * 4}, minmax(0, 1fr))` }}>{section.keys.map(renderKey)}</div>)}</div></div>
    </KeyboardViewport>
    <div className="keyboard-legend"><span><i className="legend-selected" />Selecionada</span><span><i className="legend-mapped" />Com remapeamento</span><span className="hint">Em telas menores, deslize o desenho para ver todas as teclas.</span></div>

    </div><div className="keyboard-action-panel action-sidebar">
      <div className="keyboard-selected"><span className="eyebrow">TECLA SELECIONADA</span><strong>{selectedKey ? keyName(selectedKey) : selected}</strong><span className="keyboard-current">{describe(effective?.action)}{!exact && effective ? " · ação geral" : ""}</span></div>
      <fieldset className="keyboard-fields" disabled={busy}>
        <legend className="sr-only">Remapeamento da tecla selecionada</legend>
        <label htmlFor="keyboard-action">Ação da tecla</label>
        <AppSelect id="keyboard-action" className="app-select" value={kind} onValueChange={value => update(value === "original" ? null : value === "disabled" ? "Disabled" : value === "mouse" ? { MouseButton: 1 } : { KeyChord: ["KEY_ESC"] })}>
          <option value="original">Original / ação geral</option><option value="chord">Tecla ou atalho de teclado</option><option value="disabled">Desativar tecla</option><option value="mouse">Botão do mouse</option>
        </AppSelect>
        {kind === "chord" && <>
          <label htmlFor="keyboard-target">Remapear para</label>
          <AppSelect id="keyboard-target" className="app-select" menuLayout="key-grid" value={target} onValueChange={value => update({ KeyChord: [...prefixes.filter(k => physicalKey(k) !== value), value] })}>
            {!keys.some(k => k.code === target) && <optgroup label="Outras teclas"><option value={target}>{label(target)}</option></optgroup>}
            {keyboardKeyGroups(layout).filter(group => group.name !== "Modificadores").map(group => <optgroup key={group.name} label={group.name}>{group.keys.map(key => <option key={key.code} value={key.code}>{keyName(key)}</option>)}</optgroup>)}
          </AppSelect>
          <fieldset className="keyboard-modifiers"><legend>Combinar com</legend>
            {[["LEFT", "Esquerdo"], ["RIGHT", "Direito"]].map(([side, heading]) => <fieldset key={side} className="keyboard-modifier-column">
              <legend>{heading}</legend>
              {modifiers.filter(([code]) => code.includes(side)).map(([code, name]) => renderModifier(code, name.split(" ")[0]))}
            </fieldset>)}
            {prefixes.filter(code => !modifiers.some(([m]) => m === physicalKey(code))).map(code => <div key={code} className="keyboard-modifier-extra">{renderModifier(code, label(code))}</div>)}
          </fieldset>
        </>}
        {kind === "mouse" && <><label htmlFor="keyboard-mouse-target">Botão de destino</label><AppSelect id="keyboard-mouse-target" className="app-select" value={mouseTarget} onValueChange={value => update({ MouseButton: Number(value) })}>
          {!mouseButtons.some(b => b.code === mouseTarget) && <option value={mouseTarget}>Botão {mouseTarget}</option>}
          {mouseButtons.map(button => <option key={button.code} value={button.code}>{button.name}</option>)}
        </AppSelect></>}
        {kind === "original" && <p className="hint">Sem personalização neste teclado. Se houver uma ação geral para esta tecla, ela é mantida.</p>}
        {kind === "disabled" && <p className="hint">A tecla ficará sem ação enquanto o remapeamento deste teclado estiver ativo.</p>}
      </fieldset>
    </div>

    </div>
    <details className="editor-controls-list"><summary>Ativação e atalho de emergência</summary><p className="keyboard-availability">Salvar altera os mapeamentos deste teclado no perfil em edição. Use Ativar no dispositivo para iniciar o remapeamento. Para pausar por emergência: Ctrl + Alt + Pause no teclado físico.</p></details>
    {error && <p role="alert">{error}</p>}
    {inline ? <EditorSaveNotice dirty={dirty} busy={busy} discard={() => { setBindings(structuredClone(profile.bindings)); setError(""); }} save={() => void persist()} /> : <footer className="editor-savebar"><span role="status">{dirty ? "Alterações não salvas" : "Todas as alterações salvas"}</span><div className="button-row"><AppButton type="button" className="secondary-button" disabled={busy || inline && !dirty} onClick={inline ? () => { setBindings(structuredClone(profile.bindings)); setError(""); } : dismiss}>{inline ? "Descartar" : "Fechar"}</AppButton><AppButton type="button" disabled={busy || !dirty} onClick={persist}>{busy ? "Salvando…" : "Salvar alterações"}</AppButton></div></footer>}
    {discard && <Modal title="Descartar alterações?" compact onDismiss={() => setDiscard(false)} initialFocus="[data-keep-keyboard]">
      <p>Os mapeamentos deste teclado ainda não foram salvos.</p><div className="button-row modal-actions"><AppButton type="button" data-keep-keyboard onClick={() => setDiscard(false)}>Continuar editando</AppButton><AppButton type="button" className="danger-button" onClick={close}>Descartar e fechar</AppButton></div>
    </Modal>}
  </>;
  return inline ? <div className="device-editor-body">{content}</div> : <Modal wide title={`${device.name} · ${profile.name}`} onDismiss={dismiss} busy={busy} restoreFocus={restoreFocus} initialFocus='[data-key-code="KEY_CAPSLOCK"]'>{content}</Modal>;
}
