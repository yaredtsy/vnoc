import { useEffect, useLayoutEffect, useRef, useState } from "react";
import type { ThemedToken } from "shiki/core";

import { type CodeLook, type LineLook, lineLook } from "../layout.ts";
import { type PaneView, moreLit, nextPage, scrollTarget, viewOf } from "../scroll.ts";
import type { Shown } from "./useCode.ts";
import { useHighlighter, useThemeName, useTokens } from "./useHighlight.ts";
import { languageOf } from "../../lib/highlighter.ts";

/** One row of the code is this high (px): the scroll rule counts in lines. */
export const LINE_H = 18;

/** When the reader last moved with a key (`→ ← ↓ ↑`, Space): a move by key scrolls the code to the lit lines wherever the pointer is. The stage sets it, the pane reads it. */
export const keyMoved = { at: 0 };
const KEY_MOVE_MS = 2000;

export interface CodeDrawn {
  scrolledTo: number;
  lit: number[];
  thin: number[];
  /** how many tokens were drawn in a colour (0: plain) */
  coloured: number;
  /** the step and the code text this was drawn for: the stage tells the suite what it drew only when these are the ones it holds, never a step behind */
  step: string | null;
  text: string | null;
  /** the chip's number: the lit lines below the pane's bottom (0: no chip) */
  more: number;
}

/**
 * The code pane (design 03 §2.2): the node's code, as it is now, with its real line numbers. The step's lines are lit and the rest dim (the look of each line is
 * `lineLook`); it scrolls by itself by the rule of `scroll.ts` and otherwise only by hand. A long line wraps (soft wrap) and the wrapped part is indented two columns past the
 * line's own indent, so nothing is cut off at the pane's edge and the code still reads as code; the scroll rule counts lines, and a wrapped one is one line. The syntax colours are the cards' (Shiki, the same
 * highlighter and themes), under the marks: a lit line is coloured and has its background and bar, a dim one is the same colours at less strength. A file whose language the
 * highlighter does not have is drawn plain, as before.
 */
