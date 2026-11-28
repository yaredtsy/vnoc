// The **Ask for a Tour** dialog's state (docs/26-tours/05-on-screen.md §5, TR23b): what is open, and what its buttons do. The form and the request it makes are `ask.ts`; the sentence, the words
// after it and the size of a depth are the core's (`vnoc/tourAsk`).

import { create } from "zustand";

import type { TourAskSize, TourId } from "@vnoc/shared/tours.ts";
import { request } from "../host.ts";
import { type AskForm, formFor, formFromTour, paramsOf } from "./ask.ts";

export interface AskDialog {
  form: AskForm;
  /** the core's sentence for the parts now; null before the first answer */
  prompt: string | null;
  /** the core's size of each depth of the node; null when it gave none (no node, or it could not say) */
  sizes: TourAskSize[] | null;
}

export const useAskDialog = create<{ dialog: AskDialog | null }>(() => ({ dialog: null }));

/** Opens the dialog, filled in from a node (`key`) or a row's tour (`tour`: its root, question and detail, as the core gives them). Opened again, it starts anew. */
export async function openAskDialog(from: { key?: string; tour?: TourId }): Promise<void> {
  if (from.tour === undefined) {
    useAskDialog.setState({ dialog: { form: formFor(from.key ?? null), prompt: null, sizes: null } });
    return;
  }
  const tour = from.tour;
  try {
    const answer = await request("tourAskText", { tour });
    useAskDialog.setState({ dialog: { form: formFromTour(tour, answer.filled), prompt: answer.text, sizes: null } });
  } catch {
    // the tour is gone: the dialog opens empty
    useAskDialog.setState({ dialog: { form: formFor(null), prompt: null, sizes: null } });
  }
}

export const closeAskDialog = () => useAskDialog.setState({ dialog: null });

export const changeAskForm = (part: Partial<AskForm>) => useAskDialog.setState((s) => (s.dialog ? { dialog: { ...s.dialog, form: { ...s.dialog.form, ...part } } } : s));

/** **Copy Prompt**: the host asks the core, copies and says the core's words; the dialog closes. A refusal keeps it open. */
export async function copyAskPrompt(): Promise<boolean> {
  const d = useAskDialog.getState().dialog;
  if (!d) return false;
  try {
    await request("tourAsk", paramsOf(d.form));
  } catch {
    return false;
  }
  closeAskDialog();
  return true;
}
