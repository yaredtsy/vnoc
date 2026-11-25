// A step whose code is not what the text was written about (docs/26-tours/05-on-screen.md §3, design 05 §4). Pure: it chooses how the stage shows such a step from what the core
// said (`TourView`: the step's `fresh`, the stop's `check`, `pinned`, `pin_in_repository`) and from `vnoc/tourCode`'s answer. Nothing here compares code or counts lines.

import { isWriting, mostChanged } from "@vnoc/shared/tourWriting.ts";
import type { AsWritten, StepView, TourStop, TourView } from "@vnoc/shared/tours.ts";
import type { CodeLook } from "./layout.ts";

/** What the reader asked for on a step that changed: the code as it was written, or as it is now. Not kept: a step opens as written. */
export type Choice = "written" | "now";

/** A step the code no longer matches: its own lines changed, or the stop is not in the code any more (every step of a gone stop is). */
export const isMarked = (stop: Pick<TourStop, "check">, step: Pick<StepView, "fresh">): boolean => stop.check === "gone" || step.fresh === "changed";

/** The looks that have a code pane the mark can change: not the cover, a folder's or a file's contents (rows), or a stop that is not written. */
const marks = (look: CodeLook): boolean => look !== "none" && look !== "rows" && look !== "plain";

/** Whether `vnoc/tourCode` is worth asking for this step: it is marked, and the tour is pinned at a commit this repository has. */
export const wantsWritten = (view: Pick<TourView, "pinned" | "pin_in_repository">, stop: Pick<TourStop, "check">, step: Pick<StepView, "fresh">, look: CodeLook): boolean =>
  marks(look) && isMarked(stop, step) && view.pinned && view.pin_in_repository === true;

/**
 * How the stage shows a step:
 * - `fresh`: the code is as written, or this look has no mark; nothing is added;
 * - `asking`: pinned, the commit is here, and the answer of `tourCode` has not come;
 * - `written`: the pane shows the code at the commit, with its own lines lit; `switch` says whether **Now** is offered (a gone stop has no now);
 * - `now`: the reader chose Now: the code as it is, the lit lines dashed, and the switch to go back;
 * - `dashed`: not pinned, or the commit is not here, or the core had no code to give: the code as it is now, the lit lines dashed, and the note;
 * - `gone`: a stop that is gone and cannot be shown as written: no code pane, one sentence.
 */
export type WrittenLook = { look: "fresh" } | { look: "asking" } | { look: "written"; switch: boolean; gone: boolean } | { look: "now" } | { look: "dashed" } | { look: "gone" };

export function writtenLook(i: {
  view: Pick<TourView, "pinned" | "pin_in_repository">;
  stop: Pick<TourStop, "check">;
  step: Pick<StepView, "fresh">;
  codeLook: CodeLook;
  /** `vnoc/tourCode`'s answer: undefined while it is asked for, null when the core has none */
  answer: AsWritten | null | undefined;
  choice: Choice;
}): WrittenLook {
  const gone = i.stop.check === "gone";
  if (!marks(i.codeLook) || !isMarked(i.stop, i.step)) return { look: "fresh" };
  if (!wantsWritten(i.view, i.stop, i.step, i.codeLook)) return gone ? { look: "gone" } : { look: "dashed" };
  if (i.answer === undefined) return { look: "asking" };
  if (i.answer === null) return gone ? { look: "gone" } : { look: "dashed" };
  return i.choice === "now" && !gone ? { look: "now" } : { look: "written", switch: !gone, gone };
}

/** The sentence under the text of a step shown as written or as it is now (05 §3). */
export const changedSentence = (gone: boolean): string => (gone ? "This is no longer in the code." : "The code changed since then.");

/** What the cover says about the pin (design 05 §3, 05-on-screen §3): where it is pinned, or that it is not. Nothing for a tour that is being written: it cannot be pinned yet. */
export function pinNote(view: Pick<TourView, "state" | "pinned" | "pin" | "pin_in_repository">): { text: string; title?: string } | null {
  if (isWriting(view)) return null;
  if (!view.pinned || !view.pin) return { text: "not pinned" };
  const at = `pinned at ${view.pin.slice(0, 7)}`;
  return { text: view.pin_in_repository === false ? `${at}, a commit this repository does not have` : at, title: view.pin };
}

/** The lines the cover adds when the code moved on (design 05 §4): the first stop is gone; most of the tour is about older code. */
export function coverNotes(view: Pick<TourView, "stops" | "check" | "written">): { gone: string | null; most: string | null } {
  const first = view.stops[0];
  return {
    gone: first?.check === "gone" ? `${first.name} is no longer in the code. The tour shows what was written about it.` : null,
    most: mostChanged(view) ? "Most of this tour is about older code. Ask the agent for a new one." : null,
  };
}
