import { useState } from "react";
import { AppIcon } from "./app-icon";

export function ProfileIcon({ icon }: { icon?: string | null }) {
  const [failed, setFailed] = useState<string | null>(null);
  return <span className="profile-icon" aria-hidden="true">
    <span className="profile-icon-clip">{icon?.startsWith("data:image/png;base64,") && failed !== icon
      ? <img src={icon} alt="" onError={() => setFailed(icon)} />
      : <AppIcon name="profiles" />}</span>
  </span>;
}

export async function readProfileIcon(file: File): Promise<string> {
  const ico = /\.ico$/i.test(file.name) || ["image/x-icon", "image/vnd.microsoft.icon"].includes(file.type);
  if (!ico && !["image/png", "image/jpeg", "image/webp"].includes(file.type)) throw new Error("Escolha uma imagem PNG, JPG, WebP ou ICO.");
  if (file.size > 5 * 1024 * 1024) throw new Error("Escolha uma imagem de até 5 MB.");
  // Some file pickers report ICO as application/octet-stream or with no MIME type.
  const url = URL.createObjectURL(ico ? new Blob([file], { type: "image/x-icon" }) : file);
  try {
    const image = new Image();
    image.src = url;
    await image.decode();
    const canvas = document.createElement("canvas");
    const scale = Math.min(96 / image.naturalWidth, 96 / image.naturalHeight);
    canvas.width = Math.max(1, Math.round(image.naturalWidth * scale));
    canvas.height = Math.max(1, Math.round(image.naturalHeight * scale));
    const context = canvas.getContext("2d");
    if (!context) throw new Error("Não foi possível preparar o ícone.");
    context.drawImage(image, 0, 0, canvas.width, canvas.height);
    const icon = canvas.toDataURL("image/png");
    if (icon.length - "data:image/png;base64,".length > 65536) throw new Error("A imagem é muito complexa. Escolha um ícone mais simples.");
    return icon;
  } catch (error) {
    if (error instanceof DOMException) throw new Error("Não foi possível abrir a imagem. Escolha outro arquivo.");
    throw error;
  } finally { URL.revokeObjectURL(url); }
}
