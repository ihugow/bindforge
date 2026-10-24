import { useEffect, useRef, useState } from "react";
import { Modal } from "./modal";
import { deviceAccessApi, type AccessAction, type AccessStatus } from "./device-access";
import type { ConfiguredDevice } from "./configured-devices";

export function DeviceAccessButton({ device, disabled = false, api = deviceAccessApi, onChanged }: {
  device?: ConfiguredDevice; disabled?: boolean; api?: typeof deviceAccessApi; onChanged?: () => void;
}) {
  const [open, setOpen] = useState(false);
  const trigger = useRef<HTMLButtonElement>(null);
  return <><button type="button" className="secondary-button" ref={trigger} disabled={disabled || !device} onClick={() => setOpen(true)}>Gerenciar acesso{device ? ` · ${device.name}` : ""}</button>
    {open && device && <DeviceAccessPanel key={`${device.id}:${device.interface_id}`} device={device} api={api} close={() => setOpen(false)} restoreFocus={trigger} onChanged={onChanged} />}
  </>;
}
export function DeviceAccessPanel({ device, api = deviceAccessApi, close, restoreFocus, onChanged }: {
  device: ConfiguredDevice; api?: typeof deviceAccessApi; close: () => void;
  restoreFocus?: React.RefObject<HTMLElement | null>; onChanged?: () => void;
}) {
  const [status, setStatus] = useState<AccessStatus | null>(null);
  const [loading, setLoading] = useState(true);
  const [operation, setOperation] = useState<AccessAction | null>(null);
  const [error, setError] = useState("");
  const [message, setMessage] = useState("");
  const [reconnect, setReconnect] = useState(false);
  const [refresh, setRefresh] = useState(0);
  const busy = loading || operation !== null;
  useEffect(() => {
    let cancelled = false;
    setLoading(true); setError("");
    void api.status(device.id).then(next => { if (!cancelled) setStatus(next); }).catch(e => { if (!cancelled) { setStatus(null); setError(String(e)); } }).finally(() => { if (!cancelled) setLoading(false); });
    return () => { cancelled = true; };
  }, [device.id, device.interface_id, api, refresh]);
  async function change(action: AccessAction) {
    if (busy || !status) return;
    setOperation(action); setError(""); setMessage("");
    try {
      const result = await api.control(device.id, action, status.fingerprint);
      setStatus(result.status); setMessage(result.message); setReconnect(result.reconnect_required); onChanged?.();
    } catch (e) {
      setError(String(e));
      // Authorization may have completed before a disconnect/error. Read the actual rule state.
      try { setStatus(await api.status(device.id)); } catch { setStatus(null); }
      onChanged?.();
    } finally { setOperation(null); }
  }
  return <Modal title={`Acesso · ${device.name}`} compact onDismiss={close} busy={busy} restoreFocus={restoreFocus}>
    <p>Autorize a interface deste {device.kind === "keyboard" ? "teclado" : "mouse"} para usar o remapeamento físico.</p>
    {loading && <p role="status">Consultando as permissões…</p>}
    {status && <>
      <p className="hint">Interface identificada: {status.device_name} · {status.model}</p>
      <dl className="access-details">
        <div><dt>Entrada do dispositivo</dt><dd>{status.readable ? "Acesso disponível" : "Sem acesso"}</dd></div>
        <div><dt>Saída virtual</dt><dd>{status.output_writable ? "Acesso disponível" : "Sem acesso"}</dd></div>
        <div><dt>Autorização do BindForge</dt><dd>{status.revoked ? "Revogada · ativação bloqueada" : status.managed ? "Instalada" : "Não instalada"}</dd></div>
      </dl>
      <p className="hint">{status.scope}</p><p role="status">{status.message}</p>
      <div className="access-explanation"><p>O sistema solicitará autenticação administrativa. A autorização permite que programas do usuário da sessão local ativa acessem esta interface e criem a saída virtual de remapeamento.</p>
        <p>Remover a autorização pausa a captura e bloqueia novas ativações no BindForge até uma nova concessão. Reconectar encerra acessos residuais do sistema; permissões externas podem continuar disponíveis.</p></div>
    </>}
    {operation && <p role="status">{operation === "grant" ? "Aguardando autenticação para conceder acesso…" : "Aguardando autenticação para remover a autorização…"}</p>}
    {message && <p role="status">{message}</p>}
    {reconnect && <p className="hint">Depois de reconectar, atualize a lista de dispositivos. Se a interface mudou, selecione-a novamente no cadastro.</p>}
    {error && <p role="alert">{error}</p>}
    <div className="button-row modal-actions">
      <button type="button" className="secondary-button" disabled={busy} onClick={() => setRefresh(n => n + 1)}>Atualizar estado</button>
      <button type="button" disabled={busy || !status?.helper_available} onClick={() => void change("grant")}>{status?.managed ? "Reaplicar autorização" : "Conceder acesso"}</button>
      {status?.managed && <button type="button" className="danger-button" disabled={busy || !status.helper_available} onClick={() => void change("revoke")}>Remover autorização</button>}
      <button type="button" disabled={busy} onClick={close}>Fechar</button>
    </div>
  </Modal>;
}
