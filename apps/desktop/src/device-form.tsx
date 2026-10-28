import { AppSelect } from "./app-select";
import { useRef, useState } from "react";
import { DeviceAccessButton } from "./device-access-panel";
import { Modal } from "./modal";
import { compatible, connectionsOf, layouts, type Connection, type ConfiguredDevice } from "./configured-devices";
import type { DetectedDevice } from "./devices";
export function DeviceForm({ device, inventory, refreshInventory, save, close, remove }: {
  device?: ConfiguredDevice; inventory: DetectedDevice[]; refreshInventory?: () => Promise<DetectedDevice[]>; save: (device: ConfiguredDevice, inventory?: DetectedDevice[]) => Promise<void>; close: () => void;
  remove?: () => Promise<void>;
}) {
  const [id] = useState(() => device?.id ?? `device-${crypto.randomUUID()}`);
  const [name, setName] = useState(device?.name ?? "");
  const [kind, setKind] = useState<ConfiguredDevice["kind"]>(device?.kind ?? "mouse");
  const [interfaceId, setInterfaceId] = useState("");
  const [connections, setConnections] = useState(() => device ? connectionsOf(device) : []);
  const [editingConnection, setEditingConnection] = useState<string | null>(null);
  const connectionEditor = useRef<HTMLDivElement>(null);
  const [connectionName, setConnectionName] = useState("Receptor");
  const [count, setCount] = useState(device?.button_count ?? 5);
  const [layout, setLayout] = useState<NonNullable<ConfiguredDevice["layout"]>>(device?.layout ?? "abnt2");
  const [busy, setBusy] = useState(false);
  const [freshInventory, setFreshInventory] = useState<DetectedDevice[] | null>(null);
  const [error, setError] = useState("");
  const [confirmRemove, setConfirmRemove] = useState(false);
  const currentInventory = freshInventory ?? inventory;
  const available = currentInventory.filter(d => compatible(d, kind));
  const fingerprintOf = (d: DetectedDevice) => kind === "mouse" ? d.mouse_fingerprint : d.keyboard_fingerprint;
  const detectedFor = (connection: Connection) => currentInventory.find(d => compatible(d, kind) &&
    (connection.fingerprint ? fingerprintOf(d) === connection.fingerprint : d.id === connection.interface_id));
  const editing = connections.find(c => c.interface_id === editingConnection);
  const selected = available.find(d => d.id === interfaceId);
  const duplicate = connections.some(c => c.interface_id !== editingConnection &&
    (c.interface_id === interfaceId || !!selected && !!c.fingerprint && c.fingerprint === fingerprintOf(selected)));
  const canLink = !!connectionName.trim() && (!!selected || !!editing && interfaceId === editing.interface_id) && !duplicate && (!!editing || connections.length < 8);
  function resetConnectionEditor() { setEditingConnection(null); setInterfaceId(""); setConnectionName("Cabo"); }
  function editConnection(connection: Connection) {
    setEditingConnection(connection.interface_id);
    setInterfaceId(detectedFor(connection)?.id ?? connection.interface_id);
    setConnectionName(connection.name); setError("");
    connectionEditor.current?.scrollIntoView({ block: "nearest", behavior: "smooth" });
    document.getElementById("connection-name")?.focus();
  }
  function applyConnection() {
    if (!canLink) return;
    const connection: Connection = { interface_id: selected?.id ?? editing!.interface_id, name: connectionName.trim(),
      interface_name: selected?.name ?? editing?.interface_name,
      fingerprint: selected ? fingerprintOf(selected) : editing?.fingerprint };
    setConnections(editing ? connections.map(c => c.interface_id === editingConnection ? connection : c) : [connection, ...connections]);
    resetConnectionEditor();
  }
  return <Modal title={device ? "Configurações do dispositivo" : "Configurar novo dispositivo"} onDismiss={close} busy={busy} initialFocus="#device-name">
    <form onSubmit={async e => {
      e.preventDefault(); setError("");
      if (editingConnection || connections.length && interfaceId) { setError("Conclua ou cancele a edição da conexão antes de salvar o dispositivo."); return; }
      setBusy(true);
      try {
        const savedConnections = connections.length ? connections : selected && connectionName.trim() ? [{
          interface_id: selected.id, name: connectionName.trim(), interface_name: selected.name, fingerprint: fingerprintOf(selected),
        }] : [];
        if (!savedConnections.length) throw new Error("Vincule pelo menos uma conexão ao dispositivo.");
        await save({ id, name, kind, interface_id: savedConnections[0].interface_id,
          connections: savedConnections.map(c => ({ ...c, interface_name: detectedFor(c)?.name ?? c.interface_name })),
          button_count: kind === "mouse" ? count : null, layout: kind === "keyboard" ? layout : null }, currentInventory);
        close();
      } catch (e) { setError(String(e)); } finally { setBusy(false); }
    }}>
      <fieldset disabled={busy} className="device-fields">
        <label htmlFor="device-kind">Tipo</label><AppSelect id="device-kind" className="app-select" value={kind} disabled={!!device} onValueChange={value => { setKind(value as typeof kind); setInterfaceId(""); setConnections([]); resetConnectionEditor(); }}><option value="mouse">Mouse</option><option value="keyboard">Teclado</option></AppSelect>
        <label htmlFor="device-name">Nome do dispositivo</label><input id="device-name" value={name} required maxLength={128} placeholder={kind === "mouse" ? "Logitech G PRO Wireless" : "Aula F99"} onChange={e => setName(e.target.value)} />
        {kind === "mouse" ? <><label htmlFor="device-buttons">Quantidade de botões</label><input id="device-buttons" type="number" min={3} max={32} required value={count} onChange={e => setCount(Number(e.target.value))} /><p className="hint">Inclui os cliques esquerdo, direito e da roda. Não inclui rolagem. De 3 a 32 botões.</p></> : <><label htmlFor="device-layout">Layout físico</label><AppSelect id="device-layout" className="app-select" value={layout} onValueChange={value => setLayout(value as typeof layout)}>{Object.entries(layouts).map(([id, label]) => <option key={id} value={id}>{label}</option>)}</AppSelect><p className="hint">Descreve o teclado; não altera o layout configurado no sistema.</p></>}
        <div className="connection-editor" ref={connectionEditor}>
          <div className="connection-section-heading"><h3>{editing ? "Editar conexão" : "Adicionar conexão"}</h3>
            {refreshInventory && <button type="button" className="secondary-button" onClick={async () => {
              setBusy(true); setError("");
              try { setFreshInventory(await refreshInventory()); } catch (e) { setError(String(e)); } finally { setBusy(false); }
            }}>Atualizar interfaces</button>}
          </div>
          <label htmlFor="device-interface">Interface do dispositivo</label>
          <AppSelect id="device-interface" className="app-select" value={interfaceId} onValueChange={setInterfaceId}>
            <option value="">Selecione a interface correspondente</option>
            {editing && !available.some(d => d.id === editing.interface_id) && <option value={editing.interface_id}>{editing.interface_name ?? "Interface salva"} · desconectada</option>}
            {available.map((d, i) => <option key={d.id} value={d.id}>{d.name} · {d.connection} · interface {i + 1}</option>)}
          </AppSelect>
          <label htmlFor="connection-name">Nome da conexão</label>
          <input id="connection-name" value={connectionName} maxLength={128} placeholder="Receptor, Cabo ou Bluetooth" onChange={e => setConnectionName(e.target.value)} />
          <div className="connection-editor-actions">
            <button type="button" className="connection-link-button" disabled={!canLink} onClick={applyConnection}>
              {!editing && <span aria-hidden="true">＋</span>}{editing ? "Salvar conexão" : "Vincular conexão"}
            </button>
            {(editing || interfaceId) && <button type="button" className="secondary-button" onClick={resetConnectionEditor}>Cancelar{editing ? " edição" : " seleção"}</button>}
          </div>
          {duplicate && <p className="hint" role="status">Esta interface já está vinculada ao dispositivo.</p>}
        </div>
        {!!connections.length && <div className="saved-connections"><h3>Conexões vinculadas</h3>
          {connections.map((c, index) => {
            const detected = detectedFor(c);
            return <div key={c.interface_id} className="connection-row" data-editing={editingConnection === c.interface_id || undefined}>
              <div className="connection-info"><div className="connection-title"><strong>{c.name}</strong>{index === 0 && <span className="connection-preferred">Preferida</span>}</div>
                <span className="connection-interface-name">{detected?.name ?? c.interface_name ?? "Nome da interface indisponível"}</span>
                {!detected && <span className="connection-offline">Desconectada</span>}
              </div>
              <div className="connection-actions">
                <button type="button" className="secondary-button" aria-label={`Editar conexão ${c.name}`} onClick={() => editConnection(c)}>Editar</button>
                {index > 0 && <button type="button" className="secondary-button" aria-label={`Priorizar conexão ${c.name}`} onClick={() => setConnections([c, ...connections.filter((_, i) => i !== index)])}>Priorizar</button>}
                <button type="button" className="secondary-button" aria-label={`Remover conexão ${c.name}`} onClick={() => {
                  setConnections(connections.filter((_, i) => i !== index));
                  if (editingConnection === c.interface_id) resetConnectionEditor();
                }}>Remover</button>
              </div>
            </div>;
          })}
        </div>}
        <p className="hint">Vincule receptor e cabo ao mesmo cadastro. Todas as conexões compartilham as ações. Quando várias estiverem presentes, usamos apenas a primeira da lista; priorize o cabo para usá-lo mesmo com o receptor conectado. Conecte cada uma e atualize a lista para vinculá-la. Sem número de série, o reconhecimento pode depender da porta USB.</p>
      </fieldset>
      {error && <p role="alert">{error}</p>}
      <div className="button-row modal-actions"><button type="button" disabled={busy} onClick={close}>Cancelar</button><button type="submit" disabled={busy}>{busy ? "Salvando…" : "Salvar dispositivo"}</button></div>
    </form>
    {device && <div className="device-access-entry"><h3>Acesso para remapeamento</h3><DeviceAccessButton device={device} disabled={busy} /><p className="hint">Autorize cada conexão salva que será usada para remapear.</p></div>}
    {device && remove && <div className="device-access-entry"><h3>Remover dispositivo</h3><p className="hint">Remove o cadastro, suas conexões e as ações específicas deste dispositivo em todos os perfis.</p>
      <button type="button" className="danger-button" disabled={busy} onClick={() => { setError(""); setConfirmRemove(true); }}>Remover dispositivo</button>
    </div>}
    {confirmRemove && <Modal title={`Remover ${device!.name}?`} compact busy={busy} initialFocus="[data-cancel-removal]" onDismiss={() => setConfirmRemove(false)}>
      <p>O cadastro, as conexões e as ações específicas deste dispositivo serão removidos de todos os perfis. As ações gerais e os outros dispositivos serão mantidos.</p>
      {error && <p role="alert">{error}</p>}
      <div className="button-row modal-actions"><button type="button" data-cancel-removal disabled={busy} onClick={() => setConfirmRemove(false)}>Cancelar</button>
        <button type="button" className="danger-button" disabled={busy} onClick={async () => {
          setBusy(true); setError("");
          try { await remove!(); close(); } catch (e) { setError(String(e)); } finally { setBusy(false); }
        }}>{busy ? "Removendo…" : "Confirmar remoção"}</button>
      </div>
    </Modal>}
  </Modal>;
}
