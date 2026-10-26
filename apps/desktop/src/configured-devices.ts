import type { Config } from "./profiles";
import type { DetectedDevice } from "./devices";
export type ConfiguredDevice = {
  id: string; name: string; kind: "mouse" | "keyboard"; interface_id: string;
  connections?: Connection[];
  button_count: number | null; layout: "abnt2" | "ansi" | "iso" | null;
};
export type Connection = { interface_id: string; name: string; interface_name?: string | null; fingerprint?: string | null };
export function connectionsOf(device: ConfiguredDevice): Connection[] {
  return device.connections?.length ? device.connections : [{ interface_id: device.interface_id, name: "Conexão principal" }];
}
export function bindingScope(device: ConfiguredDevice) { return device.connections?.length ? `device:${device.id}` : device.interface_id; }
export function connectedInterface(device: ConfiguredDevice, inventory: DetectedDevice[]) {
  return connectionsOf(device).flatMap(c => {
    const matches = inventory.filter(i => compatible(i, device.kind) && (c.fingerprint ? (device.kind === "mouse" ? i.mouse_fingerprint : i.keyboard_fingerprint) === c.fingerprint : i.id === c.interface_id));
    return matches.length === 1 ? matches : [];
  })[0];
}
export const layouts = { abnt2: "Português brasileiro · ABNT2", ansi: "ANSI · padrão americano", iso: "ISO · padrão europeu" };
export function compatible(device: DetectedDevice, kind: ConfiguredDevice["kind"]) { return device.kind === kind || device.kind === "composite"; }
export function withDevice(config: Config, device: ConfiguredDevice, inventory: DetectedDevice[]): Config {
  const name = device.name.trim();
  if (!name || new TextEncoder().encode(name).length > 128 || /[\x00-\x1f\x7f-\x9f]/.test(name)) throw new Error("Informe um nome de até 128 bytes, sem caracteres de controle.");
  const devices = config.devices ?? [];
  const previous = devices.find(d => d.id === device.id);
  if (!previous && devices.length >= 64) throw new Error("Limite de 64 dispositivos atingido.");
  const connections = connectionsOf(device);
  if (!connections.length || connections.length > 8 || new Set(connections.map(c => c.interface_id)).size !== connections.length) throw new Error("Use de uma a oito conexões distintas.");
  const fingerprints = connections.flatMap(c => c.fingerprint ? [c.fingerprint] : []);
  if (new Set(fingerprints).size !== fingerprints.length) throw new Error("Esta conexão já está vinculada ao dispositivo.");
  for (const c of connections) {
    const detected = inventory.find(d => d.id === c.interface_id);
    const saved = previous && connectionsOf(previous).find(old => old.interface_id === c.interface_id && old.fingerprint === c.fingerprint);
    if (!saved && (!detected || !compatible(detected, device.kind))) throw new Error("Selecione uma interface compatível identificada na lista.");
    if (!c.name.trim() || new TextEncoder().encode(c.name).length > 128 || /[\x00-\x1f\x7f-\x9f]/.test(c.name)) throw new Error("Informe um nome válido para a conexão.");
    if (devices.some(d => d.id !== device.id && d.kind === device.kind && connectionsOf(d).some(other => other.interface_id === c.interface_id || !!c.fingerprint && other.fingerprint === c.fingerprint))) throw new Error("Esta conexão já está associada a outro dispositivo desse tipo.");
  }
  device = { ...device, interface_id: connections[0].interface_id, connections };
  if (previous && previous.kind !== device.kind) throw new Error("O tipo de um dispositivo cadastrado não pode ser alterado.");
  if (device.kind === "mouse" ? !Number.isInteger(device.button_count) || device.button_count! < 3 || device.button_count! > 32 || device.layout !== null : device.button_count !== null || !device.layout || !Object.hasOwn(layouts, device.layout)) throw new Error("Confira as propriedades do dispositivo.");
  // Move legacy interface bindings once; subsequent transport changes keep this scope.
  const scope = bindingScope(device);
  const sources = new Set([...(previous ? [bindingScope(previous), ...connectionsOf(previous).map(c => c.interface_id)] : []), ...connections.map(c => c.interface_id)]);
  sources.delete(scope);
  const profiles = config.profiles.map(p => {
    const isKind = (b: typeof p.bindings[number]) => device.kind === "mouse" ? "MouseButton" in b.input : "Key" in b.input;
    const moving = p.bindings.filter(b => b.device_id && sources.has(b.device_id) && isKind(b));
    const seen = new Set(p.bindings.filter(b => b.device_id === scope).map(b => JSON.stringify(b.input)));
    for (const b of moving) {
      const key = JSON.stringify(b.input);
      if (seen.has(key)) throw new Error(`O dispositivo já possui ações conflitantes no perfil “${p.name}”. Revise as ações antes de vincular.`);
      seen.add(key);
    }
    return { ...p, bindings: p.bindings.map(b => moving.includes(b) ? { ...b, device_id: scope } : b) };
  });
  return { ...config, profiles, devices: previous ? devices.map(d => d.id === device.id ? { ...device, name } : d) : [...devices, { ...device, name }] };
}
