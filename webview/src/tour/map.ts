// The tour on the map (docs/26-tours/design/04-route-and-map.md §2, §3): one card per stop of the route, from the first second, and the edges between them. Pure, and it
// reads only the `TourView`: a stop's number, depth, `from` and `line`, `same_as`, kind, state, `key`, and `place.stop` for the ring. Where a card goes is the layout's
// rule (a row each, the depth to the right); which edges are heavy is the way from the first stop to the current one by the stops' own `from`.

import type { TourStop, TourView } from "@vnoc/shared/tours.ts";
import { CARD_H, CARD_W, COL_W, ROW_H } from "../canvas/layout.ts";
import { type Mark, markOf, stopDescription } from "../sidebar/tours/route.ts";

// a `type`, not an interface: React Flow wants node data to be assignable to Record<string, unknown>
export type TourCardData = {
  /** the stop's id: the card's id, and what a click jumps to */
  id: string;
  number: number;
  depth: number;
  x: number;
  y: number;
  kind: string | null;
  name: string;
  path: string;
  /** the stop's first line in its file, when it has lines (a folder has none) */
  line: number | null;
  /** `✓` seen · `●` here · `○` ahead · `◌` not written: the badge */
  mark: Mark;
  /** the ring: the stop the reader is on (`place.stop`) */
  here: boolean;
  planned: boolean;
  /** a stop reached by a call is a call card: the line of its caller where it is called (`↳ line 16`); null for a stop reached by containment and for the first */
  callLine: number | null;
  /** the stop it was reached from */
  from: string | null;
  /** the one word of the footer's right: `writing…` · `changed` · `⇢ 2` */
  description: string | null;
  /** the number of the stop it is explained at (`⇢ explained at 2`) */
  explainedAt: number | null;
  /** how many steps it has, when it has more than one and is written (`3 steps`) */
  steps: number | null;
  key: string | null;
};

export interface TourEdgeData {
  id: string;
  source: string;
  target: string;
  /** `call`: the stop was reached by a call (dashed); `node`: by containment (solid) */
  kind: "call" | "node";
  /** on the way from the first stop to the current one: drawn heavy */
  onPath: boolean;
}

/** The stops from the current one up to the first, by `from`: the ids whose edge to their parent is on the way. A chain that loops is cut (a hand-edited file). */
function wayTo(view: TourView, current: string | null): Set<string> {
  const way = new Set<string>();
  let at = current;
  for (let n = 0; at !== null && n <= view.stops.length; n++) {
    const stop: TourStop | undefined = view.stops.find((s) => s.id === at);
    if (!stop || stop.from == null || !view.stops.some((s) => s.id === stop.from)) break;
    way.add(stop.id);
    at = stop.from;
  }
  return way;
}

export function mapOf(view: TourView): { cards: TourCardData[]; edges: TourEdgeData[] } {
  const way = wayTo(view, view.place.stop);
  const cards = view.stops.map((stop, i): TourCardData => {
    const explained = stop.same_as == null ? null : view.stops.find((s) => s.id === stop.same_as);
    return {
      id: stop.id,
      number: stop.number,
      depth: stop.depth,
      x: stop.depth * COL_W,
      y: i * ROW_H,
      kind: stop.kind,
      name: stop.name,
      path: stop.path,
      line: stop.lines ? stop.lines[0] : null,
      mark: markOf(view, stop.id, stop.planned),
      here: view.place.stop === stop.id,
      planned: stop.planned,
      callLine: stop.from != null && stop.line != null ? stop.line : null,
      from: stop.from ?? null,
      description: stopDescription(view, stop),
      explainedAt: explained?.number ?? null,
      steps: !stop.planned && stop.steps > 1 ? stop.steps : null,
      key: stop.key,
    };
  });
  const edges: TourEdgeData[] = view.stops.flatMap((stop) =>
    stop.from != null && view.stops.some((s) => s.id === stop.from)
      ? [{ id: `${stop.from}->${stop.id}`, source: stop.from, target: stop.id, kind: stop.line != null ? ("call" as const) : ("node" as const), onPath: way.has(stop.id) }]
      : [],
  );
  return { cards, edges };
}

/** A card's box in the map's coordinates, for the camera. */
export const boxOf = (card: Pick<TourCardData, "x" | "y">) => ({ x: card.x, y: card.y, width: CARD_W, height: CARD_H });
