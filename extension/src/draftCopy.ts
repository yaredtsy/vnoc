import * as vscode from "vscode";

import { CLOSE_BUTTONS, DISCARD, REOPEN_CANVAS, SAVE_ALL, type SaveAnswer, START_BUTTONS, closeNotice, countingDrafts, countsOnClose, failedNotice, readDrafts, savedAs, settle, startNotice } from "@vnoc/shared/drafts";
import type { Draft, EditSaveParams, EditSaveResult } from "@vnoc/shared/messages";

// The host's copy of the canvas's drafts (docs/17-edit-in-canvas.md E6, §7, 9.5c). A webview panel cannot be dirty and cannot veto its own close, so VS Code cannot ask before it closes the
// canvas: the page feeds this copy as the drafts change, and the host acts on it only when the canvas is closed or at start, to ask. Everything it decides is `shared/drafts.ts`.

/** The key of the copy in `workspaceState`: the whole list, replaced each time. */
export const DRAFTS_KEY = "vnoc.drafts";

/** A notification with buttons: the button pressed, or `undefined` when it was dismissed. A parameter of the copy, so a test can answer it. */
export type Notify = (message: string, ...buttons: string[]) => Promise<string | undefined>;
export const showNotification: Notify = async (message, ...buttons) => vscode.window.showInformationMessage(message, ...buttons);

/** What the copy needs of the rest of the extension. */
export interface DraftCopyDeps {
  /** Save on a draft: every row of §6, the question about an ID included */
  save(params: EditSaveParams): Promise<EditSaveResult>;
  /** brings the canvas up: its page asks for the copy */
  reopen(): void;
  /** whether a canvas is open, or a tab of one is waiting to be shown */
  canvasOpen(): boolean;
}

export class DraftCopy {
  private held: Draft[];
  /** how a notification is shown; a test replaces it */
  notify: Notify = showNotification;

  constructor(
    private readonly memento: vscode.Memento,
    private readonly deps: DraftCopyDeps,
  ) {
    this.held = readDrafts(memento.get(DRAFTS_KEY));
  }

  get drafts(): Draft[] {
    return this.held;
  }

  /** What the page said (the whole list): the copy is replaced. An empty list clears it: the last draft was saved or discarded. */
  replace(drafts: unknown): void {
    this.set(readDrafts(drafts));
  }

  clear(): void {
    this.set([]);
  }

  private set(next: Draft[]): void {
    this.held = next;
    void this.memento.update(DRAFTS_KEY, next.length > 0 ? next : undefined);
  }

  /**
   * The canvas was closed. Drafts that count: a notification at once, with Save All, Reopen Canvas and Discard; dismissed or ignored, the copy stays and the drafts come back the next
   * time the canvas opens. Nothing counts (an Edit nothing was typed into): there is nothing to lose, and an untouched draft would only be stale when the canvas opens again, so the copy goes.
   */
  async onCanvasClosed(): Promise<void> {
    const counting = countingDrafts(this.held);
    if (counting.length === 0) return this.clear();
    const answer = await this.notify(closeNotice(counting.length), ...CLOSE_BUTTONS);
    if (answer === SAVE_ALL) await this.saveAll();
    else if (answer === REOPEN_CANVAS) this.deps.reopen();
    else if (answer === DISCARD) this.clear();
  }

  /** The extension started (a window reload or a new window): the copy has drafts that count and no canvas is open, so say so; Reopen Canvas or Discard. Save All is not offered here. */
  async atStart(): Promise<void> {
    const counting = countingDrafts(this.held);
    if (counting.length === 0 || this.deps.canvasOpen()) return;
    const answer = await this.notify(startNotice(counting.length), ...START_BUTTONS);
    if (answer === REOPEN_CANVAS) this.deps.reopen();
    else if (answer === DISCARD) this.clear();
  }

  /**
   * Save All: each draft that counts goes through Save, one after the other (two in one file: the first changes the version, so the second takes §6's second row). Saved or written, it leaves
   * the copy; a conflict or a gone function stays in that state, and a save the user cancelled or that failed stays as it was. What stays is said in a second notification, with Reopen Canvas.
   */
  private async saveAll(): Promise<void> {
    const drafts = this.held;
    const answers: (SaveAnswer | "error" | undefined)[] = [];
    for (const d of drafts) {
      if (!countsOnClose(d)) {
        answers.push(undefined);
        continue;
      }
      try {
        answers.push(savedAs(await this.deps.save({ key: d.key, path: d.path, range: d.range, version: d.version, base: d.base, prefix: d.prefix, eol: d.eol, text: d.text })));
      } catch {
        answers.push("error");
      }
    }
    const { remaining, failed } = settle(drafts, answers);
    this.set(remaining);
    if (failed > 0 && (await this.notify(failedNotice(failed), REOPEN_CANVAS)) === REOPEN_CANVAS) this.deps.reopen();
  }
}
