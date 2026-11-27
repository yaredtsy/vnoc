// What the stage draws for the place the host holds (docs/26-tours/design/03-stage.md §3, 05-on-screen.md §2). Pure, and it only reads: the view and its place are
// the core's, and nothing here walks the tour, counts stops or works out what comes next. The kind of a step is the core's word (`StepView.kind`); this table is
// what each kind looks like.

import type { Beat, NextState } from "@vnoc/shared/tourWriting.ts";
import type { StepKind, StepView, TourStop, TourView } from "@vnoc/shared/tours.ts";

/** Where the reader is, found in the view: the stop and the step `place` names (a lookup by id in what the core sent). The cover and the end have neither. */
export interface Current {
  mode: "cover" | "end" | "step";
  stop: TourStop | null;
  step: StepView | null;
  /** the block a detail is a detail of: its line goes above the detail's title */
  parent: StepView | null;
}

export function currentOf(view: TourView): Current {
  const { at, stop: id } = view.place;
  if (at === "cover" || at === "end") return { mode: at, stop: null, step: null, parent: null };
  const stop = view.stops.find((s) => s.id === id) ?? null;
  const step = stop?.reading.find((r) => r.id === at) ?? null;
  const parent = step?.parent ? (stop?.reading.find((r) => r.id === step.parent) ?? null) : null;
  return { mode: "step", stop, step, parent };
}

/** How the code pane treats the lines: `none` is no pane at all (the cover, the end card: one column). */
export type CodeLook =
  /** no code pane */
  | "none"
  /** the node whole; nothing lit, nothing dim, a thin mark on its first line (an intro) */
  | "intro"
  /** the block's lines lit, the rest dim */
  | "lit"
  /** a detail: its lines lit, the rest of its parent's lines keep the thin mark and are not dimmed, everything else dim */
  | "detail"
  /** the whole node lit, nothing dim (a short stop) */
  | "all"
  /** the whole node dim, nothing lit: it was read at the stop it is explained at */
  | "dim"
  /** the whole node as it is: you can read it (a stop that is not written) */
  | "plain"
  /** the node's children as rows: a folder, a file with no blocks */
  | "rows";

/** What a step looks like: its code, whether the text pane has a title, whether its paragraph is the lead size, and the extras under the text. */
export interface Look {
  code: CodeLook;
  /** the block's title is drawn (an intro and a short stop have none) */
  title: boolean;
  /** the paragraph is in the lead size (14 px): an intro is the lead */
  lead: boolean;
  /** the parent block's title in one dim line above a detail's title */
  parentLine: boolean;
  /** `⇢ Explained at stop N  [Go there]` under a sentence */
  explained: boolean;
  /** the grey bars and the sentence of a stop that is not written */
  bars: boolean;
}

const LOOKS: Record<StepKind, Look> = {
  intro: { code: "intro", title: false, lead: true, parentLine: false, explained: false, bars: false },
  block: { code: "lit", title: true, lead: false, parentLine: false, explained: false, bars: false },
  detail: { code: "detail", title: true, lead: false, parentLine: true, explained: false, bars: false },
  short: { code: "all", title: false, lead: true, parentLine: false, explained: false, bars: false },
  contents: { code: "rows", title: false, lead: true, parentLine: false, explained: false, bars: false },
  same_as: { code: "dim", title: false, lead: false, parentLine: false, explained: true, bars: false },
  planned: { code: "plain", title: false, lead: false, parentLine: false, explained: false, bars: true },
};

const NONE: Look = { code: "none", title: false, lead: false, parentLine: false, explained: false, bars: false };

/** The look of the place: the cover and the end are one column; a step is by its kind. */
export const lookOf = (current: Current): Look => (current.mode === "step" && current.step ? LOOKS[current.step.kind] : NONE);

/** How one line of the code pane is drawn: `lit` (the bar and the background), `dim` (40%), `thin` (the mark `▏`, not dimmed), `plain`. */
export type LineLook = "lit" | "dim" | "thin" | "plain";

/**
 * The look of file line `n`, for a node whose first line is `first`. `lit` is the step's lines now, `parentLit` the lines of a detail's parent block; both are
 * file lines, inclusive. An intro marks only the node's first line.
 */
export function lineLook(code: CodeLook, n: number, first: number, lit: readonly [number, number] | null, parentLit: readonly [number, number] | null): LineLook {
  const within = (r: readonly [number, number] | null) => r !== null && n >= r[0] && n <= r[1];
  switch (code) {
    case "intro":
      return n === first ? "thin" : "plain";
    case "lit":
      return lit === null ? "plain" : within(lit) ? "lit" : "dim";
    case "detail":
      return within(lit) ? "lit" : within(parentLit) ? "thin" : "dim";
    case "all":
      return "lit";
    case "dim":
      return "dim";
    default:
      return "plain";
  }
}

/** One segment of the foot: a stop, filled when it was read to its last step (`place.seen`) or is where the reader is. */
export interface Segment {
  stop: string;
  name: string;
  filled: boolean;
  /** not written: dotted */
  planned: boolean;
  current: boolean;
  /** the one dotted segment after the stops of a tour with no plan that is being written: the stop the agent has not added yet. It leads nowhere */
  adding?: boolean;
}

/** The foot's segments: one per stop, and (05 §3) one more, dotted, after the stops of a tour with no plan that is being written. */
export function segmentsOf(view: TourView): Segment[] {
  const segments: Segment[] = view.stops.map((s) => {
    const current = s.id === view.place.stop;
    return { stop: s.id, name: s.name, filled: current || view.place.seen.includes(s.id), planned: s.planned, current };
  });
  if (view.state === "writing" && view.of === null) segments.push({ stop: "", name: "the next stop: the agent is adding it", filled: false, planned: true, current: false, adding: true });
  return segments;
}

