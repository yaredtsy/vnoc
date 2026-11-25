import { useCallback, useEffect, useRef, useState } from "react";

import { findNode } from "@vnoc/shared/tree.ts";
import type { TourView } from "@vnoc/shared/tours.ts";
import { Codicon } from "@/components/Codicon.tsx";
import { TEXT_FIELD } from "../node-card/codeField.ts";
import { on, post, request } from "../host.ts";
import { useData } from "../state/data.ts";
import { dispatch } from "../state/session.ts";
import { setStageFolded, setStageHeight, setStageText, useView } from "../state/view.ts";
import { type Current, currentOf, keyMove, lookOf, positionWords } from "./layout.ts";
import { go, loadTour, useTour } from "./store.ts";
import { CodePane, type CodeDrawn, LINE_H } from "./stage/CodePane.tsx";
import { Contents } from "./stage/Contents.tsx";
import { Cover, End } from "./stage/Cover.tsx";
import { Foot } from "./stage/Foot.tsx";
import { Header } from "./stage/Header.tsx";
import { TextPane } from "./stage/TextPane.tsx";
import { useCode } from "./stage/useCode.ts";
import "./stage.css";

/** The least the dock is (the header and five lines of code) and the most (45% of the canvas): design 03 §6. */
const MIN_H = 30 + 5 * LINE_H;
const MAX_SHARE = 0.45;
const TEXT_MIN = 320;
const TEXT_MAX = 460;

const clamp = (n: number, lo: number, hi: number) => Math.min(hi, Math.max(lo, n));

/** Drags with the pointer until it is released: `move` gets how far it went from where it started. */
function drag(e: React.PointerEvent, move: (dx: number, dy: number) => void) {
  e.preventDefault();
  const [x0, y0] = [e.clientX, e.clientY];
  const onMove = (m: PointerEvent) => move(m.clientX - x0, m.clientY - y0);
  const stop = () => {
    window.removeEventListener("pointermove", onMove);
    window.removeEventListener("pointerup", stop);
  };
  window.addEventListener("pointermove", onMove);
  window.addEventListener("pointerup", stop);
}

/**
 * The stage (docs/26-tours/design/03-stage.md): the dock under the map while the active context is a tour. It draws the view and the place the host holds: the header,
 * the code of the stop with the step's lines lit, the step's words, the foot. It moves only by asking the core (`tourGo`, through the host), and draws what the host
 * sends back; it never works out a next step. The cover and the end card are places (`place.at`), the rest is the step's `kind`.
 */
