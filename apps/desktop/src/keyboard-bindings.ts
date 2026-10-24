import type { Binding } from "./profiles";

// Accept existing symbolic names while writing canonical physical codes for new bindings.
export function physicalKey(value: string): string {
  const token = value.trim().toUpperCase();
  const aliases: Record<string, string> = { CTRL: "LEFTCTRL", CONTROL: "LEFTCTRL", SHIFT: "LEFTSHIFT", ALT: "LEFTALT", ALTGR: "RIGHTALT", SUPER: "LEFTMETA", META: "LEFTMETA", WIN: "LEFTMETA", RETURN: "ENTER", ESCAPE: "ESC" };
  return token.startsWith("KEY_") ? token : `KEY_${aliases[token] ?? token}`;
}
export function keyboardBinding(bindings: Binding[], scope: string | null, code: string, effective = false): Binding | undefined {
  const matches = (b: Binding) => "Key" in b.input && physicalKey(b.input.Key) === physicalKey(code);
  return bindings.find(b => b.device_id === scope && matches(b))
    ?? (effective && scope !== null ? bindings.find(b => b.device_id === null && matches(b)) : undefined);
}
export function setKeyboardBinding(bindings: Binding[], scope: string, code: string, action: Binding["action"] | null): Binding[] {
  const next = bindings.filter(b => !(b.device_id === scope && "Key" in b.input && physicalKey(b.input.Key) === physicalKey(code)));
  if (action !== null) next.push({ device_id: scope, input: { Key: physicalKey(code) }, action });
  return next;
}
