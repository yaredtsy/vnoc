import { useRef, useState } from "react";

/**
 * The sash on the top edge of every panel after the first (06 §4.1): a 4 px hit area centred on the border and over the content, so it takes no height;
 * invisible at rest, drawn in the sash colour after 300 ms of hover or at once while dragging; `row-resize`. Reports the drag; `Panels` does the arithmetic.
 */
export function Splitter({ onStart, onMove, onEnd, onReset }: { onStart: () => void; onMove: (dy: number) => void; onEnd: (dy: number) => void; onReset: () => void }) {
  const from = useRef<number | null>(null);
  const [dragging, setDragging] = useState(false);
  return (
    <div
      role="separator"
      aria-orientation="horizontal"
      title="Drag to resize; double-click to reset"
      data-dragging={dragging}
      className="absolute inset-x-0 -top-0.5 z-20 h-1 cursor-row-resize bg-transparent transition-colors duration-0 hover:bg-[color:var(--sash)] hover:delay-300 data-[dragging=true]:bg-[color:var(--sash)] data-[dragging=true]:delay-0"
      onPointerDown={(e) => {
        e.stopPropagation(); // the splitter only moves the line: it does not activate the panel
        e.currentTarget.setPointerCapture(e.pointerId);
        from.current = e.clientY;
        setDragging(true);
        onStart();
      }}
      onPointerMove={(e) => from.current !== null && onMove(e.clientY - from.current)}
      onPointerUp={(e) => {
        if (from.current === null) return;
        const dy = e.clientY - from.current;
        from.current = null;
        setDragging(false);
        onEnd(dy);
      }}
      onPointerCancel={() => {
        from.current = null;
        setDragging(false);
      }}
      onDoubleClick={onReset}
    />
  );
}
