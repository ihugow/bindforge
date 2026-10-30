import { useEffect, useId, useRef, useState, type ReactNode } from "react";

export function KeyboardViewport({ label, descriptionId, children }: { label: string; descriptionId?: string; children: ReactNode }) {
  const id = useId();
  const viewport = useRef<HTMLDivElement>(null);
  const drag = useRef<number | null>(null);
  const [metrics, setMetrics] = useState({ offset: 0, max: 0, ratio: 1 });
  useEffect(() => {
    const node = viewport.current;
    if (!node) return;
    const sync = () => setMetrics({ offset: node.scrollLeft, max: Math.max(0, node.scrollWidth - node.clientWidth), ratio: node.clientWidth / Math.max(1, node.scrollWidth) });
    const observer = new ResizeObserver(sync);
    observer.observe(node);
    if (node.firstElementChild) observer.observe(node.firstElementChild);
    node.addEventListener("scroll", sync, { passive: true });
    sync();
    return () => { observer.disconnect(); node.removeEventListener("scroll", sync); };
  }, []);
  const size = Math.max(.08, metrics.ratio);
  const position = metrics.max ? metrics.offset / metrics.max * (1 - size) : 0;
  return <>
    <div ref={viewport} id={id} className="keyboard-scroll" role="region" aria-label={label} aria-describedby={descriptionId} tabIndex={0}>{children}</div>
    {metrics.max > 1 && <div className="keyboard-scrollbar" role="scrollbar" tabIndex={0} aria-label="Rolar teclado horizontalmente" aria-controls={id} aria-orientation="horizontal"
      aria-valuemin={0} aria-valuemax={Math.round(metrics.max)} aria-valuenow={Math.round(metrics.offset)}
      onKeyDown={event => {
        const node = viewport.current;
        if (!node) return;
        const steps: Record<string, number> = { ArrowLeft: -48, ArrowRight: 48, PageUp: -node.clientWidth, PageDown: node.clientWidth };
        if (event.key === "Home" || event.key === "End" || event.key in steps) {
          event.preventDefault();
          node.scrollLeft = event.key === "Home" ? 0 : event.key === "End" ? metrics.max : node.scrollLeft + steps[event.key];
        }
      }}
      onPointerDown={event => {
        if (event.button !== 0) return;
        const node = viewport.current;
        if (!node) return;
        const track = event.currentTarget.getBoundingClientRect();
        const thumbLeft = position * track.width, thumbWidth = size * track.width;
        const x = event.clientX - track.left;
        drag.current = x >= thumbLeft && x <= thumbLeft + thumbWidth ? x - thumbLeft : thumbWidth / 2;
        event.currentTarget.setPointerCapture(event.pointerId);
        event.currentTarget.focus();
        node.scrollLeft = Math.max(0, Math.min(1, (x - drag.current) / (track.width - thumbWidth))) * metrics.max;
        event.preventDefault();
      }}
      onPointerMove={event => {
        if (drag.current === null || !viewport.current) return;
        const track = event.currentTarget.getBoundingClientRect();
        viewport.current.scrollLeft = Math.max(0, Math.min(1, (event.clientX - track.left - drag.current) / (track.width * (1 - size)))) * metrics.max;
      }}
      onPointerUp={() => { drag.current = null; }} onPointerCancel={() => { drag.current = null; }} onLostPointerCapture={() => { drag.current = null; }}>
      <span className="keyboard-scrollbar-thumb" style={{ width: `${size * 100}%`, left: `${position * 100}%` }} />
    </div>}
  </>;
}
