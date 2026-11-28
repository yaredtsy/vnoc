import { useCallback, useEffect, useRef, useState } from "react";

import { beatOf, nextState } from "@vnoc/shared/tourWriting.ts";
import { findNode } from "@vnoc/shared/tree.ts";
import type { TourView } from "@vnoc/shared/tours.ts";
import { Codicon } from "@/components/Codicon.tsx";
import { TEXT_FIELD } from "../node-card/codeField.ts";
import { on, post, request } from "../host.ts";
import { useData } from "../state/data.ts";
import { dispatch } from "../state/session.ts";
import { setStageFolded, setStageHeight, setStageText, useView } from "../state/view.ts";
import { type Current, type Shown, ADDING_LINE, MAX_SHARE, NARROW_BELOW, becameWritten, callLines, currentOf, enterDoes, keyMove, lookOf, nextLabel, notWrittenSentence, positionWords, segmentsOf, stageHeightFor } from "./layout.ts";
import { goToStop } from "./goToCode.ts";
import { stepLabel } from "./labels.ts";
import { prefersReducedMotion } from "../lib/motion.ts";
import { go, loadTour, useTour } from "./store.ts";
import { CodePane, type CodeDrawn, LINE_H, keyMoved } from "./stage/CodePane.tsx";
import { Contents } from "./stage/Contents.tsx";
import { Cover, End } from "./stage/Cover.tsx";
import { Foot } from "./stage/Foot.tsx";
import { Header } from "./stage/Header.tsx";
import type { TextDrawn } from "./stage/Rich.tsx";
import { measureStage } from "./stage/measure.ts";
import { TextPane } from "./stage/TextPane.tsx";
import { type Shown as Code, useCode } from "./stage/useCode.ts";
import { useAsWritten } from "./stage/useWritten.ts";
import { useDryPin } from "./stage/useDryPin.ts";
import { coverPin } from "./pin.ts";
import { openPinDialog } from "./pinDialog.ts";
import { type Choice, wantsWritten, writtenLook } from "./written.ts";
import "./stage.css";

