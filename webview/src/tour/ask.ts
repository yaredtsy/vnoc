// The form of the **Ask for a Tour** dialog (docs/26-tours/05-on-screen.md §5, TR23b), pure: its parts and the request they make. The sentence, the
// words after it and the size of a depth are the core's (`vnoc/tourAsk`): nothing here builds one.

import type { AskDetail, TourAskParams, TourAskSize, TourId } from "@vnoc/shared/tours.ts";

export const DETAIL_CHOICES: { value: AskDetail; label: string }[] = [
  { value: "agent", label: "Let the agent decide" },
  { value: "quick", label: "Quick" },
  { value: "normal", label: "Normal" },
  { value: "detailed", label: "Detailed" },
];

/** The deepest a tour can go from its node (the core refuses more). */
export const MAX_DEPTH = 6;
/** The number the field shows when *Let the agent decide* is unticked. */
export const FIRST_DEPTH = 1;
/** How long after the last change the core is asked for the sentence again. */
export const ASK_DEBOUNCE_MS = 300;
/** How long after the core said to ask again (a size still being worked out). */
export const ASK_AGAIN_MS = 500;

/** What the form holds. `about` is the node (its key) or, for a row's tour, the address of that tour's root; null when there is none. `tour` is set while the row's tour fills the rest. */
export interface AskForm {
  about: string | null;
  tour: TourId | null;
  question: string;
  /** *Let the agent decide* how far: no depth is asked for */
  agentDepth: boolean;
  depth: number;
  detail: AskDetail;
}

/** A form opened from a node (or from nothing): empty but for the node. */
export const formFor = (about: string | null): AskForm => ({ about, tour: null, question: "", agentDepth: true, depth: FIRST_DEPTH, detail: "agent" });

/** A form opened from a row's tour, with what the core gave it (`filled`): its root, its question, its detail. */
export const formFromTour = (tour: TourId, filled: { key?: string; question?: string; detail: AskDetail } | undefined): AskForm => ({
  about: filled?.key ?? null,
  tour: filled?.key !== undefined ? tour : null,
  question: filled?.question ?? "",
  agentDepth: true,
  depth: FIRST_DEPTH,
  detail: filled?.detail ?? "agent",
});

/** A number the person typed, as a depth: a whole number 0 to 6; anything else (an empty box, a letter) keeps the old one. */
export const depthOf = (text: string, old: number): number => {
  const n = Number(text.trim());
  return text.trim() !== "" && Number.isInteger(n) ? Math.min(MAX_DEPTH, Math.max(0, n)) : old;
};

/**
 * The request the form makes, for the live prompt and for **Copy Prompt**: the node (or the tour that fills the rest), and every part as it stands, so a question that was cleared stays cleared.
 * With the chip removed there is no node and no tour, and no depth (it is a depth of a node); *Let the agent decide* sends none.
 */
export function paramsOf(form: AskForm): TourAskParams {
  const base: TourAskParams = form.about === null ? {} : form.tour !== null ? { tour: form.tour } : { key: form.about };
  return { ...base, question: form.question, ...(form.about !== null && !form.agentDepth ? { depth: form.depth } : {}), detail: form.detail };
}

/** The chip's words: the last part of an address (`parse_order` of `orders/service.py::OrderService.parse_order`); a function nested in another is its own name (`one_try` of `retry_charge.<locals>.one_try`). */
export const shortName = (address: string): string => address.split("::").pop()?.split("/").filter(Boolean).pop()?.split("<locals>.").pop() ?? address;

/** The chip's words for a node of the tree, as the cards say it: its qualified name, and a function nested in another its own name (never `<locals>`: the choice is between two words the core sent). */
export const aboutName = (node: { name: string; qualname: string | null } | null | undefined, address: string): string =>
  node ? (node.qualname === null || node.qualname.includes("<locals>") ? node.name : node.qualname) : shortName(address);

/** The core's size of one depth (`9 stops · about 6 min`), from the sizes it gave; null when it gave none. */
export const sizeAt = (sizes: readonly TourAskSize[] | null, depth: number): string | null => sizes?.find((s) => s.depth === depth)?.words ?? null;

/** Whether the core's refusal is its "ask again" (a size still being worked out). */
export const saysAskAgain = (error: unknown): boolean => (error as { askAgain?: unknown } | null | undefined)?.askAgain === true;
