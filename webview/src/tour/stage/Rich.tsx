import { useEffect, useId, useLayoutEffect, useMemo, useRef, useState } from "react";
import { createPortal } from "react-dom";

import { request } from "../../host.ts";
import { type Size, STEP, type View, fitView, naturalSize, panBy, wheelFactor, zoomAt } from "../diagramView.ts";
import { type MermaidLike, diagramWidth, drawDiagram, linkTarget, mermaidTheme, naturalWidth, renderText, uniqueIds } from "../text.ts";
import { highlightEnv, useHighlighter, useThemeName } from "./useHighlight.ts";

/** What the suite reads of a drawn text: the Markdown it has, and how its diagrams ended (`failed`: the source is shown with a note). */
export interface TextDrawn {
  lists: number;
  tables: number;
  fences: number;
  diagrams: number;
  drawn: number;
  failed: number;
}

/** the padding of a drawn diagram's box (`.tour-diagram-drawn`), both sides */
const DIAGRAM_PAD = 12;

/** Sets the SVG of a drawn diagram to the width the size rule gives for the box it is in (80% of natural at least: a wider one scrolls inside the box). */
function sizeDiagram(place: HTMLElement, natural: number | null) {
  const svg = place.querySelector("svg");
  if (!svg) return;
  const width = diagramWidth(natural ?? 0, place.clientWidth - DIAGRAM_PAD);
  svg.style.maxWidth = "none";
  svg.style.width = `${width}px`;
  svg.style.height = "auto";
}

/** Mermaid, brought in when a step has a diagram and not before: its own chunk. */
const loadMermaid = async (): Promise<MermaidLike> => (await import("mermaid")).default as unknown as MermaidLike;

/**
 * A model's words, drawn as Markdown (`../text.ts`): React puts in the renderer's HTML, which is made from escaped text (`html: false`), so nothing in a step is ever HTML. A
 * ` ```mermaid ` fence is drawn as a diagram by Mermaid (strict: its SVG is sanitised), loaded only now; it fits the pane, a click opens it large, and a diagram that does
 * not parse is its source and one line, never a blank. A link goes to the host (`openLink`) when it is `http(s)`; nothing navigates inside the webview.
 */
