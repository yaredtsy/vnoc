import * as vscode from "vscode";

import { type Copy, DISCARD, PAGES, SAVE_ALL, type SaveAnswer, busyWith, closeButtons, closeNotice, countingDrafts, countsOnClose, emptyCopy, failedNotice, isEmptyCopy, readCopy, reopenButton, savedAs, settle, sourceOf, startButtons, startNotice, withPage } from "@vnoc/shared/drafts";
import type { Draft, EditSaveParams, EditSaveResult, Page } from "@vnoc/shared/messages";

// The host's copy of the drafts of the pages (docs/17-edit-in-canvas.md E6, §7, §8, 9.5c, 9.6): the canvas and the docs tab each hold their drafts and feed this copy as they change. A webview
// panel cannot be dirty and cannot veto its own close, so VS Code cannot ask before it closes one: the host acts on the copy only when a page is closed or at start, to ask. Everything it
// decides is `shared/drafts.ts`.

/** The key of the copy in `workspaceState`: a list per page, replaced as a whole each time. */
export const DRAFTS_KEY = "vnoc.drafts";

/** A notification with buttons: the button pressed, or `undefined` when it was dismissed. A parameter of the copy, so a test can answer it. */
export type Notify = (message: string, ...buttons: string[]) => Promise<string | undefined>;
export const showNotification: Notify = async (message, ...buttons) => vscode.window.showInformationMessage(message, ...buttons);

/** What the copy needs of the rest of the extension. */
export interface DraftCopyDeps {
  /** Save on a draft: every row of §6, the question about an ID included */
  save(params: EditSaveParams): Promise<EditSaveResult>;
  /** brings a page up: it asks for its drafts */
  reopen(page: Page): void;
  /** whether a page is open, or a tab of it is waiting to be shown */
  pageOpen(page: Page): boolean;
}

export class DraftCopy {
  private held: Copy;
  /** how a notification is shown; a test replaces it */
  notify: Notify = showNotification;

  constructor(
    private readonly memento: vscode.Memento,
    private readonly deps: DraftCopyDeps,
  ) {
    this.held = readCopy(memento.get(DRAFTS_KEY));
  }

  /** One page's drafts. */
  drafts(page: Page): Draft[] {
    return this.held[page];
  }

  /** Every draft of both pages (the test suite reads this). */
  get all(): Draft[] {
    return [...this.held.canvas, ...this.held.docs];
  }

  /** What a page said (its whole list): that page's list is replaced, the other's is kept. An empty list clears it: the last draft was saved or discarded. */
  replace(page: Page, drafts: unknown): void {
    this.set(withPage(this.held, page, readCopy({ [page]: drafts })[page]));
  }

  clear(page: Page): void {
    this.set(withPage(this.held, page, []));
  }

  /** Whether `asking` may not edit this doc because the other page has a draft of it (§8). */
  busy(path: string, asking: Page): boolean {
    return busyWith(this.held, path, asking, (page) => this.deps.pageOpen(page));
  }

  private set(next: Copy): void {
    this.held = next;
    void this.memento.update(DRAFTS_KEY, isEmptyCopy(next) ? undefined : next);
  }

  /**
   * A page was closed. Drafts that count: a notification at once, with Save All, Reopen and Discard, naming the page; dismissed or ignored, the copy stays and the drafts come back the next
   * time the page opens. Nothing counts (an Edit nothing was typed into): there is nothing to lose, and an untouched draft would only be stale when the page opens again, so its list goes.
   */
  async onPageClosed(page: Page): Promise<void> {
    const counting = countingDrafts(this.held[page]);
    if (counting.length === 0) return this.clear(page);
    const answer = await this.notify(closeNotice(counting.length, page), ...closeButtons(page));
    if (answer === SAVE_ALL) await this.saveAll([page]);
    else if (answer === reopenButton(page)) this.deps.reopen(page);
    else if (answer === DISCARD) this.clear(page);
  }

  /**
   * The extension started (a window reload or a new window): the pages that have drafts that count and are not open, said in one notification (the canvas, the docs tab, or V-NOC for both);
   * Reopen brings those pages back, Discard clears their lists. Save All is not offered here.
   */
  async atStart(): Promise<void> {
    const pages = PAGES.filter((page) => countingDrafts(this.held[page]).length > 0 && !this.deps.pageOpen(page));
    const source = sourceOf(pages);
    if (source === null) return;
    const n = pages.reduce((sum, page) => sum + countingDrafts(this.held[page]).length, 0);
    const answer = await this.notify(startNotice(n, source), ...startButtons(source));
    if (answer === reopenButton(source)) pages.forEach((page) => this.deps.reopen(page));
    else if (answer === DISCARD) pages.forEach((page) => this.clear(page));
  }

  /**
   * Save All: each draft that counts goes through Save, one after the other (two in one file: the first changes the version, so the second takes §6's second row). Saved or written, it leaves
   * the copy; a conflict or a gone function stays in that state, and a save the user cancelled or that failed stays as it was. What stays is said in a second notification, with the reopen button.
   */
  private async saveAll(pages: Page[]): Promise<void> {
    let failed = 0;
    for (const page of pages) {
      const drafts = this.held[page];
      const answers: (SaveAnswer | "error" | undefined)[] = [];
      for (const d of drafts) {
        if (!countsOnClose(d)) {
          answers.push(undefined);
          continue;
        }
        try {
          answers.push(savedAs(await this.deps.save({ ...(d.kind ? { kind: d.kind } : {}), key: d.key, path: d.path, range: d.range, version: d.version, base: d.base, prefix: d.prefix, eol: d.eol, text: d.text })));
        } catch {
          answers.push("error");
        }
      }
      const settled = settle(drafts, answers);
      this.set(withPage(this.held, page, settled.remaining));
      failed += settled.failed;
    }
    const source = sourceOf(pages)!;
    if (failed > 0 && (await this.notify(failedNotice(failed, source), reopenButton(source))) === reopenButton(source)) pages.forEach((page) => this.deps.reopen(page));
  }
}

export { emptyCopy };
