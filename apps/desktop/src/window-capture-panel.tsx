import { AppButton } from "./app-button";
import { useEffect, useState } from "react";
import { Channel, invoke } from "@tauri-apps/api/core";
import { captureWindow, type CaptureEvent, type CaptureStart, type CaptureState } from "./window-capture";

const nativeStart: CaptureStart = async receive => {
  const events = new Channel<CaptureEvent>(); events.onmessage = receive;
  const id = await invoke<number>("start_window_capture", { events });
  return () => { events.onmessage = () => {}; void invoke("stop_window_capture", { id }).catch(() => {}); };
};
export function WindowCapturePanel({ connected, detected, pending, close, start = nativeStart }: {
  connected: boolean; detected: (appId: string) => void; pending: (value: boolean) => void; close: () => void; start?: CaptureStart;
}) {
  const [state, setState] = useState<CaptureState>({ phase: "idle" });
  const [attempt, setAttempt] = useState(0);
  const [enabled, setEnabled] = useState(true);
  useEffect(() => {
    if (!enabled) { setState({ phase: "idle" }); return; }
    if (!connected) { setState({ phase: "error", message: "Conecte o daemon KDE para detectar uma janela." }); return; }
    return captureWindow(start, setState, detected);
  }, [connected, enabled, attempt, start, detected]);
  const waiting = state.phase === "arming" || state.phase === "waiting";
  useEffect(() => { pending(waiting); return () => pending(false); }, [waiting, pending]);
  return <div className="window-capture">
    <p>Quando aparecer “Captura pronta”, clique na janela desejada ou use Alt+Tab. Você tem até 30 segundos. Depois, volte para este formulário.</p>
    <div role={state.phase === "error" ? "alert" : "status"} aria-live="polite">
      {state.phase === "arming" && "Preparando a captura…"}
      {state.phase === "waiting" && <strong>Captura pronta — selecione a janela do aplicativo.</strong>}
      {state.phase === "idle" && "Captura cancelada."}
      {state.phase === "captured" && <>ID identificado e incluído na lista: <code>{state.appId}</code>. Confira a associação antes de salvar o perfil.</>}
      {state.phase === "error" && state.message}
    </div>
    <p className="hint">Requer o daemon e o script de foco do KWin ativos. Jogos Steam identificados aparecem como steam:ID. A detecção não captura teclas ou imagens.</p>
    <div className="button-row">
      {waiting ? <AppButton type="button" onClick={() => setEnabled(false)}>Cancelar captura</AppButton>
        : <AppButton type="button" disabled={!connected} onClick={() => { setEnabled(true); setAttempt(n => n + 1); }}>Capturar outra janela</AppButton>}
      <AppButton type="button" onClick={close}>Fechar captura</AppButton>
    </div>
  </div>;
}
