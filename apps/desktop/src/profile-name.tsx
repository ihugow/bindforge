import type { Config } from "./profiles";

// The daemon's stable key identifies the profile; the editable name is its label.
// Unknown keys stay explicit rather than being presented as a user-facing name.
export function ProfileName({ id, config }: { id: string | null; config: Config | null | undefined }) {
  const name = id === null ? "Nenhum" : config === undefined ? "Carregando perfis…"
    : config === null ? "Nome indisponível" : config.profiles.find(p => p.id === id)?.name ?? "Perfil não encontrado";
  return <>{name}</>;
}