export function Rich({ text, className, onDrawn }: { text: string; className?: string; onDrawn?: (d: TextDrawn) => void }) {
  const hl = useHighlighter();
  const theme = useThemeName();
  const rendered = useMemo(() => renderText(text, highlightEnv(hl, theme)), [text, hl, theme]);
  const box = useRef<HTMLDivElement>(null);
  const prefix = useId().replace(/[^a-zA-Z0-9]/g, "");
  const [large, setLarge] = useState<string | null>(null);
  const drawnNow = useRef<TextDrawn | null>(null);
  /** what Mermaid drew, before the size was set on it: a click opens this one, at its own size */
  const original = useRef(new WeakMap<Element, { svg: string; natural: number | null }>());

  // the diagrams: drawn into their placeholders once the HTML is in; a new text is a new HTML, and the old SVGs go with it
  useEffect(() => {
    const root = box.current;
    if (!root) return;
    const places = Array.from(root.querySelectorAll<HTMLElement>(".tour-diagram[data-diagram]"));
    const counts: TextDrawn = {
      lists: root.querySelectorAll("ul, ol").length,
      tables: root.querySelectorAll("table").length,
      fences: root.querySelectorAll("pre").length - places.length,
      diagrams: places.length,
      drawn: 0,
      failed: 0,
    };
    drawnNow.current = counts;
    onDrawn?.({ ...counts });
    let live = true;
    // a pane that is dragged wider or narrower: the diagrams follow the rule again
    const sizes = new ResizeObserver((entries) => {
      for (const e of entries) sizeDiagram(e.target as HTMLElement, original.current.get(e.target)?.natural ?? null);
    });
    places.forEach(async (place, n) => {
      const source = rendered.diagrams[Number(place.dataset.diagram)];
      const result = await drawDiagram(loadMermaid, source, `tour-${prefix}-${n}`, mermaidTheme(theme));
      if (!live || !place.isConnected) return;
      if (result.ok) {
        place.innerHTML = result.svg; // Mermaid's SVG with `securityLevel: "strict"`: sanitised
        const natural = naturalWidth(result.svg);
        original.current.set(place, { svg: result.svg, natural });
        sizeDiagram(place, natural);
        sizes.observe(place);
        place.classList.add("tour-diagram-drawn");
        place.tabIndex = 0;
        place.title = "Click to see it large";
        counts.drawn += 1;
      } else {
        place.classList.add("tour-diagram-failed");
        place.insertAdjacentHTML("beforeend", `<p class="tour-diagram-note"></p>`);
        place.querySelector(".tour-diagram-note")!.textContent = result.note;
        counts.failed += 1;
      }
      onDrawn?.({ ...counts });
    });
    return () => {
      live = false;
      sizes.disconnect();
    };
  }, [rendered.html]); // eslint-disable-line react-hooks/exhaustive-deps -- `onDrawn` is the stage's note

  const onClick = (e: React.MouseEvent) => {
    const target = e.target instanceof Element ? e.target : null;
    const a = target?.closest("a");
    if (a) {
      e.preventDefault();
      const href = linkTarget(a.getAttribute("data-href"));
      if (href) void request("openLink", { href, from: "" }).catch(() => {});
      return;
    }
    const diagram = target?.closest(".tour-diagram-drawn");
    if (diagram) setLarge(original.current.get(diagram)?.svg ?? diagram.innerHTML);
  };
  const onKeyDown = (e: React.KeyboardEvent) => {
    const diagram = e.target instanceof Element ? e.target.closest(".tour-diagram-drawn") : null;
    if (diagram && (e.key === "Enter" || e.key === " ")) {
      e.preventDefault();
      setLarge(original.current.get(diagram)?.svg ?? diagram.innerHTML);
    }
  };

  return (
    <>
      <div ref={box} className={`tour-md${className ? ` ${className}` : ""}`} onClick={onClick} onKeyDown={onKeyDown} dangerouslySetInnerHTML={{ __html: rendered.html }} />
      {large !== null && <Enlarged svg={large} onClose={() => setLarge(null)} />}
    </>
  );
}

/**
 * The diagram large: over the whole window, Esc or a click outside it closes it. A portal, so no pane's overflow clips it. It opens fitted whole (scaled down, aspect kept; `diagramView.ts`),
 * and is zoomed with the wheel or `+` and `−` (about the pointer), moved by dragging, and fitted again with `0` or **Fit**; a window that is resized fits it again while it is untouched.
 */
