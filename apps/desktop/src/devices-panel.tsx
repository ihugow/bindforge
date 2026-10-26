import { connectedInterface } from "./configured-devices";
import { useEffect, useState } from "react";
import { DeviceBattery } from "./device-battery";
import { DeviceForm } from "./device-form";
import { MouseArt, KeyboardArt } from "./device-art";
import { layouts, withDevice, type ConfiguredDevice } from "./configured-devices";
import { profileApi } from "./profile-api";
import type { Config } from "./profiles";
import { watchInventory, type DetectedDevice, type InventoryState } from "./devices";

const kinds = { mouse: "Mouse", keyboard: "Teclado", composite: "Mouse e teclado" };
const connections = { usb: "USB", bluetooth: "Bluetooth", internal: "Integrado", virtual: "Virtual", unknown: "Conexão não identificada" };
const hex = (value: number | null) => value === null ? "Não informado" : value.toString(16).padStart(4, "0").toUpperCase();

export function DevicesPanel({ native, connected, read, config, onConfigChange, api = profileApi }: {
  native: boolean; connected: boolean; read: () => Promise<DetectedDevice[]>;
  config?: Config | null; onConfigChange?: (config: Config) => void; api?: typeof profileApi;
}) {
  const [inventory, setInventory] = useState<InventoryState>({ phase: "idle" });
  const [editing, setEditing] = useState<ConfiguredDevice | "new" | null>(null);
  const [message, setMessage] = useState("");
  const [refresh, setRefresh] = useState(0);
  useEffect(() => {
    if (!native || !connected) { setInventory({ phase: "idle" }); return; }
    return watchInventory(read, setInventory);
  }, [native, connected, read, refresh]);
  return <section aria-labelledby="devices-heading">
    <div className="section-heading"><h2 id="devices-heading">Mouses e teclados</h2>
      <button type="button" disabled={!native || !connected || inventory.phase === "loading"}
        onClick={() => setRefresh(n => n + 1)}>{inventory.phase === "loading" ? "Consultando…" : "Atualizar lista"}</button>
    </div>
    <p>Cadastre seus periféricos e vincule cada um à interface correspondente.</p>
    <div className="device-showcase configured-devices">
      <button type="button" className="device-card add-device-card" disabled={!native || !config} onClick={() => setEditing("new")}>
        <span className="add-device-symbol" aria-hidden="true">+</span><span className="device-card-name">Configurar dispositivo</span><span className="device-card-caption">Mouse ou teclado</span>
      </button>
      {(config?.devices ?? []).map(d => <button type="button" className="device-card" key={d.id} onClick={() => setEditing(d)}>
        <span className="device-card-art">{d.kind === "mouse" ? <MouseArt /> : <KeyboardArt />}</span><span className="device-card-name">{d.name}</span>
        <span className="device-card-caption">{d.kind === "mouse" ? `Mouse · ${d.button_count} botões` : `Teclado · ${layouts[d.layout!]}`}</span>
        <span className="device-card-caption">{!connected || inventory.phase !== "ready" ? "Conexão não verificada" : !!connectedInterface(d, inventory.devices) ? "Interface conectada" : "Interface indisponível"}</span><DeviceBattery device={inventory.phase === "ready" ? connectedInterface(d, inventory.devices) : undefined} /><span className="device-card-link">Editar dispositivo ↗</span>
      </button>)}
    </div>
    {config && !(config.devices?.length) && <p className="hint">Comece pelo + para adicionar seu primeiro dispositivo.</p>}
    {message && <p role="status">{message}</p>}
    <details className="detected-interfaces"><summary>Interfaces identificadas</summary>
    <p className="hint">Consulta de metadados, sem captura de teclas ou movimentos.</p>
    {!native ? <p role="status">A lista está disponível no aplicativo nativo.</p>
      : !connected ? <p role="status">Aguardando conexão com o daemon.</p>
      : <>
        {inventory.phase === "loading" && <p role="status">Consultando os dispositivos conectados…</p>}
        {inventory.phase === "error" && <p role="alert">{inventory.message}</p>}
        {inventory.phase === "ready" && <>
          <p role="status">{inventory.devices.length} interface(s) identificada(s).</p>
          {inventory.devices.length === 0 ? <p>Nenhum mouse ou teclado identificado nesta consulta.</p>
            : <ul className="device-list">{inventory.devices.map(device => <li key={device.id}>
                <h3>{device.name}</h3>
                <p className="device-kind">{kinds[device.kind]} · {connections[device.connection]}</p>
                <DeviceBattery device={device} />
                {(device.vendor_id !== null || device.product_id !== null) && <details>
                  <summary>Identificação do modelo</summary>
                  <p className="hint">Fabricante: {hex(device.vendor_id)} · Produto: {hex(device.product_id)}</p>
                </details>}
              </li>)}</ul>}
        </>}
      </>}
    </details>
    <p className="hint">Após conectar ou desconectar um dispositivo, clique em “Atualizar lista”. Um periférico pode aparecer em mais de uma interface; entradas virtuais são indicadas como “Virtual”.</p>
    {editing && config && <DeviceForm refreshInventory={read} device={editing === "new" ? undefined : editing} inventory={inventory.phase === "ready" ? inventory.devices : []} close={() => setEditing(null)} save={async (device, snapshot) => {
      const saved = await api.save(withDevice(config, device, snapshot ?? (inventory.phase === "ready" ? inventory.devices : [])), config.revision);
      onConfigChange?.(saved); setMessage(`Dispositivo “${device.name.trim()}” salvo.`);
    }} />}
  </section>;
}
