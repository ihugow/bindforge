import { useEffect, useId, useRef, type ReactNode, type RefObject } from "react";
import { createPortal } from "react-dom";

let openModals = 0;
let previousOverflow = "";

/** Native dialogs handle focus trapping, inert background and focus restoration.
 * Each nested dialog has its own top-layer entry; Escape dismisses only the top one.
 */
export function Modal({ title, children, onDismiss, busy = false, initialFocus, restoreFocus, compact = false, wide = false }: {
  title: string; children: ReactNode; onDismiss: () => void; busy?: boolean; initialFocus?: string; restoreFocus?: RefObject<HTMLElement | null>; compact?: boolean; wide?: boolean;
}) {
  const ref = useRef<HTMLDialogElement>(null);
  const titleId = useId();
  useEffect(() => {
    const dialog = ref.current!;
    const previousFocus = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    if (openModals++ === 0) { previousOverflow = document.body.style.overflow; document.body.style.overflow = "hidden"; }
    dialog.showModal();
    if (initialFocus) dialog.querySelector<HTMLElement>(initialFocus)?.focus();
    return () => {
      dialog.close();
      // React may temporarily disable the opening button while the modal exists.
      // Restore after the closing commit has re-enabled it.
      queueMicrotask(() => {
        const target = restoreFocus?.current ?? previousFocus;
        if (!dialog.isConnected && target?.isConnected) target.focus();
      });
      if (--openModals === 0) document.body.style.overflow = previousOverflow;
    };
  }, [initialFocus, restoreFocus]);
  return createPortal(<dialog ref={ref} className={`app-modal${compact ? " app-modal-compact" : ""}${wide ? " app-modal-wide" : ""}`}
    aria-labelledby={titleId} aria-modal="true" aria-busy={busy}
    onCancel={event => { event.preventDefault(); event.stopPropagation(); if (!busy) onDismiss(); }}>
    <div className="modal-header"><h2 id={titleId}>{title}</h2>
      <button type="button" className="modal-close" aria-label={`Fechar: ${title}`} disabled={busy} onClick={onDismiss}>×</button>
    </div>
    <div className="modal-content">{children}</div>
  </dialog>, document.body);
}