export function CodePane(p: {
  code: Shown;
  look: CodeLook;
  /** the step's lines now (file lines); null: nothing is lit */
  lit: readonly [number, number] | null;
  parentLit: readonly [number, number] | null;
  /** the step's own mark says the code changed after it was written: the lit lines get a dashed bar and no background */
  changed: boolean;
  /** the code is the pinned commit's (`vnoc/tourCode`): the pane has its own background and the label `as written · <commit>` on its top edge */
  written: { commit: string } | null;
  /** the node's file, for its language */
  path: string | null;
  stopId: string;
  stepId: string;
  gone: boolean;
  /** the calls of this code that are later stops of the tour, by file line (`callLines`): the line gets a jump to that stop */
  calls: ReadonlyMap<number, { id: string; number: number; name: string }>;
  onCall: (stop: string) => void;
  onDrawn: (d: CodeDrawn) => void;
}) {
  const box = useRef<HTMLDivElement>(null);
  const pointerOver = useRef(false);
  const previous = useRef<string | null>(null);
  const [view, setView] = useState<PaneView>({ top: 0, rows: 9 });
  const shown = p.code && "lines" in p.code ? p.code : null;
  const hl = useHighlighter();
  const theme = useThemeName();
  const tokens = useTokens(hl, shown?.text ?? null, p.path ? languageOf(p.path) : "plaintext", theme);
  const first = shown?.first ?? 1;
  const litIndex = p.lit ? ([p.lit[0] - first, p.lit[1] - first] as const) : null;

  /** the lines' places in the pane, from its own top (a wrapped line is taller than `LINE_H`): what is in view is counted in lines, not in pixels */
  const lines = (el: HTMLElement) => {
    const rows = Array.from(el.querySelectorAll<HTMLElement>(".stage-line"));
    const origin = el.getBoundingClientRect().top - el.scrollTop;
    const boxes = rows.map((row) => row.getBoundingClientRect());
    return { rows, tops: boxes.map((b) => b.top - origin), heights: boxes.map((b) => b.height) };
  };
  const measure = () => {
    const el = box.current;
    if (!el) return;
    const { tops, heights } = lines(el);
    setView((old) => {
      const now = viewOf(tops, heights, el.scrollTop, el.clientHeight);
      return old.top === now.top && old.rows === now.rows ? old : now;
    });
  };
  // the pane changes its width (the stage stacks, a divider is dragged): lines wrap differently, and what is in view is measured again
  useEffect(() => {
    const el = box.current;
    if (!el) return;
    const watch = new ResizeObserver(() => measure());
    watch.observe(el);
    return () => watch.disconnect();
  }, [shown?.text]); // eslint-disable-line react-hooks/exhaustive-deps

  // the pointer is over the pane while it is, and the state ends when the pointer leaves the pane or the window (it may leave with no `pointerleave` of the pane's: a key, a blur)
  useEffect(() => {
    const el = box.current;
    if (!el) return;
    const enter = () => (pointerOver.current = true);
    const leave = () => (pointerOver.current = false);
    el.addEventListener("pointerenter", enter);
    el.addEventListener("pointerleave", leave);
    document.documentElement.addEventListener("pointerleave", leave);
    window.addEventListener("blur", leave);
    return () => {
      el.removeEventListener("pointerenter", enter);
      el.removeEventListener("pointerleave", leave);
      document.documentElement.removeEventListener("pointerleave", leave);
      window.removeEventListener("blur", leave);
    };
  }, [shown?.text]);

  // a new step: the rule of scroll.ts, once the code of this stop is there
  useLayoutEffect(() => {
    const el = box.current;
    if (!el || !shown) return;
    const { rows: els, tops, heights } = lines(el);
    const now = viewOf(tops, heights, el.scrollTop, el.clientHeight);
    const target = scrollTarget({ newStop: previous.current !== p.stopId, lit: p.look === "lit" || p.look === "detail" ? litIndex : null, view: now, pointerOver: pointerOver.current && Date.now() - keyMoved.at > KEY_MOVE_MS });
    previous.current = p.stopId;
    if (target !== null) el.scrollTop = els[target] ? tops[target] : target * LINE_H;
    el.scrollLeft = 0; // a sideways scroll of the last step is not kept
    measure();
  }, [p.stopId, p.stepId, shown?.text]); // eslint-disable-line react-hooks/exhaustive-deps

  const more = p.look === "lit" || p.look === "detail" ? moreLit(litIndex, view) : 0;
  const looks: LineLook[] = shown ? shown.lines.map((_, i) => lineLook(p.look, first + i, first, p.lit, p.parentLit)) : [];
  const coloured = shown && tokens ? tokens.reduce((n, line, i) => (line.map((t) => t.content).join("") === shown.lines[i] ? n + line.filter((t) => t.color).length : n), 0) : 0;
  useEffect(() => {
    p.onDrawn({ scrolledTo: view.top, lit: looks.flatMap((l, i) => (l === "lit" ? [first + i] : [])), thin: looks.flatMap((l, i) => (l === "thin" ? [first + i] : [])), coloured, step: p.stepId, text: shown?.text ?? null, more });
  }, [p.stepId, shown?.text, p.look, p.lit?.[0], p.lit?.[1], p.parentLit?.[0], p.parentLit?.[1], view.top, coloured, more]); // eslint-disable-line react-hooks/exhaustive-deps

  if (p.gone) return <div className="stage-code stage-empty">This is no longer in the code.</div>;
  if (p.code === null) return <div className="stage-code stage-empty">Reading the code…</div>;
  if ("error" in p.code) return <div className="stage-code stage-empty">{p.code.error}</div>;
  return (
    <div className={`stage-code-wrap${p.written ? " stage-written" : ""}`} data-written={p.written?.commit}>
      {/* the label has a strip of its own on the pane's top edge: the code starts under it, and none of it is covered */}
      {p.written && (
        <div className="stage-written-strip" data-part="written-strip">
          <span className="stage-written-label" data-part="written-label">
            as written · {p.written.commit}
          </span>
        </div>
      )}
      <div ref={box} className="stage-code" data-part="code" onScroll={measure}>
        {shown!.lines.map((text, i) => (
          <div key={i} className={`stage-line stage-${looks[i]}${p.changed && looks[i] === "lit" ? " stage-changed" : ""}`} data-line={first + i}>
            <span className="stage-gutter">{first + i}</span>
            <span className="stage-text" style={{ "--lead": text.length - text.trimStart().length } as React.CSSProperties}>
              {text === "" ? " " : <Tokens text={text} tokens={tokens?.[i]} />}
            </span>
            {p.calls.has(first + i) && (
              <button type="button" className="stage-callto" title={`A stop of this tour: ${p.calls.get(first + i)!.name}`} onClick={() => p.onCall(p.calls.get(first + i)!.id)}>
                ⇢ stop {p.calls.get(first + i)!.number}
              </button>
            )}
          </div>
        ))}
      </div>
      {/* the chip has a row of its own under the code (the pane is a column): it covers none of the code */}
      {more > 0 && (
        <div className="stage-more-row" data-part="more-row">
          <button type="button" className="stage-more" onClick={() => {
              const el = box.current;
              if (!el) return;
              const { tops } = lines(el);
              const page = nextPage(view, shown!.lines.length);
              el.scrollTop = tops[page] ?? page * LINE_H;
            }}>
            ▾ {more} more lit {more === 1 ? "line" : "lines"}
          </button>
        </div>
      )}
    </div>
  );
}

/** One line's tokens, each in its colour (`ThemedToken`: `fontStyle` 1 is italic, 2 bold, 4 underline); the text itself when the line has none. */
function Tokens({ text, tokens }: { text: string; tokens: ThemedToken[] | undefined }) {
  if (!tokens || tokens.length === 0 || tokens.map((t) => t.content).join("") !== text) return <>{text}</>;
  return (
    <>
      {tokens.map((t, i) => (
        <span
          key={i}
          className="stage-token"
          style={{ color: t.color, fontStyle: t.fontStyle && t.fontStyle & 1 ? "italic" : undefined, fontWeight: t.fontStyle && t.fontStyle & 2 ? 700 : undefined, textDecoration: t.fontStyle && t.fontStyle & 4 ? "underline" : undefined }}
        >
          {t.content}
        </span>
      ))}
    </>
  );
}
