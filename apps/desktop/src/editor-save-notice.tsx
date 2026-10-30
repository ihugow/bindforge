import { AppIcon } from "./app-icon";
import { AppButton } from "./app-button";
import { useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";

export function EditorSaveNotice({ dirty, busy, discard, save }: {
  dirty: boolean; busy: boolean; discard: () => void; save: () => void;
}) {
  const previousDirty = useRef(dirty);
  const discarded = useRef(false);
  const [settled, setSettled] = useState(false);
  useEffect(() => {
    let timeout: ReturnType<typeof setTimeout> | undefined;
    if (dirty) setSettled(false);
    else if (previousDirty.current) {
      setSettled(true);
      timeout = setTimeout(() => setSettled(false), 3000);
    }
    previousDirty.current = dirty;
    return () => { if (timeout) clearTimeout(timeout); };
  }, [dirty]);
  if (!dirty && !busy && !settled) return null;
  return createPortal(<footer className="editor-save-notice" data-pending={dirty || busy} aria-label="Alterações do dispositivo">
      <span className="editor-notice-indicator" aria-hidden="true"><AppIcon name={dirty || busy ? "dot" : "check"} /></span>
      <span role="status">{busy ? "Salvando alterações…" : dirty ? "Alterações não salvas" : discarded.current ? "Alterações descartadas" : "Alterações salvas"}</span>
      {(dirty || busy) && <div className="button-row">
        <AppButton type="button" className="secondary-button" disabled={busy} onClick={() => { discarded.current = true; discard(); }}>Descartar</AppButton>
        <AppButton type="button" disabled={busy} onClick={() => { discarded.current = false; save(); }}>{busy ? "Salvando…" : "Salvar alterações"}</AppButton>
      </div>}
    </footer>, document.body);
}
