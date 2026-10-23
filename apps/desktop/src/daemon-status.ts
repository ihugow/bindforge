export type DaemonStatus = {
  protocol_version: number;
  simulation: boolean;
  active_profile_id: string | null;
  focus_state: "waiting" | "focused" | "no_active_window" | "stopped";
  revision: string;
  recent_profiles: { revision: string; profile_id: string }[];
  history_available: boolean;
};
export type DaemonEvent =
  | { kind: "status"; status: DaemonStatus }
  | { kind: "unavailable"; message: string };
export type DaemonConnection =
  | { phase: "browser" | "loading" }
  | { phase: "unavailable"; message: string }
  | { phase: "connected"; status: DaemonStatus };

export type Subscribe = (receive: (event: DaemonEvent) => void) => Promise<() => void>;

// Native events carry both the initial snapshot and subsequent updates. No timer
// or periodic status invoke runs here. A late startup is disposed after unmount.
export function watchDaemon(subscribe: Subscribe, publish: (state: DaemonConnection) => void): () => void {
  let stopped = false;
  let dispose: (() => void) | undefined;
  publish({ phase: "loading" });
  const receive = (event: DaemonEvent) => {
    if (stopped) return;
    publish(event.kind === "status" ? { phase: "connected", status: event.status }
      : { phase: "unavailable", message: event.message });
  };
  void Promise.resolve().then(() => stopped ? () => {} : subscribe(receive)).then(unsubscribe => {
    if (stopped) unsubscribe(); else dispose = unsubscribe;
  }).catch(() => {
    if (!stopped) publish({ phase: "unavailable", message: "Não foi possível iniciar o acompanhamento do daemon. Reabra o aplicativo." });
  });
  return () => {
    stopped = true;
    if (dispose) { dispose(); dispose = undefined; }
  };
}
