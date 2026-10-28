import type { PointerEvent } from "react";
import { ProfileCover } from "./profile-cover";
import type { Profile } from "./profiles";

export function ProfileCard({ profile, image, disabled, onEdit }: {
  profile: Profile; image?: string; disabled: boolean; onEdit: () => void;
}) {
  function highlight(event: PointerEvent<HTMLButtonElement>) {
    if (event.pointerType === "touch" || disabled) return;
    const card = event.currentTarget;
    const bounds = card.getBoundingClientRect();
    card.style.setProperty("--shine-x", `${(event.clientX - bounds.left) / bounds.width * 100}%`);
    card.style.setProperty("--shine-y", `${(event.clientY - bounds.top) / bounds.height * 100}%`);
  }
  function reset(event: PointerEvent<HTMLButtonElement>) {
    event.currentTarget.style.removeProperty("--shine-x");
    event.currentTarget.style.removeProperty("--shine-y");
  }
  return <button type="button" className="profile-card" aria-label={`Editar perfil ${profile.name}`}
    title={profile.name} disabled={disabled} onClick={onEdit}
    onPointerMove={highlight} onPointerLeave={reset} onPointerCancel={reset}>
    <ProfileCover image={image} icon={profile.icon} name={profile.name} />
  </button>;
}
