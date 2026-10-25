import { useEffect, useRef, useState } from "react";
import { ProfileIcon } from "./profile-icon";
import { appIds } from "./profiles";
import { findApplicationIcons, type ApplicationIcon, type FindApplicationIcons } from "./application-icons";

export function ApplicationIconPicker({ apps, automaticAppId, hasIcon, onSelect, onPending, find = findApplicationIcons }: {
  apps: string; automaticAppId: string | null; hasIcon: boolean; onSelect: (icon: string) => void;
  onPending: (pending: boolean) => void; find?: FindApplicationIcons;
}) {
  const [choices, setChoices] = useState<ApplicationIcon[]>([]);
  const [message, setMessage] = useState("");
  const [loading, setLoading] = useState(false);
  const request = useRef(0);
  const callbacks = useRef({ onSelect, onPending, find });
  callbacks.current = { onSelect, onPending, find };
  // Invalidate responses when associations change or this editor is closed.
  useEffect(() => {
    setChoices([]); setMessage("");
    return () => { request.current++; callbacks.current.onPending(false); };
  }, [apps]);
  async function search(ids: string[], automatic = false) {
    const version = ++request.current;
    setChoices([]); setMessage(""); setLoading(true); callbacks.current.onPending(true);
    try {
      const result = await callbacks.current.find(ids);
      if (request.current !== version) return;
      if (!result.length) {
        setMessage("Não encontramos um ícone local para esses aplicativos. Você pode escolher uma imagem.");
      } else if (result.length === 1) {
        callbacks.current.onSelect(result[0].icon);
        setMessage(`Ícone de ${result[0].name} selecionado. Salve o perfil para aplicar.`);
      } else if (!automatic) {
        setChoices(result); setMessage("Escolha o aplicativo que vai representar este perfil.");
      }
    } catch {
      if (request.current === version) setMessage("Não foi possível buscar os ícones locais. Tente novamente ou escolha uma imagem.");
    } finally {
      if (request.current === version) { setLoading(false); callbacks.current.onPending(false); }
    }
  }
  useEffect(() => {
    // Capture fills an empty icon only; a custom/previously selected icon stays intact.
    if (automaticAppId && !hasIcon) void search([automaticAppId], true);
    // Only a new capture should trigger automatic lookup.
  }, [automaticAppId]);
  return <div className="application-icon-picker">
    <button type="button" className="secondary-button" disabled={loading || !appIds(apps).length} onClick={() => void search(appIds(apps))}>
      {loading ? "Buscando ícones…" : "Usar ícone do aplicativo"}
    </button>
    {!appIds(apps).length && <p className="hint">Associe um aplicativo ao perfil para buscar seu ícone.</p>}
    {message && <p className="hint" role="status">{message}</p>}
    {choices.length > 0 && <div className="application-icon-choices" role="group" aria-label="Ícones dos aplicativos associados">
      {choices.map(choice => <button type="button" className="application-icon-choice secondary-button" key={choice.app_id}
        onClick={() => { onSelect(choice.icon); setChoices([]); setMessage(`Ícone de ${choice.name} selecionado. Salve o perfil para aplicar.`); }}>
        <ProfileIcon icon={choice.icon} /><span>{choice.name}<small>{choice.app_id}</small></span>
      </button>)}
    </div>}
  </div>;
}
