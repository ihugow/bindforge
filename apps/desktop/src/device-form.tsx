import { AppSelect } from "./app-select";
import { useState } from "react";
import { DeviceAccessButton } from "./device-access-panel";
import { Modal } from "./modal";
import { compatible, layouts, type ConfiguredDevice } from "./configured-devices";
import type { DetectedDevice } from "./devices";
export function DeviceForm({ device, inventory, save, close }: {
  device?: ConfiguredDevice; inventory: DetectedDevice[]; save: (device: ConfiguredDevice) => Promise<void>; close: () => void;
}) {
  const [id] = useState(() => device?.id ?? `device-${crypto.randomUUID()}`);
  const [name, setName] = useState(device?.name ?? "");
  const [kind, setKind] = useState<ConfiguredDevice["kind"]>(device?.kind ?? "mouse");
  const [interfaceId, setInterfaceId] = useState(device?.interface_id ?? "");
  const [count, setCount] = useState(device?.button_count ?? 5);
  const [layout, setLayout] = useState<NonNullable<ConfiguredDevice["layout"]>>(device?.layout ?? "abnt2");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const available = inventory.filter(d => compatible(d, kind));
  return <Modal title={device ? "Editar dispositivo" : "Configurar novo dispositivo"} onDismiss={close} busy={busy} initialFocus="#device-name">
    <form onSubmit={async e => { e.preventDefault(); setBusy(true); setError(""); try { await save({ id, name, kind, interface_id: interfaceId, button_count: kind === "mouse" ? count : null, layout: kind === "keyboard" ? layout : null }); close(); } catch (e) { setError(String(e)); } finally { setBusy(false); } }}>
      <fieldset disabled={busy} className="device-fields">
        <label htmlFor="device-kind">Tipo</label><AppSelect id="device-kind" className="app-select" value={kind} disabled={!!device} onValueChange={value => { setKind(value as typeof kind); setInterfaceId(""); }}><option value="mouse">Mouse</option><option value="keyboard">Teclado</option></AppSelect>
        <label htmlFor="device-name">Nome do dispositivo</label><input id="device-name" value={name} required maxLength={128} placeholder={kind === "mouse" ? "Logitech G PRO Wireless" : "Aula F99"} onChange={e => setName(e.target.value)} />
        {kind === "mouse" ? <><label htmlFor="device-buttons">Quantidade de botões</label><input id="device-buttons" type="number" min={3} max={32} required value={count} onChange={e => setCount(Number(e.target.value))} /><p className="hint">Inclui os cliques esquerdo, direito e da roda. Não inclui rolagem. De 3 a 32 botões.</p></> : <><label htmlFor="device-layout">Layout físico</label><AppSelect id="device-layout" className="app-select" value={layout} onValueChange={value => setLayout(value as typeof layout)}>{Object.entries(layouts).map(([id, label]) => <option key={id} value={id}>{label}</option>)}</AppSelect><p className="hint">Descreve o teclado; não altera o layout configurado no sistema.</p></>}
        <label htmlFor="device-interface">Interface identificada</label><AppSelect id="device-interface" className="app-select" required value={interfaceId} onValueChange={value => setInterfaceId(value)}>
          <option value="">Selecione a interface correspondente</option>
          {device && !available.some(d => d.id === device.interface_id) && <option value={device.interface_id}>Interface salva · indisponível nesta consulta</option>}
          {available.map((d, i) => <option key={d.id} value={d.id}>{d.name} · {d.connection} · interface {i + 1}</option>)}
        </AppSelect>
        {interfaceId && <details className="interface-details"><summary>Identificador da interface selecionada</summary><code>{interfaceId}</code></details>}
        {!available.length && <p role="status">Nenhuma interface compatível disponível. Conecte o daemon e atualize a lista.</p>}
        <p className="hint">O vínculo usa a interface atual. Se ela mudar após reconectar ou reiniciar, edite o dispositivo e selecione a nova interface. Suas ações acompanham a troca.</p>
      </fieldset>
      {error && <p role="alert">{error}</p>}
      <div className="button-row modal-actions"><button type="button" disabled={busy} onClick={close}>Cancelar</button><button type="submit" disabled={busy}>{busy ? "Salvando…" : "Salvar dispositivo"}</button></div>
    </form>
    {device && <div className="device-access-entry"><h3>Acesso para remapeamento</h3><DeviceAccessButton device={device} disabled={busy || interfaceId !== device.interface_id} /><p className="hint">{interfaceId !== device.interface_id ? "Salve o novo vínculo antes de gerenciar o acesso." : "Consulte, conceda ou remova a autorização deste dispositivo pela autenticação do sistema."}</p></div>}
  </Modal>;
}