export function Stage({ tourId, contextId }: { tourId: string; contextId: string }) {
  const structure = useData((s) => s.structure);
  const held = useTour();
  const height = useView((v) => v.stageHeight);
  const textWidth = useView((v) => v.stageText);
  const folded = useView((v) => v.stageFolded);
  const [drawn, setDrawn] = useState<CodeDrawn>({ scrolledTo: 0, lit: [], thin: [] });

  useEffect(() => {
    let wanted = true;
    loadTour(tourId, () => wanted);
    return () => {
      wanted = false;
    };
  }, [tourId]);

  const answer = held.id === tourId ? held.view : null;
  const view: TourView | null = answer && "stops" in answer ? answer : null;
  const current: Current | null = view ? currentOf(view) : null;
  const look = current ? lookOf(current) : null;
  const stop = current?.stop ?? null;
  const step = current?.step ?? null;
  const node = structure && stop?.key ? (findNode(structure.root, stop.key) ?? null) : null;
  const wantsCode = !!node && look !== null && look.code !== "none" && look.code !== "rows";
  const code = useCode(node && wantsCode ? node.path : null, node?.range ?? null, wantsCode);

  // what a click or a key does: a move for the core, Go to Code, closing the tour
  const act = useRef<Record<string, () => void>>({});
  act.current = {
    next: () => go(tourId, "next"),
    start: () => go(tourId, "next"),
    prev: () => go(tourId, "prev"),
    back: () => go(tourId, "prev"),
    cover: () => go(tourId, "cover"),
    again: () => go(tourId, "cover"),
    end: () => go(tourId, "end"),
    closeTour: () => dispatch({ type: "closeContext", id: contextId }, contextId),
    goToCode: () => {
      if (!node) return;
      if (node.kind === "folder") return void request("goToCode", { key: node.id }).catch(() => {});
      const line = (step?.lines?.[0] ?? (node.range ? node.range.start.line + 1 : 1)) - 1;
      void request("openAt", { path: node.path, range: { start: { line, character: 0 }, end: { line, character: 0 } } }).catch(() => {});
    },
  };
  const jump = useCallback((to: string) => go(tourId, to), [tourId]);

  // the keys; the capture phase and `stopImmediatePropagation`, so that the canvas's own arrows and Space do not also move a card
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const target = e.target instanceof Element ? e.target : null;
      if (target?.closest(TEXT_FIELD)) return;
      const move = keyMove(e.key, { ctrl: e.ctrlKey, meta: e.metaKey, alt: e.altKey });
      if (!move || (e.key === " " && target?.closest("button, a"))) return; // Space on a button is the button's
      e.preventDefault();
      e.stopImmediatePropagation();
      act.current[move]?.();
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, []);
  // the extension's test suite cannot press keys or click in a webview: it sends the same key, and the same button
  useEffect(
    () =>
      on("viewCommand", ({ name, key }) => {
        if (name === "stageKey" && key) document.body.dispatchEvent(new KeyboardEvent("keydown", { key, bubbles: true, cancelable: true }));
        if (name === "stageClick" && key) act.current[key]?.();
      }),
    [],
  );

  // what the suite reads of what was drawn
  useEffect(() => {
    if (!view || !current) return;
    post({
      type: "stageDrawn",
      tour: view.id,
      mode: current.mode,
      stop: stop?.id ?? null,
      step: step?.id ?? null,
      kind: step?.kind ?? null,
      header: stop ? `${stop.planned ? "◌" : "●"} ${stop.number} ${stop.name} ${stop.path}` : view.title,
      title: current.mode === "step" ? (step?.title ?? null) : view.title,
      text: step?.text ?? null,
      position: positionWords(view),
      next: view.place.at === "cover" ? "Start" : view.place.position && view.place.position.stop === view.place.position.of && view.place.position.step === view.place.position.steps ? "Finish" : "Next",
      code: code && "lines" in code ? { first: code.first, text: code.text } : null,
      // what the code pane drew: nothing when there is no code pane (the cover, the end, contents)
      lit: wantsCode ? drawn.lit : [],
      thin: wantsCode ? drawn.thin : [],
      scrolledTo: wantsCode ? drawn.scrolledTo : 0,
    });
  }, [view, code, drawn.scrolledTo, drawn.lit.join(","), drawn.thin.join(",")]); // eslint-disable-line react-hooks/exhaustive-deps

  const style = { height: folded ? 30 : height };
  const grip = (
    <div
      className="stage-grip"
      title="Drag to change the height"
      onPointerDown={(e) => {
        const max = Math.floor(((e.currentTarget.closest(".stage") as HTMLElement | null)?.parentElement?.clientHeight ?? 800) * MAX_SHARE);
        const start = height;
        drag(e, (_dx, dy) => setStageHeight(clamp(start - dy, MIN_H, Math.max(MIN_H, max))));
      }}
    />
  );

  if (!view) {
    return (
      <div className="stage" style={{ height: 30 }} data-stage="loading">
        <div className="stage-header">
          <span className="stage-dimtext">{answer && "unreadable" in answer ? `This tour cannot be read${answer.unreadable ? `: ${answer.unreadable}` : "."}` : "Opening the tour…"}</span>
          <span className="stage-grow" />
          <button type="button" className="stage-icon" title="Close Tour" aria-label="Close Tour" onClick={() => act.current.closeTour()}>
            <Codicon name="close" />
          </button>
        </div>
      </div>
    );
  }

  const position = positionWords(view);
  const where = stop ? `${stop.path.slice(stop.path.lastIndexOf("/") + 1)}${stop.lines ? `:${stop.lines[0]}` : ""}` : "";

  if (folded) {
    // the mini bar (design 03 §6): one line; the keys and ‹ › still move
    return (
      <div className="stage" style={style} data-stage="folded">
        <div className="stage-header">
          <span className="stage-name">{stop ? `${stop.number}  ${stop.name}` : view.title}</span>
          {step?.title && <span className="stage-dimtext"> · {step.title}</span>}
          <span className="stage-grow" />
          <span className="stage-position">{position}</span>
          <button type="button" className="stage-icon" aria-label="Prev" onClick={() => act.current.prev()}>
            <Codicon name="chevron-left" />
          </button>
          <button type="button" className="stage-icon" aria-label="Next" onClick={() => act.current.next()}>
            <Codicon name="chevron-right" />
          </button>
          <button type="button" className="stage-icon" title="Unfold the stage" aria-label="Unfold the stage" onClick={() => setStageFolded(false)}>
            <Codicon name="chevron-up" />
          </button>
        </div>
      </div>
    );
  }

  return (
    <div className="stage" style={style} data-stage={current!.mode === "step" ? (step?.kind ?? "step") : current!.mode}>
      {grip}
      {current!.mode === "step" && stop && step && look ? (
        <>
          <Header
            stop={stop}
            where={where}
            position={position}
            onGoToCode={() => act.current.goToCode()}
            onFold={() => setStageFolded(true)}
            onCalledFrom={() => stop.from != null && jump(stop.from)}
          />
          <div className="stage-body">
            <div className="stage-left">
              {look.code === "rows" ? (
                <Contents node={node} stops={view.stops} onJump={jump} />
              ) : (
                <CodePane
                  code={code}
                  look={look.code}
                  lit={step.lines && (look.code === "lit" || look.code === "detail") ? step.lines : null}
                  parentLit={current!.parent?.lines ?? null}
                  changed={step.fresh === "changed"}
                  stopId={stop.id}
                  stepId={step.id}
                  gone={node === null}
                  onDrawn={(d) => setDrawn((old) => (old.scrolledTo === d.scrolledTo && old.lit.join() === d.lit.join() && old.thin.join() === d.thin.join() ? old : d))}
                />
              )}
            </div>
            <div className="stage-divider" title="Drag to change the width of the text" onPointerDown={(e) => {
              const start = (e.currentTarget.nextElementSibling as HTMLElement).clientWidth;
              drag(e, (dx) => setStageText(clamp(start - dx, TEXT_MIN, TEXT_MAX)));
            }} />
            <div className="stage-right" style={textWidth === null ? undefined : { width: textWidth }}>
              <TextPane view={view} stop={stop} step={step} parent={current!.parent} look={look} onJump={jump} />
              <Foot view={view} onJump={jump} onMove={(m) => act.current[m]()} />
            </div>
          </div>
        </>
      ) : (
        <>
          <div className="stage-header">
            <span className="stage-name">{view.title}</span>
            <span className="stage-grow" />
            <button type="button" className="stage-icon" title="Fold the stage" aria-label="Fold the stage" onClick={() => setStageFolded(true)}>
              <Codicon name="chevron-down" />
            </button>
          </div>
          {current!.mode === "cover" ? <Cover view={view} onStart={() => act.current.start()} /> : <End view={view} onBack={() => act.current.back()} onAgain={() => act.current.again()} onClose={() => act.current.closeTour()} />}
        </>
      )}
    </div>
  );
}