function Enlarged({ svg, onClose }: { svg: string; onClose: () => void }) {
  const natural = useMemo(() => naturalSize(svg) ?? { width: 600, height: 400 }, [svg]);
  const html = useMemo(() => uniqueIds(svg, "-large"), [svg]);
  const port = useRef<HTMLDivElement>(null);
  const [room, setRoom] = useState<Size>({ width: 0, height: 0 });
  const [view, setView] = useState<View>({ scale: 1, x: 0, y: 0 });
  const [dragging, setDragging] = useState(false);
  /** the reader has not zoomed or moved it: a new room fits it again */
  const untouched = useRef(true);
  const latest = useRef({ view, room });
  latest.current = { view, room };

  useLayoutEffect(() => {
    const el = port.current;
    if (!el) return;
    const read = () => {
      const now = { width: el.clientWidth, height: el.clientHeight };
      setRoom(now);
      if (untouched.current) setView(fitView(natural, now));
    };
    read();
    const watch = new ResizeObserver(read);
    watch.observe(el);
    return () => watch.disconnect();
  }, [natural]);

  const zoomBy = (factor: number, at?: { x: number; y: number }) => {
    const { view, room } = latest.current;
    untouched.current = false;
    setView(zoomAt(view, factor, at ?? { x: room.width / 2, y: room.height / 2 }, natural, room));
  };
  const fit = () => {
    untouched.current = true;
    setView(fitView(natural, latest.current.room));
  };

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const zoom = e.key === "+" || e.key === "=" ? STEP : e.key === "-" || e.key === "_" ? 1 / STEP : null;
      if (e.key === "Escape") {
        e.preventDefault();
        e.stopImmediatePropagation();
        onClose();
      } else if (zoom !== null || e.key === "0") {
        e.preventDefault();
        e.stopImmediatePropagation();
        if (zoom !== null) zoomBy(zoom);
        else fit();
      }
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, [onClose]); // eslint-disable-line react-hooks/exhaustive-deps -- `zoomBy` and `fit` read `latest`
  // the wheel zooms about the pointer; a native listener, because React's is passive and cannot stop the page behind from scrolling
  useEffect(() => {
    const el = port.current;
    if (!el) return;
    const onWheel = (e: WheelEvent) => {
      e.preventDefault();
      const box = el.getBoundingClientRect();
      zoomBy(wheelFactor(e.deltaMode === 1 ? e.deltaY * 33 : e.deltaY), { x: e.clientX - box.left, y: e.clientY - box.top });
    };
    el.addEventListener("wheel", onWheel, { passive: false });
    return () => el.removeEventListener("wheel", onWheel);
  }, []); // eslint-disable-line react-hooks/exhaustive-deps -- `zoomBy` reads `latest`

  const onPointerDown = (e: React.PointerEvent) => {
    if (e.button !== 0 || (e.target as Element).closest("button")) return;
    e.preventDefault();
    const el = e.currentTarget as HTMLElement;
    el.setPointerCapture(e.pointerId);
    setDragging(true);
    let [x, y] = [e.clientX, e.clientY];
    const move = (m: PointerEvent) => {
      const { view, room } = latest.current;
      untouched.current = false;
      setView(panBy(view, m.clientX - x, m.clientY - y, natural, room));
      [x, y] = [m.clientX, m.clientY];
    };
    const end = () => {
      el.removeEventListener("pointermove", move);
      el.removeEventListener("pointerup", end);
      el.removeEventListener("pointercancel", end);
      setDragging(false);
    };
    el.addEventListener("pointermove", move);
    el.addEventListener("pointerup", end);
    el.addEventListener("pointercancel", end);
  };

  return createPortal(
    <div className="tour-overlay" role="dialog" aria-label="Diagram" data-part="diagram-large" onClick={onClose}>
      <button type="button" className="tour-overlay-close" aria-label="Close the diagram" onClick={onClose}>
        ✕
      </button>
      <div
        ref={port}
        className={`tour-overlay-body${dragging ? " tour-overlay-dragging" : ""}`}
        data-part="diagram-port"
        data-scale={view.scale.toFixed(3)}
        onClick={(e) => e.stopPropagation()}
        onPointerDown={onPointerDown}
      >
        <div
          className="tour-overlay-canvas"
          data-part="diagram-canvas"
          style={{ width: natural.width, height: natural.height, transform: `translate(${view.x}px, ${view.y}px) scale(${view.scale})` }}
          dangerouslySetInnerHTML={{ __html: html }}
        />
        <div className="tour-overlay-tools" data-part="diagram-tools">
          <button type="button" aria-label="Zoom out" title="Zoom out (−)" onClick={() => zoomBy(1 / STEP)}>
            −
          </button>
          <button type="button" aria-label="Fit the diagram" title="Fit the diagram (0)" onClick={fit}>
            Fit
          </button>
          <button type="button" aria-label="Zoom in" title="Zoom in (+)" onClick={() => zoomBy(STEP)}>
            +
          </button>
        </div>
      </div>
    </div>,
    document.body,
  );
}
