export type CaptureEvent = { kind: "captured"; app_id: string } | { kind: "error"; message: string };
export type CaptureState = { phase: "idle" | "arming" | "waiting" } | { phase: "captured"; appId: string } | { phase: "error"; message: string };
export type CaptureStart = (receive: (event: CaptureEvent) => void) => Promise<() => void>;

// A quick result can precede the start response. Disposing while start is pending
// must cancel its eventual handle and ignore every late result.
export function captureWindow(start: CaptureStart, publish: (state: CaptureState) => void, detected: (appId: string) => void): () => void {
  let active = true;
  let finished = false;
  let stop: (() => void) | undefined;
  publish({ phase: "arming" });
  Promise.resolve().then(() => active ? start(event => {
    if (!active || finished) return;
    finished = true;
    if (event.kind === "captured" && event.app_id.toLowerCase() === "gamescope") {
      publish({ phase: "error", message: "Só foi identificado o Gamescope. Não foi possível identificar o jogo da Steam; nenhuma associação foi adicionada. Confira se o script KWin está atualizado e tente novamente. Se persistir, este modo de inicialização ainda não fornece uma identificação individual." });
    } else if (event.kind === "captured" && event.app_id.trim() && new TextEncoder().encode(event.app_id).length <= 4096 && !/[\x00-\x1f\x7f-\x9f]/.test(event.app_id)) {
      detected(event.app_id);
      publish({ phase: "captured", appId: event.app_id });
    } else {
      publish({ phase: "error", message: event.kind === "error" ? event.message : "O KDE retornou um ID inválido. Tente outra janela." });
    }
    const cancel = stop; stop = undefined; cancel?.();
  }) : undefined).then(cancel => {
    if (!cancel) return;
    if (!active || finished) { cancel(); return; }
    stop = cancel;
    publish({ phase: "waiting" });
  }).catch(error => {
    if (active && !finished) { finished = true; publish({ phase: "error", message: String(error) }); }
  });
  return () => { active = false; stop?.(); stop = undefined; };
}