/** The least the dock is (the header and five lines of code) and the most (`MAX_SHARE` of the canvas): design 03 §6. */
const MIN_H = 30 + 5 * LINE_H;
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
  // the canvas the dock is in: under 640 px wide the panes stack (design 03 §6), and the dock's height is a share of the canvas's own
  const [room, setRoom] = useState({ width: 1000, height: 800 });
  const watching = useRef<ResizeObserver | null>(null);
  const stageEl = useRef<HTMLDivElement | null>(null);
  const stageRef = useCallback((el: HTMLDivElement | null) => {
    stageEl.current = el;
    watching.current?.disconnect();
    watching.current = null;
    const canvas = el?.parentElement;
    if (!canvas) return;
    const read = () => setRoom((old) => (old.width === canvas.clientWidth && old.height === canvas.clientHeight ? old : { width: canvas.clientWidth, height: canvas.clientHeight }));
    read();
    watching.current = new ResizeObserver(read);
    watching.current.observe(canvas);
  }, []);
  const narrow = room.width < NARROW_BELOW;
  const [drawn, setDrawn] = useState<CodeDrawn>({ scrolledTo: 0, lit: [], thin: [], coloured: 0, step: null, text: null, more: 0 });
  const [textDrawn, setTextDrawn] = useState<TextDrawn>({ lists: 0, tables: 0, fences: 0, diagrams: 0, drawn: 0, failed: 0 });

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
  // a step the code no longer matches: for a pinned tour, the code as it was written (`tourCode`, asked when such a step is shown), else the dashed look
  const [chosen, setChosen] = useState<{ step: string; choice: Choice } | null>(null);
  const choice: Choice = chosen !== null && chosen.step === step?.id ? chosen.choice : "written"; // not kept: a step opens as written
  useEffect(() => setChosen((c) => (c !== null && c.step !== (step?.id ?? null) ? null : c)), [step?.id]);
  const theirs = useAsWritten(tourId, stop?.id ?? null, !!view && !!stop && !!step && !!look && wantsWritten(view, stop, step, look.code));
  const written = view && stop && step && look ? writtenLook({ view, stop, step, codeLook: look.code, answer: theirs, choice }) : ({ look: "fresh" } as const);
  const asWritten = written.look === "written" && theirs ? theirs : null;
  const showWritten = asWritten !== null;
  const asking = written.look === "asking";
  const wantsCode = !!node && look !== null && look.code !== "none" && look.code !== "rows" && !showWritten && !asking;
  const nowCode = useCode(node && wantsCode ? node.path : null, node?.range ?? null, wantsCode);
  // the code the pane shows: the commit's lines for a step shown as written, nothing while they are asked for, else the node as it is now
  const code: Code = asWritten ? { first: asWritten.lines[0], lines: asWritten.code, text: asWritten.code.join("\n") } : asking ? null : nowCode;
  const wantsPane = wantsCode || showWritten || asking;
  // the cover of a finished tour that is not pinned: why not, from the core (a dry pin, nothing is written), and what it offers by the project's settings
  const dry = useDryPin(view, current?.mode === "cover");
  const pinned = view ? coverPin(view, dry) : null;
  // the agent: what it is doing (the tour's state, and no news), and whether `Next` has anywhere to go yet
  const beat = view ? beatOf(view) : "finished";
  const next = view ? nextState(view) : "ready";
  // the text of a step that was grey bars a moment ago fades in for a moment: the step is the same, its kind went from planned to a written one
  const before = useRef<Shown | null>(null);
  const [fadeStep, setFadeStep] = useState<string | null>(null);
  const shown: Shown = { id: step?.id ?? null, kind: step?.kind ?? null };
  const justWritten = becameWritten(before.current, shown); // decided in the render that draws the text, so that it appears already fading
  const fading = justWritten ? shown.id : fadeStep;
  useEffect(() => {
    before.current = shown;
    if (!justWritten) return;
    setFadeStep(shown.id);
    const timer = setTimeout(() => setFadeStep(null), 800);
    return () => {
      clearTimeout(timer);
      setFadeStep(null);
    };
  }, [shown.id, shown.kind]); // eslint-disable-line react-hooks/exhaustive-deps

  // what a click or a key does: a move for the core, Go to Code, closing the tour
  const act = useRef<Record<string, () => void>>({});
  /** The `seq` of the last stageKey/stageClick this page handled (0 before any): every `stageDrawn` says it, so the suite reads only drawings made after its action. */
  const lastSeq = useRef(0);
  /** the calls of the code on screen that lead to a later stop, by file line: what a click on one jumps to */
  const callTo = useRef(new Map<number, { id: string }>());
  /** the step on the stage, for the suite's `choose:now` */
  const stepNow = useRef<string | null>(null);
  stepNow.current = step?.id ?? null;
  /** where the reader is (the cover, a step, the end card): `Enter` is the stage's only on the cover and the end card */
  const modeNow = useRef<Current["mode"] | null>(null);
  modeNow.current = current?.mode ?? null;
  callTo.current = view && stop ? callLines(view, stop.id) : new Map();
  act.current = {
    // a tour with no plan, on its last written step: there is no next step to go to yet, and the core would take the reader to the end card of a tour that is not finished
    next: () => next !== "adding" && go(tourId, "next"),
    start: () => go(tourId, "next"),
    prev: () => go(tourId, "prev"),
    // `↓` and `↑`: the next and the previous stop (design 05 §6)
    nextStop: () => go(tourId, "next-stop"),
    prevStop: () => go(tourId, "prev-stop"),
    back: () => go(tourId, "prev"),
    history: () => go(tourId, "back"),
    skipped: () => view?.place.first_unseen != null && go(tourId, view.place.first_unseen),
    cover: () => go(tourId, "cover"),
    again: () => go(tourId, "cover"),
    end: () => go(tourId, "end"),
    // "called on line N": the step of the caller that holds the line, or the caller when the core named none
    calledFrom: () => {
      const to = stop?.line_step ?? stop?.from;
      if (to != null) go(tourId, to);
    },
    closeTour: () => dispatch({ type: "closeContext", id: contextId }, contextId),
    // `Esc` with the stage folded unfolds it first; the next one closes the tour (design 05 §6)
    escape: () => (useView.getState().stageFolded ? setStageFolded(false) : dispatch({ type: "closeContext", id: contextId }, contextId)),
    // `.` folds and unfolds the stage
    fold: () => setStageFolded(!useView.getState().stageFolded),
    // `Enter`: Start on the cover, Close Tour on the end card; a step has none
    enter: () => {
      const does = current ? enterDoes(current.mode) : null;
      if (does === "start") go(tourId, "next");
      else if (does === "closeTour") dispatch({ type: "closeContext", id: contextId }, contextId);
    },
    // the cover's offers: **Write IDs** is the host's (it saves the stops' files first), **Commit and Pin…** opens the dialog
    writeIds: () => void request("tourWriteIds", { id: tourId }).catch(() => {}),
    commitAndPin: () => void openPinDialog(tourId, view?.title ?? null),
    // the end card's **Share…**: the row's act; the host asks the core and says its words
    share: () => void request("tourShare", { id: tourId }).catch(() => {}),
    goToCode: () => stop && goToStop(structure?.root ?? null, stop, step),
  };
  const jump = useCallback((to: string) => go(tourId, to), [tourId]);

  // the keys; the capture phase and `stopImmediatePropagation`, so that the canvas's own arrows and Space do not also move a card
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const target = e.target instanceof Element ? e.target : null;
      if (target?.closest(TEXT_FIELD)) return;
      const move = keyMove(e.key, { ctrl: e.ctrlKey, meta: e.metaKey, alt: e.altKey });
      if (!move || ((e.key === " " || e.key === "Enter") && !e.ctrlKey && !e.metaKey && target?.closest("button, a, [role=option], [role=menuitem]"))) return; // Space and Enter on a button are the button's
      if (move === "closeTour" && document.querySelector('[role="menu"], [role="dialog"]')) return; // Esc closes a menu first
      // on a step `Enter` is nobody's here: the canvas keeps it
      if (move === "enter" && (modeNow.current === null || enterDoes(modeNow.current) === null)) return;
      if (move === "next" || move === "prev" || move === "nextStop" || move === "prevStop") keyMoved.at = Date.now(); // a move by key scrolls the code wherever the pointer is
      e.preventDefault();
      e.stopImmediatePropagation();
      act.current[move === "closeTour" ? "escape" : move]?.();
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, []);
  // the extension's test suite cannot press keys or click in a webview: it sends the same key, and the same button
  useEffect(
    () =>
      on("viewCommand", ({ name, key, seq }) => {
        const ticket = (name === "stageKey" || name === "stageClick") && seq !== undefined ? seq : null;
        if (ticket !== null) lastSeq.current = ticket; // a drawing from here on says it saw this action
        // the suite cannot drag the window: it narrows the page itself (the same layout rules run on it), and gives it back with no `key`
        if (name === "canvasWidth") document.body.style.width = key ? `${key}px` : "";
        if (name === "stageKey" && key) {
          // `Alt+ArrowLeft`: the modifiers, then the key
          const parts = key.length > 1 && key.includes("+") ? key.split("+") : [key];
          const mods = parts.slice(0, -1);
          document.body.dispatchEvent(new KeyboardEvent("keydown", { key: parts[parts.length - 1], altKey: mods.includes("Alt"), ctrlKey: mods.includes("Ctrl"), metaKey: mods.includes("Meta"), bubbles: true, cancelable: true }));
        }
        if (name === "stageClick" && key) {
          // `jump:s2` and `callLine:6`: a jump to a stop, and a click on the call at that line of the code
          const [what, arg] = key.split(":");
          if (what === "jump") go(tourId, arg);
          else if (what === "choose") setChosen({ step: stepNow.current ?? "", choice: arg === "now" ? "now" : "written" });
          else if (what === "callLine") {
            const target = callTo.current.get(Number(arg));
            if (target) go(tourId, target.id);
          }
          else if (what === "grip") setStageHeight(clamp(useView.getState().stageHeight - Number(arg), MIN_H, Math.max(MIN_H, Math.floor((stageEl.current?.parentElement?.clientHeight ?? 800) * MAX_SHARE)))); // a drag of the grip, `dy` px
          else if (what === "pointer") document.querySelector('[data-part="code"]')?.dispatchEvent(new PointerEvent(arg === "out" ? "pointerleave" : "pointerenter"));
          else act.current[key]?.();
        }
        if (ticket !== null) post({ type: "handled", seq: ticket }); // the handler has run: the page got the action
      }),
    [tourId], // the suite opens another tour in the same page: `go` is for the tour on the stage now
  );

  // what the suite reads of what was drawn: only a drawing that is whole. While the code is being read, and between a new step and the code pane's report of it, the page has not
  // drawn the step yet (a drawing then would carry the lit lines of the step before)
  const codeDrawn = folded || !wantsPane || (code !== null && ("error" in code || (drawn.step === (step?.id ?? null) && drawn.text === code.text)));
  useEffect(() => {
    if (!view || !current || !codeDrawn) return;
    post({
      type: "stageDrawn",
      seq: lastSeq.current,
      reducedMotion: prefersReducedMotion(),
      tour: view.id,
      mode: current.mode,
      stop: stop?.id ?? null,
      step: step?.id ?? null,
      kind: step?.kind ?? null,
      header: stop ? `${stop.planned ? "◌" : "●"} ${stop.number} ${stop.name} ${stop.path}` : current.mode === "cover" ? view.title : "",
      title: current.mode === "step" ? (step?.title ?? null) : view.title,
      text: step?.text ?? null,
      position: positionWords(view),
      next: nextLabel(view, next),
      unseen: view.place.unseen,
      firstUnseen: view.place.first_unseen,
      calls: stop ? [...callLines(view, stop.id).keys()] : [],
      code: code && "lines" in code ? { first: code.first, text: code.text } : null,
      // what the code pane drew: nothing when there is no code pane (the cover, the end, contents)
      lit: wantsPane && !folded ? drawn.lit : [],
      thin: wantsPane && !folded ? drawn.thin : [],
      scrolledTo: wantsPane && !folded ? drawn.scrolledTo : 0,
      coloured: wantsPane && !folded ? drawn.coloured : 0,
      asWritten: asWritten ? { commit: asWritten.commit, lit: drawn.lit } : null,
      switch: written.look === "written" && written.switch ? "written" : written.look === "now" ? "now" : null,
      checkFrom: view.check?.from ?? null,
      markdown: textDrawn,
      beat,
      nextState: next,
      nextLabel: nextLabel(view, next),
      sentence: step?.kind === "planned" ? notWrittenSentence(view, stop!, beat) : null,
      adding: next === "adding" ? ADDING_LINE : null,
      fade: fading !== null && fading === (step?.id ?? null),
      segments: (() => {
        const all = segmentsOf(view);
        return { total: all.length, planned: all.filter((x) => x.planned).length, filled: all.filter((x) => x.filled).length };
      })(),
      pin: current.mode === "cover" ? (pinned?.pin?.text ?? null) : null,
      reason: current.mode === "cover" ? (pinned?.reason ?? null) : null,
      offers: current.mode === "cover" ? (pinned?.offers ?? []).map((o) => `${o.text} | ${o.button}`) : [],
      layout: stageEl.current ? measureStage(stageEl.current, narrow, folded) : { narrow, folded, stacked: false, width: 0, canvasHeight: 0, height: 0, codeHeight: 0, codeRows: 0, wrapped: 0, codeCut: false, outside: [], chipOverCode: null, coverParts: [] },
    });
  }, [narrow, folded, room.width, view, dry, beat, next, fading, code, codeDrawn, drawn.scrolledTo, drawn.coloured, drawn.more, drawn.lit.join(","), drawn.thin.join(","), textDrawn.lists, textDrawn.tables, textDrawn.fences, textDrawn.diagrams, textDrawn.drawn, textDrawn.failed]); // eslint-disable-line react-hooks/exhaustive-deps

  const style = { height: folded ? 30 : stageHeightFor(height, narrow, Math.floor(room.height * MAX_SHARE), MIN_H) };
  const cls = `stage${narrow ? " stage-narrow" : ""}`;
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
      <div ref={stageRef} className={cls} style={{ height: 30 }} data-stage="loading">
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
      <div ref={stageRef} className={cls} style={style} data-stage="folded">
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

  // what a screen reader is told of the stage: the step (design 05 §8), or the cover and the end card by their tour
  const label = current!.mode === "step" && stop && step ? stepLabel(view, stop, step, current!.parent) : `${current!.mode === "cover" ? "Cover" : "End"} of the tour of ${view.title}`;
  return (
    <div ref={stageRef} className={cls} style={style} role="region" aria-label={label} data-stage={current!.mode === "step" ? (step?.kind ?? "step") : current!.mode}>
      {grip}
      {current!.mode === "step" && stop && step && look ? (
        <>
          <Header
            stop={stop}
            where={where}
            position={position}
            onGoToCode={() => act.current.goToCode()}
            canGoToCode={stop.check !== "gone"}
            onFold={() => setStageFolded(true)}
            onCalledFrom={() => act.current.calledFrom()}
          />
          <div className="stage-body">
            <div className="stage-left">
              {look.code === "rows" ? (
                <Contents node={node} stops={view.stops} onJump={jump} />
              ) : (
                <CodePane
                  code={code}
                  look={look.code}
                  lit={look.code === "lit" || look.code === "detail" ? (asWritten ? (asWritten.steps[step.id] ?? null) : step.lines) : null}
                  parentLit={asWritten ? (current!.parent ? (asWritten.steps[current!.parent.id] ?? null) : null) : (current!.parent?.lines ?? null)}
                  changed={step.fresh === "changed" && !showWritten}
                  written={asWritten ? { commit: asWritten.commit } : null}
                  path={asWritten ? asWritten.path : (node?.path ?? null)}
                  stopId={stop.id}
                  stepId={step.id}
                  gone={node === null && !showWritten && !asking}
                  calls={callLines(view, stop.id)}
                  onCall={jump}
                  onDrawn={(d) => setDrawn((old) => (old.scrolledTo === d.scrolledTo && old.lit.join() === d.lit.join() && old.thin.join() === d.thin.join() && old.coloured === d.coloured && old.step === d.step && old.text === d.text ? old : d))}
                />
              )}
            </div>
            <div className="stage-divider" title="Drag to change the width of the text" onPointerDown={(e) => {
              const start = (e.currentTarget.nextElementSibling as HTMLElement).clientWidth;
              drag(e, (dx) => setStageText(clamp(start - dx, TEXT_MIN, TEXT_MAX)));
            }} />
            <div className="stage-right" style={textWidth === null ? undefined : { width: textWidth }}>
              <TextPane view={view} stop={stop} step={step} parent={current!.parent} look={look} beat={beat} next={next} fade={fading !== null && fading === step.id} written={written} onChoose={(c) => setChosen({ step: step.id, choice: c })} onJump={jump} onTextDrawn={(d) => setTextDrawn((old) => (JSON.stringify(old) === JSON.stringify(d) ? old : d))} />
              <Foot view={view} next={next} onJump={jump} onMove={(m) => act.current[m]()} />
            </div>
          </div>
        </>
      ) : (
        <>
          {/* the cover's header says the tour's title (its body starts with the question); the end card's body says it, and its header only folds */}
          <div className="stage-header">
            {current!.mode === "cover" ? <span className="stage-name" title={view.title}>{view.title}</span> : null}
            <span className="stage-grow" />
            <button type="button" className="stage-icon" title="Fold the stage" aria-label="Fold the stage" onClick={() => setStageFolded(true)}>
              <Codicon name="chevron-down" />
            </button>
          </div>
          {current!.mode === "cover" ? <Cover view={view} beat={beat} pin={pinned!} onStart={() => act.current.start()} onOffer={(kind) => act.current[kind === "ids" ? "writeIds" : "commitAndPin"]()} /> : <End view={view} onBack={() => act.current.back()} onSkipped={() => act.current.skipped()} onAgain={() => act.current.again()} onClose={() => act.current.closeTour()} onShare={() => act.current.share()} />}
        </>
      )}
    </div>
  );
}
