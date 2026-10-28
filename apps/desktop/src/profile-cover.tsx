import { useState } from "react";
import { ProfileIcon } from "./profile-icon";

export function ProfileCover({ image, icon, name }: { image?: string; icon?: string | null; name?: string }) {
  const [failed, setFailed] = useState<string>();
  return <div className="profile-cover" aria-hidden="true">
    <div className="profile-cover-placeholder"><ProfileIcon icon={icon} />{name && (!image || failed === image) && <span className="profile-cover-name">{name}</span>}</div>
    {image && failed !== image && <img className="profile-cover-image" src={image} alt="" width={600} height={900}
      loading="lazy" decoding="async" onError={() => setFailed(image)} />}
  </div>;
}
