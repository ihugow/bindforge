import type { ConfiguredDevice } from "./configured-devices";
export type Rule = { app_id: string | null; executable: string | null; title_contains: string | null };
export type Binding = {
  device_id: string | null;
  input: { Key: string } | { MouseButton: number };
  action: { KeyChord: string[] } | { MouseButton: number } | "Disabled";
};
export type Profile = { id: string; name: string; icon?: string | null; enabled: boolean; priority: number; rules: Rule[]; bindings: Binding[] };
export type Config = { devices?: ConfiguredDevice[]; version: number; revision: number; fallback_id: string; profiles: Profile[] };
export type Draft = { icon?: string | null; id: string | null; name: string; apps: string; priority: string; enabled: boolean; source: string };
export function appIds(value: string): string[] {
  return [...new Set(value.split(/\r?\n/).map(s => s.trim()).filter(Boolean))];
}
export function updateProfile(config: Config, draft: Draft, newId: string): Config {
  const existing = config.profiles.find(p => p.id === draft.id);
  const fallback = existing?.id === config.fallback_id;
  const name = draft.name.trim();
  const apps = appIds(draft.apps);
  if (!name || new TextEncoder().encode(name).length > 128 || /[\x00-\x1f\x7f]/.test(name)) throw new Error("Informe um nome de até 128 bytes.");
  if (!fallback && !apps.length && !existing?.rules.some(r => r.executable !== null || r.title_contains !== null)) throw new Error("Informe pelo menos um ID de aplicativo.");
  const priority = Number(draft.priority);
  if (!draft.priority.trim() || !Number.isInteger(priority) || priority < -2147483648 || priority > 2147483647) throw new Error("A prioridade deve ser um número inteiro de 32 bits.");
  const source = existing ?? config.profiles.find(p => p.id === draft.source);
  const advanced = existing?.rules.some(r => r.executable !== null || r.title_contains !== null);
  const profile: Profile = {
    id: existing?.id ?? newId, name, icon: draft.icon === undefined ? existing?.icon ?? null : draft.icon, enabled: fallback ? true : draft.enabled, priority,
    // Preserve advanced predicates when editing an existing rule's application list.
    rules: fallback ? [] : advanced ? structuredClone(existing!.rules) : apps.map(app_id => existing?.rules.find(r => r.app_id === app_id) ?? { app_id, executable: null, title_contains: null }),
    bindings: structuredClone(source?.bindings ?? []),
  };
  if (!existing && config.profiles.some(p => p.id === newId)) throw new Error("ID de perfil duplicado.");
  return { ...config, profiles: existing ? config.profiles.map(p => p.id === existing.id ? profile : p) : [...config.profiles, profile] };
}
export function conflictingProfiles(config: Config, draft: Draft): Profile[] {
  const apps = appIds(draft.apps);
  return config.profiles.filter(p => p.id !== draft.id && p.enabled && p.rules.some(r => r.app_id !== null && apps.includes(r.app_id)));
}

export function removeProfile(config: Config, id: string): Config {
  if (id === config.fallback_id) throw new Error("O perfil Padrão não pode ser excluído.");
  if (!config.profiles.some(p => p.id === id)) throw new Error("Este perfil não existe mais. Recarregue a lista.");
  return { ...config, profiles: config.profiles.filter(p => p.id !== id) };
}
