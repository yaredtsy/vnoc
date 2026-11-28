// What a screen reader is given for a tour (docs/26-tours/design/05-states.md §8): a step as *"stop 3 of 10, function reserve_stock, step 2 of 3. Refuse what is not there."*,
// a route row as *"stop 6, function retry_charge, not written, the agent is writing it, level 2"*, and a block nested in another as *"detail of …"*. Pure: the numbers are the
// core's (`place.position`, the stop's `number` and `depth`), and the words are the design's.

import type { StepView, TourStop, TourView } from "@vnoc/shared/tours.ts";
import type { RouteRow } from "../sidebar/tours/route.ts";

/** `function`, `class`, `file`, `folder`: what a stop is, as it is read aloud. A stop with no kind (not written yet) is just a stop. */
const kindWord = (kind: string | null): string => (kind === "function" || kind === "class" || kind === "file" || kind === "folder" ? kind : "");

/** `stop 3 of 10, function reserve_stock, step 2 of 3`: where the reader is, in the core's numbers (`3 of 10` is `3, 6 so far` for a tour with no plan that is being written). */
export function placeLabel(view: TourView, stop: Pick<TourStop, "kind" | "name">): string {
  const p = view.place.position;
  const parts: string[] = [];
  if (p) parts.push(view.of !== null ? `stop ${p.stop} of ${p.of}` : `stop ${p.stop}, ${view.written} so far`);
  parts.push([kindWord(stop.kind), stop.name].filter(Boolean).join(" "));
  if (p && p.steps > 1) parts.push(`step ${p.step} of ${p.steps}`);
  return parts.join(", ");
}

/**
 * What the stage's step is called: the place, then (for a block nested in another) *"detail of <the parent block's title>"*, then the step's own title. The text that follows is the
 * step's own and is read as it is.
 */
export function stepLabel(view: TourView, stop: TourStop, step: StepView, parent: StepView | null): string {
  const lead = placeLabel(view, stop);
  const detail = parent?.title ? `detail of ${parent.title}` : null;
  const title = step.title ?? null;
  const tail = [detail, title].filter((x): x is string => !!x);
  return tail.length > 0 ? `${lead}. ${tail.join(". ")}` : lead;
}

/** What a row's one word is, said in full: the route says `writing…`, `gone`, `⇢ 2`; the screen reader says what that means. */
export function describedAs(description: string | null): string | null {
  if (description === null) return null;
  if (description === "writing…") return "the agent is writing it";
  if (description === "no news") return "no news from the agent";
  if (description === "gone") return "its code is gone";
  if (description === "changed") return "changed since it was written";
  const explained = /^⇢ (\d+)$/.exec(description);
  return explained ? `explained at stop ${explained[1]}` : description;
}

/** A route row: a stop is *"stop 6, function retry_charge, not written, the agent is writing it, level 2"*; a step is its title, and a detail says which block it is a detail of. */
export function routeLabel(row: RouteRow): string {
  if (row.type === "step") {
    const what = row.parent ? `detail of ${row.parent}: ${row.text}` : row.text;
    return row.current ? `${what}, the step you are on` : what;
  }
  const parts = [`stop ${row.number}`, [kindWord(row.kind), row.name].filter(Boolean).join(" ")];
  if (row.mark === "planned") parts.push("not written");
  else if (row.mark === "seen") parts.push("seen");
  else if (row.mark === "here") parts.push("you are here");
  const said = describedAs(row.description);
  if (said) parts.push(said);
  parts.push(`level ${row.depth + 1}`);
  return parts.join(", ");
}
