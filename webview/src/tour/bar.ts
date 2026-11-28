// What the bar says in a tour context (docs/26-tours/design/04-route-and-map.md §5). Pure, from the `TourView` and its place alone: the crumbs are the stops from the first
// one down to the current one by the stop each was reached from (`from`), and "moved by the agent" is a place that moved and says a command moved it.

import type { Place, TourView } from "@vnoc/shared/tours.ts";

/** How long the bar says `moved by the agent`, in ms (docs/26-tours/05-on-screen.md §3). */
export const MOVED_MS = 3000;

export interface TourCrumb {
  /** the stop's id: a click jumps to it, the pointer over it points at its card */
  id: string;
  label: string;
}

/**
 * The stops from the first one down to the current one, each reached from the one before (`from`): `checkout_order › reserve_stock`. The project is not a crumb: a
 * tour has no focus to clear. At the cover and the end there is no current stop, and none. A `from` that points at nothing, or loops, ends the chain.
 */
export function crumbsOf(view: TourView): TourCrumb[] {
  const chain: TourCrumb[] = [];
  let at = view.place.stop;
  for (let n = 0; at !== null && n <= view.stops.length; n++) {
    const stop = view.stops.find((s) => s.id === at);
    if (!stop) break;
    chain.unshift({ id: stop.id, label: stop.name });
    at = stop.from ?? null;
  }
  return chain;
}

/**
 * Whether a place that arrived moved by another process: it moved since the last one the bar saw (another step, or another `moved` time: the core's time is whole
 * seconds, so a move right after the reader's own can have the same one) and says `command` moved it. The first place the bar sees (a tour just opened) is nothing to
 * say, and neither is the same place again.
 */
export function movedByAgent(before: Place | null, now: Place): boolean {
  return before !== null && now.by === "command" && now.moved !== "" && (now.moved !== before.moved || now.at !== before.at);
}

/** The tooltip of the bar's `←`: back to where you were before the last jump. Disabled with nothing to go back to. */
export const canGoBack = (view: TourView): boolean => view.place.back.length > 0;

/** The agent's words on a narrow bar (design 04 §5): `writing 6/10` is `6/10`, `writing · 6 so far` is `6 so far`; the others (`stopping`, `stopped`, `no news`) are as they are. */
export const narrowAgent = (words: string): string => words.replace(/^writing (?:· )?/, "");
