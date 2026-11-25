// What a tour says while the agent is writing it (docs/26-tours/05-on-screen.md §3, design/05-states.md §1, §3): the words of the bar, the route's foot, the status bar item,
// and when `Next` is dim. Pure, from the `TourView` the core sent: its `quiet` (no news: the core's, from the file's time) and its `of` (the size of the route, `null` while it
// is not known). Nothing here walks a tour, sums a size or works out which step comes next: the route's order is the core's (`stops`), a stop's number is its own, and
// `written`, `planned`, `of` and `quiet` are the core's.

import type { TourId, TourRow, TourView } from "./tours.ts";

/** What the reader is told the agent is doing: the tour's state, and `quiet` (no news) when it is being written and its file has not grown for 5 minutes. */
export type Beat = "finished" | "writing" | "stopping" | "stopped" | "quiet";

export const beatOf = (view: Pick<TourView, "state" | "quiet">): Beat => (view.state === "finished" ? "finished" : view.state === "stopped" ? "stopped" : view.quiet ? "quiet" : view.state);

/**
 * Most of the tour is about older code (design 05 §4): the stops a check found changed or gone are more than half of the stops written. `null` when no check was made, nothing was
 * written, or it is not most. The numbers are the core's (`check.changed`, `check.gone`, `written`); this is the one comparison, and the cover and the route's foot both read it.
 */
export function mostChanged(view: Pick<TourView, "check" | "written">): { stale: number; written: number } | null {
  if (!view.check || view.written === 0) return null;
  const stale = view.check.changed + view.check.gone;
  return stale * 2 > view.written ? { stale, written: view.written } : null;
}

/** The route's foot of a finished tour that is mostly about older code (design 05 §4). */
export const staleWords = (m: { stale: number; written: number }): string => `${m.stale} of ${m.written} stops changed since this was written`;

/** Whether the tour is still being written: `writing` or `stopping` (the agent has not answered the halt yet). */
export const isWriting = (view: Pick<TourView, "state">): boolean => view.state === "writing" || view.state === "stopping";

/** The number on screen of the stop the agent is on (`view.writing`), or null. */
function writingNumber(view: TourView): number | null {
  return view.writing === null ? null : (view.stops.find((s) => s.id === view.writing)?.number ?? null);
}

/** `6 stops so far`: what a tour with no plan, being written, has to say about its size. */
export const soFar = (view: Pick<TourView, "written">): string => `${view.written} ${view.written === 1 ? "stop" : "stops"} so far`;

/**
 * The agent's words on the bar (design 04 §5 #4): `writing 6/10`, `writing · 6 so far` for a tour with no plan, `stopping`, `stopped`, `no news`. Null when the tour is finished.
 * The ■ is drawn only on `writing` (`stoppable`): after it is pressed the bar says `stopping`, and a tour with no news has nobody to hear it.
 */
export function agentWords(view: TourView): string | null {
  switch (beatOf(view)) {
    case "finished":
      return null;
    case "quiet":
      return "no news";
    case "stopping":
      return "stopping";
    case "stopped":
      return "stopped";
    case "writing":
      return view.of !== null ? `writing ${writingNumber(view) ?? view.written}/${view.of}` : `writing · ${view.written} so far`;
  }
}

/** Whether the ■ is on the bar and the route's foot: only while the agent writes and the halt has not been written. */
export const stoppable = (view: Pick<TourView, "state" | "quiet">): boolean => beatOf(view) === "writing";

/**
 * The route's foot line (design 05 §1): `writing 6 of 10`, `writing · 6 stops so far`, `stopping`, `stopped: 6 of 10 written`, `no news from the agent · 6 of 10 written`. For a
 * finished tour, null, or `7 of 10 stops changed since this was written` when most of it is about older code (design 05 §4).
 */
export function footWords(view: TourView): string | null {
  const written = view.of !== null ? `${view.written} of ${view.of} written` : soFar(view);
  switch (beatOf(view)) {
    case "finished": {
      const most = mostChanged(view);
      return most ? staleWords(most) : null;
    }
    case "quiet":
      return `no news from the agent · ${written}`;
    case "stopping":
      return "stopping: the agent finishes the stop it is on";
    case "stopped":
      return view.of !== null && view.planned > 0 ? `stopped: ${written}` : `stopped: ${view.written} ${view.written === 1 ? "stop" : "stops"} written`;
    case "writing":
      return view.of !== null ? `writing ${writingNumber(view) ?? view.written} of ${view.of}` : `writing · ${soFar(view)}`;
  }
}

/**
 * The status bar item (design 04 §6): `▶ Tour 3/10` from the place's numbers (`▶ Tour` at the cover and at the end, where the place has none), and the agent's words after it
 * while it writes: `▶ Tour 3/10 · writing 6/10`. Null when no tour is open.
 */
export function statusBarText(view: TourView | null): string | null {
  if (!view) return null;
  const p = view.place.position;
  const where = p ? ` ${p.stop}/${p.of}` : "";
  const agent = agentWords(view);
  return `▶ Tour${where}${agent ? ` · ${agent}` : ""}`;
}

/**
 * What `Next` is, from the view: `ready`; `planned` (the next step is a stop the agent has not written: dim with `◌`, and pressing it still goes there, where the stage says
 * the stop is being written); `adding` (the last written step of a tour that is being written and has no plan: dim with `◌`, and nothing to go to yet). The next step after the
 * last step of a stop is the first of the next stop on the route (`stops` is the route's order, `place.position.stop` is this stop's place on it); inside a stop there is nothing
 * to wait for, and the cover's `Start` is always ready.
 */
export type NextState = "ready" | "planned" | "adding";

export function nextState(view: TourView): NextState {
  const p = view.place.position;
  if (view.place.at === "cover" || view.place.at === "end" || p === null || p.step < p.steps) return "ready";
  const next = view.stops[p.stop]; // the stop after this one: the stop number is from 1, the array from 0
  if (next) return next.planned ? "planned" : "ready";
  return beatOf(view) === "writing" && view.of === null ? "adding" : "ready";
}

/**
 * The tours a list shows that the window had not heard of and that are being written: the ones to announce once (design 04 §6, W11). `known` is the ids of the last list (null: none was
 * held, so nothing is new: a window that starts with tours in the folder announces none of them), `announced` the ids that were told already.
 */
export function toAnnounce(rows: readonly TourRow[], known: ReadonlySet<TourId> | null, announced: ReadonlySet<TourId>): TourRow[] {
  if (known === null) return [];
  return rows.filter((r) => r.readable && r.state === "writing" && !known.has(r.id) && !announced.has(r.id));
}
