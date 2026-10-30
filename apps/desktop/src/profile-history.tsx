import { AppButton } from "./app-button";
import { useState } from "react";
import { ProfileName } from "./profile-name";
import type { Config } from "./profiles";
import type { DaemonStatus } from "./daemon-status";

export function ProfileHistory({ status, config }: { status: DaemonStatus; config: Config | null | undefined }) {
  const [visible, setVisible] = useState(false);
  return <div className="profile-history">
    <div className="section-heading">
      <h3>Últimas trocas de perfil</h3>
      <AppButton type="button" className="history-toggle" role="switch" aria-checked={visible}
        aria-controls="profile-history-content" aria-label="Exibir histórico das últimas trocas de perfil" onClick={() => setVisible(value => !value)}>
        <span className="toggle-track" aria-hidden="true"><span /></span>Exibir histórico
      </AppButton>
    </div>
    <div id="profile-history-content" hidden={!visible}>
      <p className="hint">Ao voltar para esta janela, o perfil atual pode retornar a <strong><ProfileName id={config?.fallback_id ?? "default"} config={config} /></strong>. As trocas anteriores ficam registradas abaixo.</p>
      {!status.history_available
        ? <p className="hint">Reinicie o daemon para habilitar o histórico desta versão.</p>
        : status.recent_profiles.length === 0
          ? <p className="hint">Aguardando a primeira seleção de perfil.</p>
          : <ol className="history-list">{[...status.recent_profiles].reverse().map(change =>
              <li key={change.revision}><strong><ProfileName id={change.profile_id} config={config} /></strong><span>Atualização {change.revision}</span></li>
            )}</ol>}
    </div>
  </div>;
}
