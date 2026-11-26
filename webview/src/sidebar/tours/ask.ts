// The prompt builder of **Ask for a Tour…** (docs/26-tours/05-on-screen.md §5, TR23a): the form's parts, and the request they make. The sentence is the core's (`vnoc/tourAsk`): nothing
// here builds one, only the choices' labels and the parameters of the request.

import type { AskDetail, AskReach, TourAskParams, TourId } from "@vnoc/shared/tours.ts";

export const REACH_CHOICES: { value: AskReach; label: string }[] = [
  { value: "agent", label: "Let the agent decide" },
  { value: "only", label: "Only this" },
  { value: "calls", label: "And what it calls" },
  { value: "calls2", label: "Two levels of calls" },
];

export const DETAIL_CHOICES: { value: AskDetail; label: string }[] = [
  { value: "agent", label: "Let the agent decide" },
  { value: "quick", label: "Quick" },
  { value: "normal", label: "Normal" },
  { value: "detailed", label: "Detailed" },
];

/** How long after the last change the core is asked for the sentence again. */
export const ASK_DEBOUNCE_MS = 300;

/** What the form holds. `about` is the node (its key) or, for a row's tour, the address of that tour's root; null when there is none. `tour` is set while the row's tour fills the rest. */
export interface AskForm {
  about: string | null;
  tour: TourId | null;
  question: string;
  reach: AskReach;
  detail: AskDetail;
}

/** A form opened from a node (or from nothing): empty but for the node. */
export const formFor = (about: string | null): AskForm => ({ about, tour: null, question: "", reach: "agent", detail: "agent" });

/** A form opened from a row's tour, with what the core gave it (`filled`): its root, its question, its detail. */
export const formFromTour = (tour: TourId, filled: { key?: string; question?: string; detail: AskDetail } | undefined): AskForm => ({
  about: filled?.key ?? null,
  tour: filled?.key !== undefined ? tour : null,
  question: filled?.question ?? "",
  reach: "agent",
  detail: filled?.detail ?? "agent",
});

/**
 * The request the form makes, for the live prompt and for **Copy Prompt**: the node (or the tour that fills the rest), and every part as it stands, so a question that was cleared stays cleared.
 * With the chip removed there is no node and no tour.
 */
export function paramsOf(form: AskForm): TourAskParams {
  const base: TourAskParams = form.about === null ? {} : form.tour !== null ? { tour: form.tour } : { key: form.about };
  return { ...base, question: form.question, reach: form.reach, detail: form.detail };
}

/** The chip's words: the last part of an address (`parse_order` of `orders/service.py::OrderService.parse_order`); a node's name is given by the caller. */
export const shortName = (address: string): string => address.split("::").pop()?.split("/").filter(Boolean).pop() ?? address;
