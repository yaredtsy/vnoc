// The pin and the IDs of a tour, on screen (docs/26-tours/05-on-screen.md §3 "Pinned, and as written", §5 "Decided for TR22a"). Pure: what the cover says under `not pinned`, the lines it
// offers, and the files of the **Commit and Pin…** dialog. Every sentence about *why* is the core's (`PinReason.words`); what is made here is only the offers' lines (their numbers
// are the core's: `TourRow.offers`) and the dialog's wording.

import type { OffersView, TourPinAnswer, TourView } from "@vnoc/shared/tours.ts";
import { callsShape } from "./map.ts";
import { pinNote } from "./written.ts";

/** The cover asks the core why the tour is not pinned only for a tour that is finished and not pinned: a tour being written cannot be pinned yet, and a pinned one needs no reason. */
export const wantsDryPin = (view: Pick<TourView, "state" | "pinned">): boolean => view.state === "finished" && !view.pinned;

/**
 * What the dry pin's answer depends on, as the page sees it: the tour, how much of it is written, where its stops are and what a check said of them, whether it is pinned, and what the
 * core offered. The cover asks again only when this changed (and when it is shown): a `tour` event that only moved the reader asks nothing, as the core answers one request per CPU at once.
 */
export const dryKey = (view: TourView): string => `${view.id}|${view.state}|${view.written}|${view.pinned}|${view.pin ?? ""}|${callsShape(view)}|${JSON.stringify(view.offers)}`;

/** One thing the cover offers, by the project's settings: the sentence and its button. */
export interface Offer {
  kind: "ids" | "commit";
  text: string;
  button: string;
}

/**
 * The cover's offers (05 §3): the core gives the numbers by `[tours] ids` and `commit` (`TourRow.offers`: only `"ask"` has any, `"always"` was done at `finish`, `"never"` offers
 * nothing), and here they become sentences: *"4 stops have no ID: the tour will not follow a rename."* **Write IDs**, *"2 files have changes that are not committed."* **Commit and Pin…**.
 */
export function offersOf(offers: OffersView | null): Offer[] {
  if (offers === null) return [];
  const out: Offer[] = [];
  if (offers.ids > 0) out.push({ kind: "ids", text: `${offers.ids === 1 ? "1 stop has" : `${offers.ids} stops have`} no ID: the tour will not follow a rename.`, button: "Write IDs" });
  if (offers.commit.length > 0) out.push({ kind: "commit", text: `${offers.commit.length === 1 ? "1 file has" : `${offers.commit.length} files have`} changes that are not committed.`, button: "Commit and Pin…" });
  return out;
}

/**
 * What the cover says of the pin: its line (`pinned at f4d6e15`, `not pinned`), under `not pinned` the reasons that have **no offer**, in the core's words, and the offers. A reason that has an
 * offer is said once, as its offer (*"2 files have changes that are not committed."* with its button), not again as a reason. `dry` is the dry pin's answer: undefined while it is awaited.
 */
export interface CoverPin {
  pin: { text: string; title?: string } | null;
  reason: string | null;
  offers: Offer[];
}

export function coverPin(view: Pick<TourView, "state" | "pinned" | "pin" | "pin_in_repository" | "offers">, dry: TourPinAnswer | null | undefined): CoverPin {
  const pin = pinNote(view);
  const unpinned = pin !== null && pin.text === "not pinned";
  const offers = view.state === "finished" ? offersOf(view.offers) : [];
  // the one reason that has an offer is the files that are not committed (`commit`'s)
  const offered = (code: string) => code === "uncommitted" && offers.some((o) => o.kind === "commit");
  const said = unpinned && wantsDryPin(view) ? (dry?.reasons ?? []).filter((r) => !offered(r.code)).map((r) => r.words) : [];
  return { pin, reason: said.length > 0 ? said.join(" · ") : null, offers };
}

/** The message the dialog proposes (the core's own default, `Code for the tour "<title>"`). */
export const defaultMessage = (title: string): string => `Code for the tour "${title}"`;

/**
 * The files **Commit and Pin** will commit, from the dry pin's answer: the files of its `uncommitted` reason, then the `node.toml` of the tour's IDs that go with them (`node_files`), each
 * once. Empty when nothing is in the way of a commit (the tour cannot be pinned for another reason, or can be pinned as it is).
 */
export function dialogFiles(answer: Pick<TourPinAnswer, "reasons" | "node_files">): string[] {
  const reason = (answer.reasons ?? []).find((r) => r.code === "uncommitted");
  const code = reason && reason.code === "uncommitted" ? reason.files.map((f) => f.path) : [];
  return [...new Set([...code, ...(code.length > 0 ? (answer.node_files ?? []) : [])])];
}
