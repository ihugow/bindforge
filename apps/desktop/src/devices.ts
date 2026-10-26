export type DetectedDevice = {
  id: string;
  name: string;
  kind: "mouse" | "keyboard" | "composite";
  connection: "usb" | "bluetooth" | "internal" | "virtual" | "unknown";
  mouse_fingerprint?: string | null;
  keyboard_fingerprint?: string | null;
  vendor_id: number | null;
  product_id: number | null;
  battery?: { percentage: number; state: "charging" | "discharging" | "full" | "unknown" } | null;
};
export type InventoryState = { phase: "idle" | "loading" }
  | { phase: "ready"; devices: DetectedDevice[] }
  | { phase: "error"; message: string };

// One snapshot per request. A cancelled/older request cannot replace newer UI data.
export function loadInventory(read: () => Promise<DetectedDevice[]>, publish: (state: InventoryState) => void): () => void {
  let cancelled = false;
  publish({ phase: "loading" });
  void Promise.resolve().then(() => cancelled ? null : read()).then(devices => {
    if (!cancelled && devices !== null) publish({ phase: "ready", devices });
  }).catch(error => {
    if (!cancelled) publish({ phase: "error", message: typeof error === "string" ? error
      : error instanceof Error ? error.message : "Não foi possível carregar a lista de dispositivos." });
  });
  return () => { cancelled = true; };
}

// Refresh only visible views; cancel an older read before accepting a new snapshot.
export function watchInventory(read: () => Promise<DetectedDevice[]>, publish: (state: InventoryState) => void): () => void {
  let cancel = () => {};
  let ready = false;
  const refresh = () => {
    if (globalThis.document?.visibilityState === "hidden") return;
    cancel();
    cancel = loadInventory(read, state => {
      if (state.phase === "loading" && ready) return;
      ready = state.phase === "ready";
      publish(state);
    });
  };
  refresh();
  const timer = setInterval(refresh, 60_000);
  globalThis.document?.addEventListener("visibilitychange", refresh);
  return () => { cancel(); clearInterval(timer); globalThis.document?.removeEventListener("visibilitychange", refresh); };
}
export function batteryLabel(device?: DetectedDevice): string {
  const battery = device?.battery;
  if (!battery || !Number.isInteger(battery.percentage) || battery.percentage < 0 || battery.percentage > 100) return "Bateria indisponível";
  const state = { charging: " · Carregando", discharging: "", full: " · Carga completa", unknown: "" }[battery.state] ?? "";
  return `Bateria: ${battery.percentage}%${state}`;
}
