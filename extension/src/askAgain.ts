// A request the core cannot answer yet says so in data: `{ "askAgain": true }` beside its words (it waited as long as it may for the first build or for an update that
// was running, or the answer is still being made). The words are the core's; this file makes no sentence. No `vscode` in it, so it is tested with `node --test`.

/** How long the host waits before it asks again. */
export const ASK_AGAIN_MS = 2000;

/** Whether a failed request is the core's "ask again": its `data.askAgain`, not its words. */
export const asksAgain = (error: unknown): boolean => (error as { data?: { askAgain?: unknown } } | null | undefined)?.data?.askAgain === true;

/** The core's words for a failure, as they were sent. */
export const wordsOf = (error: unknown): string => String((error as { message?: unknown } | null | undefined)?.message ?? error);

export interface AskOptions {
  /** waits this long; the host's timer (a test's is instant) */
  sleep(ms: number): Promise<void>;
  /** the core's words while it is asked again, and `null` when that ends (an answer, or another failure): what a page draws as a quiet line */
  waiting?(words: string | null): void;
}

/**
 * Asks, and asks again every `ASK_AGAIN_MS` while the core says to ask again and `stillWanted()` is true; there is no cap (the core always ends what it waits for: the build ends,
 * an update is bounded). Any other failure is the caller's, as it was. When what asked is not wanted any more, the refusal is thrown at once and nothing more is asked.
 */
export async function askUntilAnswered<T>(ask: () => Promise<T>, stillWanted: () => boolean, options: AskOptions): Promise<T> {
  let waited = false;
  try {
    for (;;) {
      try {
        return await ask();
      } catch (e) {
        if (!asksAgain(e) || !stillWanted()) throw e;
        waited = true;
        options.waiting?.(wordsOf(e));
        await options.sleep(ASK_AGAIN_MS);
        if (!stillWanted()) throw e;
      }
    }
  } finally {
    if (waited) options.waiting?.(null);
  }
}
