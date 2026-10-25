import type { Binding, Config, Profile } from "./profiles";
export const mouseButtons = [
  { code: 1, name: "Botão esquerdo", x: 36, y: 23 },
  { code: 3, name: "Botão direito", x: 64, y: 23 },
  { code: 2, name: "Roda (clique)", x: 50, y: 22 },
  { code: 8, name: "Botão lateral 2", x: 19, y: 41 },
  { code: 9, name: "Botão lateral 1", x: 19, y: 56 },
];
// Symbolic button IDs follow the existing core convention; scroll is not a button.
export function mouseControls(count = 5) {
  return Array.from({ length: count }, (_, i) => mouseButtons[i] ?? { code: i + 5, name: `Botão extra ${i - 4}`, x: 78 + (i % 2) * 10, y: 14 + ((i - 5) % 9) * 8 });
}
export type MouseAction = Binding["action"];
export function mouseBinding(bindings: Binding[], scope: string | null, button: number, effective = false): Binding | undefined {
  const matches = (b: Binding) => "MouseButton" in b.input && b.input.MouseButton === button;
  return bindings.find(b => b.device_id === scope && matches(b))
    ?? (effective && scope !== null ? bindings.find(b => b.device_id === null && matches(b)) : undefined);
}
export function actionLabel(action?: MouseAction): string {
  if (!action) return "Comportamento original";
  if (action === "Disabled") return "Desativado";
  if ("KeyChord" in action) return action.KeyChord.join(" + ");
  return mouseButtons.find(b => b.code === action.MouseButton)?.name ?? `Botão ${action.MouseButton}`;
}
export function keyChord(value: string): MouseAction {
  const keys = value.split("+").map(k => k.trim());
  if (!keys.length || keys.length > 16 || keys.some(k => !k || new TextEncoder().encode(k).length > 128 || /[\x00-\x1f\x7f-\x9f]/.test(k))) {
    throw new Error("Informe de 1 a 16 teclas, separadas por +. Exemplo: Ctrl + V.");
  }
  if (new Set(keys.map(k => k.toLowerCase())).size !== keys.length) throw new Error("O atalho não pode repetir a mesma tecla.");
  return { KeyChord: keys };
}
export function setMouseBinding(bindings: Binding[], scope: string | null, button: number, action: MouseAction | null): Binding[] {
  const matches = (b: Binding) => b.device_id === scope && "MouseButton" in b.input && b.input.MouseButton === button;
  const current = bindings.filter(matches);
  if (action === null && current.length === 0 || action !== null && current.length === 1 && JSON.stringify(current[0].action) === JSON.stringify(action)) return bindings;
  const next = bindings.filter(b => !matches(b));
  if (action !== null) next.push({ device_id: scope, input: { MouseButton: button }, action });
  return next;
}
export function configWithBindings(config: Config, profile: Profile, baseRevision: number): Config {
  if (config.revision !== baseRevision) throw new Error("Os perfis mudaram desde o início da edição. Descarte as alterações e abra o editor novamente.");
  if (!config.profiles.some(p => p.id === profile.id)) throw new Error("O perfil foi removido. Selecione outro perfil.");
  return { ...config, profiles: config.profiles.map(p => p.id === profile.id ? { ...p, bindings: profile.bindings } : p) };
}

export type MouseActionDraft = { kind: "original" | "chord" | "mouse" | "disabled"; keys: string; target: number };
export function mouseActionDraft(action?: MouseAction): MouseActionDraft {
  return { kind: !action ? "original" : action === "Disabled" ? "disabled" : "KeyChord" in action ? "chord" : "mouse",
    keys: action && action !== "Disabled" && "KeyChord" in action ? action.KeyChord.join(" + ") : "",
    target: action && action !== "Disabled" && "MouseButton" in action ? action.MouseButton : 1 };
}
export function draftMouseAction(draft: MouseActionDraft): MouseAction | null {
  return draft.kind === "original" ? null : draft.kind === "disabled" ? "Disabled" : draft.kind === "mouse" ? { MouseButton: draft.target } : keyChord(draft.keys);
}
export function mouseDraftBindings(bindings: Binding[], scope: string | null, drafts: Record<number, MouseActionDraft>): Binding[] {
  return Object.entries(drafts).reduce((next, [code, draft]) => setMouseBinding(next, scope, Number(code), draftMouseAction(draft)), bindings);
}
export function mouseDraftChanged(bindings: Binding[], scope: string | null, drafts: Record<number, MouseActionDraft>): boolean {
  return Object.entries(drafts).some(([code, draft]) => {
    try { return JSON.stringify(draftMouseAction(draft)) !== JSON.stringify(mouseBinding(bindings, scope, Number(code))?.action ?? null); }
    catch { return true; }
  });
}
