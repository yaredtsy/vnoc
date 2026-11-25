// The route of the open tour, as rows (docs/26-tours/design/04-route-and-map.md §1). Pure, and it reads only the view the core sent: the stops in its order, nested by
// `depth`, the mark from `place` (here: `place.stop`; seen: `place.seen`; not written: the stop's own `planned`), and under the stop you are in its steps, indented by
// their `depth`. Nothing is counted, sorted or worked out.

import type { TourStop, TourView } from "@vnoc/shared/tours.ts";

/** `✓` seen · `●` here · `○` ahead · `◌` not written: four shapes, so they read without colour (§1.1). */
export type Mark = "seen" | "here" | "ahead" | "planned";

export type RouteRow =
  | {
      type: "stop";
      id: string;
      /** the number on screen, from the core */
      number: number;
      /** one level of indent per `depth`: a stop sits under the stop it was reached from */
      depth: number;
      kind: string | null;
      name: string;
      /** the node's key in the structure, for the row's menu */
      key: string | null;
      mark: Mark;
      /** the stop the reader is on */
      current: boolean;
      /** one word, the first that applies: `writing…` · `changed` · `⇢ 2` */
      description: string | null;
    }
  | {
      type: "step";
      id: string;
      /** the indent: the stop's, then one more for a block and one more for a detail */
      depth: number;
      /** the intro's row reads `what it is for`; a block's is its title */
      text: string;
      current: boolean;
    };

/** The mark of a stop. A stop that is not written is `◌` whatever else is true of it. */
export function markOf(view: TourView, stopId: string, planned: boolean): Mark {
  if (planned) return "planned";
  if (view.place.stop === stopId) return "here";
  return view.place.seen.includes(stopId) ? "seen" : "ahead";
}

/** The one word a stop says, the first that applies: `writing…` · `changed` · `⇢ 2` (explained at stop 2). The route's row and the map's card say the same. */
export function stopDescription(view: TourView, stop: TourStop): string | null {
  const explained = stop.same_as == null ? null : view.stops.find((s) => s.id === stop.same_as);
  return view.writing === stop.id ? "writing…" : stop.check === "changed" ? "changed" : explained ? `⇢ ${explained.number}` : null;
}

export function routeRows(view: TourView): RouteRow[] {
  const rows: RouteRow[] = [];
  for (const stop of view.stops) {
    const description = stopDescription(view, stop);
    const current = view.place.stop === stop.id;
    rows.push({ type: "stop", id: stop.id, number: stop.number, depth: stop.depth, kind: stop.kind, name: stop.name, key: stop.key, mark: markOf(view, stop.id, stop.planned), current, description });
    // the steps are under the stop you are in, and only when it has more than one: leaving it folds them away (§1.2)
    if (current && stop.reading.length > 1) {
      for (const step of stop.reading) rows.push({ type: "step", id: step.id, depth: stop.depth + step.depth, text: step.depth === 0 ? "what it is for" : (step.title ?? ""), current: step.id === view.place.at });
    }
  }
  return rows;
}

/** The header's place: `3 of 10`, the core's numbers; nothing at the cover and the end. */
export const headerPlace = (view: TourView): string => (view.place.position ? `${view.place.position.stop} of ${view.place.position.of}` : "");

/**
 * A row's React key: a stop and its own intro step share an id (`s1`), so the kind is part of it. Two rows with one key made the route draw a row of the tour before it.
 */
export const rowKey = (row: RouteRow): string => `${row.type}:${row.id}`;
