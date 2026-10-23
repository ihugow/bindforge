import type { Config } from "./profiles";
import type { DetectedDevice } from "./devices";
export type ConfiguredDevice = {
  id: string; name: string; kind: "mouse" | "keyboard"; interface_id: string;
  button_count: number | null; layout: "abnt2" | "ansi" | "iso" | null;
};
export const layouts = { abnt2: "Português brasileiro · ABNT2", ansi: "ANSI · padrão americano", iso: "ISO · padrão europeu" };
export function compatible(device: DetectedDevice, kind: ConfiguredDevice["kind"]) { return device.kind === kind || device.kind === "composite"; }
export function withDevice(config: Config, device: ConfiguredDevice, inventory: DetectedDevice[]): Config {
  const name = device.name.trim();
  if (!name || new TextEncoder().encode(name).length > 128 || /[\x00-\x1f\x7f-\x9f]/.test(name)) throw new Error("Informe um nome de até 128 bytes, sem caracteres de controle.");
  const devices = config.devices ?? [];
  const previous = devices.find(d => d.id === device.id);
  if (!previous && devices.length >= 64) throw new Error("Limite de 64 dispositivos atingido.");
  const detected = inventory.find(d => d.id === device.interface_id);
  if ((!previous || previous.interface_id !== device.interface_id || previous.kind !== device.kind) && (!detected || !compatible(detected, device.kind))) throw new Error("Selecione uma interface compatível identificada na lista.");
  if (!device.interface_id) throw new Error("Selecione uma interface.");
  if (devices.some(d => d.id !== device.id && d.interface_id === device.interface_id && d.kind === device.kind)) throw new Error("Esta interface já está associada a outro dispositivo desse tipo.");
  if (previous && previous.kind !== device.kind) throw new Error("O tipo de um dispositivo cadastrado não pode ser alterado.");
  if (device.kind === "mouse" ? !Number.isInteger(device.button_count) || device.button_count! < 3 || device.button_count! > 32 || device.layout !== null : device.button_count !== null || !device.layout || !Object.hasOwn(layouts, device.layout)) throw new Error("Confira as propriedades do dispositivo.");
  // Preserve per-profile actions when rebinding the same physical device.
  const profiles = config.profiles.map(p => {
    if (!previous || previous.interface_id === device.interface_id) return p;
    const isKind = (b: typeof p.bindings[number]) => device.kind === "mouse" ? "MouseButton" in b.input : "Key" in b.input;
    const moving = p.bindings.filter(b => b.device_id === previous.interface_id && isKind(b));
    if (moving.some(b => p.bindings.some(other => other.device_id === device.interface_id && JSON.stringify(other.input) === JSON.stringify(b.input)))) throw new Error(`A interface de destino já possui ações no perfil “${p.name}”. Escolha outra interface ou remova essas ações antes de vincular.`);
    return { ...p, bindings: p.bindings.map(b => moving.includes(b) ? { ...b, device_id: device.interface_id } : b) };
  });
  return { ...config, profiles, devices: previous ? devices.map(d => d.id === device.id ? { ...device, name } : d) : [...devices, { ...device, name }] };
}
