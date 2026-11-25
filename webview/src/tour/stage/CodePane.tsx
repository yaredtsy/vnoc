import { useEffect, useLayoutEffect, useRef, useState } from "react";
import type { ThemedToken } from "shiki/core";

import { type CodeLook, type LineLook, lineLook } from "../layout.ts";
import { type PaneView, moreLit, nextPage, scrollTarget } from "../scroll.ts";
import type { Shown } from "./useCode.ts";
import { useHighlighter, useThemeName, useTokens } from "./useHighlight.ts";
import { languageOf } from "../../lib/highlighter.ts";

/** One row of the code is this high (px): the scroll rule counts in lines. */
export const LINE_H = 18;

export interface CodeDrawn {
  scrolledTo: number;
  lit: number[];
  thin: number[];
  /** how many tokens were drawn in a colour (0: plain) */
  coloured: number;
  /** the step and the code text this was drawn for: the stage tells the suite what it drew only when these are the ones it holds, never a step behind */
  step: string | null;
  text: string | null;
}

/**
 * The code pane (design 03 §2.2): the node's code, as it is now, with its real line numbers. The step's lines are lit and the rest dim (the look of each line is
 * `lineLook`); it scrolls by itself by the rule of `scroll.ts` and otherwise only by hand. Long lines are not wrapped. The syntax colours are the cards' (Shiki, the same
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

  const measure = () => {
    const el = box.current;
    if (el) setView({ top: Math.round(el.scrollTop / LINE_H), rows: Math.max(1, Math.floor(el.clientHeight / LINE_H)) });
  };

  // a new step: the rule of scroll.ts, once the code of this stop is there
  useLayoutEffect(() => {
    const el = box.current;
    if (!el || !shown) return;
    const rows = Math.max(1, Math.floor(el.clientHeight / LINE_H));
    const top = Math.round(el.scrollTop / LINE_H);
    const target = scrollTarget({ newStop: previous.current !== p.stopId, lit: p.look === "lit" || p.look === "detail" ? litIndex : null, view: { top, rows }, pointerOver: pointerOver.current });
    previous.current = p.stopId;
    if (target !== null) el.scrollTop = target * LINE_H;
    measure();
  }, [p.stopId, p.stepId, shown?.text]); // eslint-disable-line react-hooks/exhaustive-deps

  const looks: LineLook[] = shown ? shown.lines.map((_, i) => lineLook(p.look, first + i, first, p.lit, p.parentLit)) : [];
  const coloured = shown && tokens ? tokens.reduce((n, line, i) => (line.map((t) => t.content).join("") === shown.lines[i] ? n + line.filter((t) => t.color).length : n), 0) : 0;
  useEffect(() => {
    p.onDrawn({ scrolledTo: view.top, lit: looks.flatMap((l, i) => (l === "lit" ? [first + i] : [])), thin: looks.flatMap((l, i) => (l === "thin" ? [first + i] : [])), coloured, step: p.stepId, text: shown?.text ?? null });
  }, [p.stepId, shown?.text, p.look, p.lit?.[0], p.lit?.[1], p.parentLit?.[0], p.parentLit?.[1], view.top, coloured]); // eslint-disable-line react-hooks/exhaustive-deps

  if (p.gone) return <div className="stage-code stage-empty">This is no longer in the code.</div>;
  if (p.code === null) return <div className="stage-code stage-empty">Reading the code…</div>;
  if ("error" in p.code) return <div className="stage-code stage-empty">{p.code.error}</div>;
  const more = p.look === "lit" || p.look === "detail" ? moreLit(litIndex, view) : 0;
  return (
    <div className={`stage-code-wrap${p.written ? " stage-written" : ""}`} data-written={p.written?.commit}>
      {p.written && (
        <span className="stage-written-label" data-part="written-label">
          as written · {p.written.commit}
        </span>
      )}
      <div ref={box} className="stage-code" data-part="code" onScroll={measure} onPointerEnter={() => (pointerOver.current = true)} onPointerLeave={() => (pointerOver.current = false)}>
        {shown!.lines.map((text, i) => (
          <div key={i} className={`stage-line stage-${looks[i]}${p.changed && looks[i] === "lit" ? " stage-changed" : ""}`} data-line={first + i}>
            <span className="stage-gutter">{first + i}</span>
            <span className="stage-text">{text === "" ? " " : <Tokens text={text} tokens={tokens?.[i]} />}</span>
            {p.calls.has(first + i) && (
              <button type="button" className="stage-callto" title={`A stop of this tour: ${p.calls.get(first + i)!.name}`} onClick={() => p.onCall(p.calls.get(first + i)!.id)}>
                ⇢ stop {p.calls.get(first + i)!.number}
              </button>
            )}
          </div>
        ))}
      </div>
      {more > 0 && (
        <button type="button" className="stage-more" onClick={() => box.current && (box.current.scrollTop = nextPage(view, shown!.lines.length) * LINE_H)}>
          ▾ {more} more lit {more === 1 ? "line" : "lines"}
        </button>
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
