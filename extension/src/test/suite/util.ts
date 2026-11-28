import * as assert from "assert";
import * as vscode from "vscode";

// helpers for the suites that drive the real window

export const sleep = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));
export const canvasTabs = () => vscode.window.tabGroups.all.flatMap((g) => g.tabs).filter((t) => t.input instanceof vscode.TabInputWebview && t.input.viewType.endsWith("vnoc.canvas"));

/** Every deadline of a test is multiplied by this: 3 on CI, 1 here, or what `VNOC_TEST_SLOWDOWN` says. */
export const SLOWDOWN = Number(process.env.VNOC_TEST_SLOWDOWN) || (process.env.CI ? 3 : 1);
export const deadline = (ms: number) => ms * SLOWDOWN;

/** Waits for a check, every 100 ms, until the deadline (measured on the clock, not by counting naps). */
export async function until(what: string, check: () => boolean, ms = 20_000) {
  const end = Date.now() + deadline(ms);
  while (!check()) {
    if (coreStopped()) assert.fail(CORE_STOPPED);
    if (Date.now() >= end) assert.fail(`timed out waiting for ${what}`);
    await sleep(100);
  }
}

/** Waits for an async check, polling every 200 ms, until the deadline (on the clock). */
export async function waitFor(what: string, check: () => Promise<boolean> | boolean, ms = 20_000) {
  const end = Date.now() + deadline(ms);
  while (!(await check())) {
    if (coreStopped()) assert.fail(CORE_STOPPED);
    assert.ok(Date.now() < end, `timed out waiting for ${what}`);
    await sleep(200);
  }
}

/** The fields of `report` that `expected` names. */
const trimmed = (report: object, expected: object) => Object.fromEntries(Object.keys(expected).map((k) => [k, (report as any)[k]]));
/** Does the report hold every field `expected` names, deep equal (an array or an object as a whole)? */
const holds = (report: object, expected: object) => {
  try {
    assert.deepStrictEqual(trimmed(report, expected), expected);
    return true;
  } catch {
    return false;
  }
};

/**
 * Waits until the newest report made after the ticket (`seq >= after`) holds every field of `expected`, and gives it back. A report from before the action never counts, and a later unrelated one
 * is read as it is: the wait is for the whole state the action asked for. It reads what is there every 50 ms. On the deadline it fails with the expected fields and the newest report after the ticket.
 */
export async function shows<R extends { seq: number }>(what: string, reports: () => R[], expected: Partial<R>, opts: { after: number; ms?: number; also?: () => string }): Promise<R> {
  const end = Date.now() + deadline(opts.ms ?? 20_000);
  const newest = () => reports().filter((r) => r.seq >= opts.after).at(-1);
  for (;;) {
    const r = newest();
    if (r && holds(r, expected)) return r;
    if (coreStopped()) assert.fail(CORE_STOPPED);
    if (Date.now() >= end) {
      const drawn = r ? JSON.stringify(trimmed(r, expected), null, 1) : `no report after seq ${opts.after}`;
      assert.fail([`timed out waiting for ${what}`, `expected: ${JSON.stringify(expected, null, 1)}`, `newest report after seq ${opts.after}: ${drawn}`, opts.also?.()].filter((x) => x).join("\n"));
    }
    await sleep(50);
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
