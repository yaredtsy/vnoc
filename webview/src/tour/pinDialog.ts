// The **Commit and Pin…** dialog (docs/26-tours/05-on-screen.md §3, §5 "Decided for TR22a"): the only dialog a tour has. The canvas page draws it. This holds its state and what its two
// buttons do: the files and their diffs are the core's (the dry pin's answer), the message is the user's, and nothing is committed until **Commit and Pin**.

import { create } from "zustand";

import { request } from "../host.ts";
import { defaultMessage, dialogFiles } from "./pin.ts";

export interface Dialog {
  tour: string;
  /** the files that will be committed; null while the core is asked */
  files: string[] | null;
  message: string;
  /** *Always commit and pin without asking in this project* */
  always: boolean;
  /** git's message of a commit that failed, or why there is nothing to commit */
  error: string | null;
  /** a commit is being made */
  busy: boolean;
}

export const usePinDialog = create<{ dialog: Dialog | null }>(() => ({ dialog: null }));

const set = (change: Partial<Dialog>) => usePinDialog.setState((s) => (s.dialog ? { dialog: { ...s.dialog, ...change } } : s));
const wording = (e: unknown): string => (e instanceof Error ? e.message : String(e));

/** Opens the dialog for a tour: the dry pin says which files would be committed (nothing is written). A second call while it is open for that tour keeps what was typed. */
export async function openPinDialog(tour: string, title: string | null): Promise<void> {
  if (usePinDialog.getState().dialog?.tour === tour) return;
  usePinDialog.setState({ dialog: { tour, files: null, message: defaultMessage(title ?? tour), always: false, error: null, busy: false } });
  try {
    const answer = await request("tourPin", { id: tour, dry: true });
    if (usePinDialog.getState().dialog?.tour !== tour) return;
    const files = dialogFiles(answer);
    set({ files, error: files.length > 0 ? null : (answer.reasons?.[0]?.words ?? (answer.state === "pinned" ? "There is nothing to commit: the tour can be pinned as it is." : "There is nothing to commit.")) });
  } catch (e) {
    if (usePinDialog.getState().dialog?.tour === tour) set({ files: [], error: wording(e) });
  }
}

export const closePinDialog = () => usePinDialog.setState({ dialog: null });
export const setMessage = (message: string) => set({ message });
export const setAlways = (always: boolean) => set({ always });

/**
 * **Commit and Pin**: the core commits those files and nothing else, then pins at the new commit. *Always commit and pin without asking in this project* is sent (`tourSettings { commit: "always" }`) only after a
 * commit was made. A commit that fails shows git's own message in the dialog and keeps it open; one that was made but did not pin says so under the files.
 */
export async function commitAndPin(): Promise<void> {
  const d = usePinDialog.getState().dialog;
  if (!d || d.busy || d.files === null || d.files.length === 0) return;
  set({ busy: true, error: null });
  const message = d.message.trim();
  try {
    const answer = await request("tourPin", { id: d.tour, commit: message === "" ? {} : { message } });
    const committed = answer.state === "pinned" || answer.committed !== undefined;
    if (committed && d.always) {
      try {
        await request("tourSettings", { commit: "always" });
      } catch (e) {
        return set({ busy: false, files: [], error: `${answer.state === "pinned" ? "Committed and pinned" : "Committed"}, but the setting was not saved: ${wording(e)}` });
      }
    }
    if (answer.state === "pinned") return closePinDialog();
    set({ busy: false, files: [], error: `${answer.committed ? `Committed as ${answer.committed.commit.slice(0, 7)}, but not pinned` : "Nothing was committed"}: ${answer.reasons?.[0]?.words ?? answer.state}` });
  } catch (e) {
    set({ busy: false, error: wording(e) });
  }
}