/** The words of the foot's primary button: `Start ›` on the cover, `Finish ›` on the last step, else `Next ›` (`Next ◌` when it is dim: the stage draws the ◌). Never on the end card. */
export function nextLabel(view: TourView, next: NextState = "ready"): "Start" | "Next" | "Finish" {
  const at = view.place.at;
  if (at === "cover") return "Start";
  if (next !== "ready") return "Next";
  const p = view.place.position;
  return p !== null && p.stop === p.of && p.step === p.steps ? "Finish" : "Next";
}

/**
 * `stop 3 of 10 · 2 of 5`, as the header says it: the second part only when the stop has more than one step. Nothing at the cover and the end. A tour with no plan that is being
 * written has no "of": `stop 3 · 6 so far` (05 §3).
 */
export function positionWords(view: TourView): string {
  const p = view.place.position;
  if (p === null) return "";
  const stop = view.of !== null ? `stop ${p.stop} of ${p.of}` : `stop ${p.stop} · ${view.written} so far`;
  return p.steps > 1 ? `${stop} · ${p.step} of ${p.steps}` : stop;
}

/**
 * What a step that is not written says in the text pane's place (design 03 §3). `stopped`: the tour was stopped before it; `quiet`: no news; the stop the agent is on says it is being
 * written; another says where the agent is. After ■ and before the agent has answered (`stopping`), it says the agent will stop.
 */
export function notWrittenSentence(view: TourView, stop: Pick<TourStop, "id">, beat: Beat): string {
  if (beat === "stopped") return "Not written: the tour was stopped here. Ask the agent to continue it.";
  if (beat === "quiet") return "Not written. The agent may have stopped: look at its terminal, or ask it to continue the tour.";
  if (beat === "stopping") return "Not written yet: the agent stops after the stop it is on.";
  if (view.writing === stop.id) return "The agent is writing this stop. You can read the code meanwhile.";
  const on = view.writing === null ? undefined : view.stops.find((s) => s.id === view.writing);
  return `Not written yet${on ? `: the agent is on stop ${on.number}.` : "."}`;
}

/** The line under the last written step of a tour with no plan that is being written (05 §3): the text pane keeps the step's words; this is added, and `Next` is dim. */
export const ADDING_LINE = "The agent is adding the next stop.";

/** What the text pane showed for a step: its id and its kind. */
export interface Shown {
  id: string | null;
  kind: string | null;
}

/** The reader is on a stop that was `planned` a moment ago and is written now: the same step, its text appears where the grey bars were, and fades in. */
export const becameWritten = (before: Shown | null, now: Shown): boolean => before !== null && before.id !== null && before.id === now.id && before.kind === "planned" && now.kind !== null && now.kind !== "planned";

/** What a key does in the stage (design 05 §6): a move for the core, Go to Code, back to where you were, closing or folding, or the key that does the cover's or the end card's one thing. */
export type KeyMove = "next" | "prev" | "nextStop" | "prevStop" | "cover" | "end" | "goToCode" | "history" | "closeTour" | "enter" | "fold";

/**
 * What a key does in the stage (design 05 §6): `→` and Space are Next, `←` Prev, `↓` and `↑` the next and the previous **stop**, `Alt+←` back to where you were (the core's
 * history, not Prev), `Home` the cover, `End` the end card, `Enter` the cover's Start or the end card's Close Tour (`enter`: the stage knows which), `Ctrl/Cmd+Enter` Go to
 * Code, `Esc` closes the tour (unfolds the stage first when it is folded), `.` folds and unfolds the stage.
 */
export function keyMove(key: string, mod: { ctrl: boolean; meta: boolean; alt: boolean }): KeyMove | null {
  if ((mod.ctrl || mod.meta) && key === "Enter") return "goToCode";
  if (mod.alt && !mod.ctrl && !mod.meta && key === "ArrowLeft") return "history";
  if (mod.ctrl || mod.meta || mod.alt) return null;
  switch (key) {
    case "ArrowRight":
    case " ":
      return "next";
    case "ArrowLeft":
      return "prev";
    case "ArrowDown":
      return "nextStop";
    case "ArrowUp":
      return "prevStop";
    case "Home":
      return "cover";
    case "End":
      return "end";
    case "Enter":
      return "enter";
    case "Escape":
      return "closeTour";
    case ".":
      return "fold";
    default:
      return null;
  }
}

/**
 * What the `Enter` key does where the reader is: **Start** on the cover, **Close Tour** on the end card, and nothing on a step (a step has no one thing to do: `→` goes on). The
 * stage does the same for `Esc` with the stage folded: it unfolds first, and the next `Esc` closes.
 */
export const enterDoes = (mode: Current["mode"]): "start" | "closeTour" | null => (mode === "cover" ? "start" : mode === "end" ? "closeTour" : null);

/**
 * The calls of a stop's code that are stops of this tour, by file line: the stops reached from this one by a call (`from` is this stop, `line` the line of the call). The
 * view says which stop a line leads to; nothing is matched by name. When two stops were reached from one line, the first on the route is the one.
 */
export function callLines(view: TourView, stopId: string): Map<number, TourStop> {
  const calls = new Map<number, TourStop>();
  for (const s of view.stops) if (s.from === stopId && s.line != null && !calls.has(s.line)) calls.set(s.line, s);
  return calls;
}
