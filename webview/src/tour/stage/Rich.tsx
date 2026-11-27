import { useEffect, useId, useMemo, useRef, useState } from "react";
import { createPortal } from "react-dom";

import { request } from "../../host.ts";
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

/** The diagram large: over the whole window, Esc or a click closes it. A portal, so no pane's overflow clips it. */
function Enlarged({ svg, onClose }: { svg: string; onClose: () => void }) {
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.preventDefault();
        e.stopImmediatePropagation();
        onClose();
      }
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, [onClose]);
  return createPortal(
    <div className="tour-overlay" role="dialog" aria-label="Diagram" data-part="diagram-large" onClick={onClose}>
      <button type="button" className="tour-overlay-close" aria-label="Close the diagram" onClick={onClose}>
        ✕
      </button>
      <div className="tour-overlay-body" onClick={(e) => e.stopPropagation()} dangerouslySetInnerHTML={{ __html: uniqueIds(svg, "-large") }} />
    </div>,
    document.body,
  );
}
