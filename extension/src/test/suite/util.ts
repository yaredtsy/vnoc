import * as assert from "assert";
import * as vscode from "vscode";

// helpers for the suites that drive the real window

export const sleep = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));
export const canvasTabs = () => vscode.window.tabGroups.all.flatMap((g) => g.tabs).filter((t) => t.input instanceof vscode.TabInputWebview && t.input.viewType.endsWith("vnoc.canvas"));

export async function until(what: string, check: () => boolean, ms = 20_000) {
  for (let waited = 0; !check(); waited += 100) {
    if (coreStopped()) assert.fail(CORE_STOPPED);
    if (waited >= ms) assert.fail(`timed out waiting for ${what}`);
    await sleep(100);
  }
}

/** Waits for an async check, polling every 200 ms. */
export async function waitFor(what: string, check: () => Promise<boolean> | boolean, ms = 20_000) {
  for (let waited = 0; !(await check()); waited += 200) {
    if (coreStopped()) assert.fail(CORE_STOPPED);
    assert.ok(waited < ms, `timed out waiting for ${what}`);
    await sleep(200);
  }
}

export async function closeCanvas() {
  await vscode.window.tabGroups.close(canvasTabs());
  await until("the canvas tab to close", () => canvasTabs().length === 0);
}


// ── every request a test makes to the core goes through `core` ───────────────────────────────────────────────────────────────────────────────────

/** How long a test waits for the core to answer one request. Mocha's own limit (60 s) stays the outer one. */
export const CORE_LIMIT_MS = 10_000;
export const CORE_STOPPED = "the core stopped answering earlier in this suite";
/** The method of the first request that went unanswered. Each suite file is its own bundle, so this is the suite's: after it the rest of the suite fails at once. */
let unanswered: string | null = null;
/** The bundles (suite files) whose core stopped answering: shared by every bundle of the run, because the check before each test (`index.ts`) is another bundle and marks the suite's. */
const stoppedFiles: Set<string> = ((globalThis as any).__vnocStoppedFiles ??= new Set<string>());
export const markStopped = (file: string) => void stoppedFiles.add(file);

/** The words of a request the core did not answer in time (a test's `afterEach` in `index.ts` looks for them). */
export const noAnswer = (method: string, ms: number) => `the core did not answer ${method} within ${ms / 1000} s`;

/**
 * THE helper: runs one request to the core (`call`) and fails the test with "the core did not answer <method> within 10 s" when it is not answered in time, not after mocha's 60 s.
 * Once one request has gone unanswered, every later one in the suite fails at once with {@link CORE_STOPPED}: one failure is one line, not a chain of slow ones.
 * A request that FAILS (the core is not running, or it said "ask again") is not an unanswered one: the error comes through as it is.
 */
export async function core<T>(method: string, call: () => Promise<T>, ms = CORE_LIMIT_MS): Promise<T> {
  if (coreStopped()) throw new Error(CORE_STOPPED);
  let timer: NodeJS.Timeout | undefined;
  const limit = new Promise<never>((_, reject) => {
    timer = setTimeout(() => {
      unanswered = method;
      markStopped(__filename);
      reject(new Error(noAnswer(method, ms)));
    }, ms);
  });
  try {
    return await Promise.race([call(), limit]);
  } finally {
    clearTimeout(timer);
  }
}

/** For the check that runs before each test (`index.ts`), which has a bundle of its own and must not carry one suite's failure into the next. */
export const forgetUnanswered = () => {
  unanswered = null;
};

/** Has a request of this suite gone unanswered? */
export function coreStopped(): boolean {
  return unanswered !== null || stoppedFiles.has(__filename);
}

/**
 * A hook (`suiteSetup`, `setup`) that waits for the core, made to stand aside when the suite's core has stopped answering: it returns instead of failing, so mocha still runs the suite's tests,
 * and each of them fails at once (`index.ts` puts the failure in its body). The first unanswered request still fails its own hook.
 */
export const soft = (fn: (this: Mocha.Context) => unknown): Mocha.AsyncFunc =>
  async function (this: Mocha.Context) {
    try {
      await fn.call(this);
    } catch (e) {
      if ((e as Error).message !== CORE_STOPPED) throw e;
    }
  };

/** What the suites use as `api`: the extension's test api, with each of its requests to the core going through {@link core}. */
export function guarded<A extends object>(api: A): A {
  const a = api as any;
  const wrapped: any = { ...a };
  const ask = (name: string, label: (args: unknown[]) => string) => {
    if (typeof a[name] === "function") wrapped[name] = (...args: unknown[]) => core(label(args), () => a[name](...args));
  };
  ask("request", (args) => String(args[0]));
  ask("ping", () => "vnoc/ping");
  ask("structure", () => "vnoc/structure");
  for (const name of ["codeOf", "editStartRequest", "editSave", "moveToTrash", "moveAllToTrash", "restoreEntry"]) ask(name, () => `api.${name}`);
  if (a.tours && typeof a.tours.list === "function") wrapped.tours = { ...a.tours, list: (...args: unknown[]) => core("vnoc/tours", () => a.tours.list(...args)) };
  return wrapped;
}
