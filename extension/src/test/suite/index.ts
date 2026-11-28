import * as fs from "fs";
import * as path from "path";
import Mocha from "mocha";
import * as vscode from "vscode";
import { CORE_STOPPED, core, forgetUnanswered, markStopped, noAnswer } from "./util";

// The window suites, in the order they run. `bar` stops the core on purpose and restarts it: the suites after it prove it came back.
const SUITES = ["skeleton", "bar", "session", "sidebar", "attention", "refactor", "calls", "edit", "search", "cli", "bend", "tour"];
/** Suites that need a window of their own, with a core that is slow on purpose (`runTest.ts` starts it with `VNOC_WINDOW=held`): in no other window, and no other suite in that one. */
const HELD = ["waits"];

/**
 * Suites that do not pass alone, and what they need first (found by running each suite alone). `attention` and `calls` wait for the host to read the structure,
 * and the host reads it only while a page is open to ask: `session` shows the STRUCTURE side bar, whose page stays. `skeleton` does that too, with the canvas,
 * but its page does not stay for `attention`. The others (skeleton, bar, session, sidebar, refactor, edit, search, cli, bend, tour) pass alone.
 */
const NEEDS: Record<string, string[]> = { attention: ["session"], calls: ["session"] };

/**
 * `VNOC_SUITE=edit` (or `edit,calls`) runs only the named suites and what they need, in the order above; unset, all of them. A name that is not a suite is an
 * error, not an empty run.
 */
function chosen(): string[] {
  if (process.env.VNOC_WINDOW === "held") return HELD;
  const asked = (process.env.VNOC_SUITE ?? "").split(",").map((s) => s.trim()).filter(Boolean);
  const unknown = asked.filter((s) => !SUITES.includes(s) && !HELD.includes(s));
  if (unknown.length > 0) throw new Error(`VNOC_SUITE names no suite: ${unknown.join(", ")} (they are ${[...SUITES, ...HELD].join(", ")})`);
  if (asked.length > 0 && asked.every((s) => HELD.includes(s))) return [];
  if (asked.length === 0) return SUITES;
  const wanted = new Set(asked.filter((s) => !HELD.includes(s)).flatMap((s) => [s, ...(NEEDS[s] ?? [])]));
  return SUITES.filter((s) => wanted.has(s));
}

/** How long the check before each test waits for `vnoc/ping`. */
const PING_MS = 3_000;

/**
 * A core that stops answering must be ONE failure, not a chain of 60 s ones. Before every test one `ping` is asked (3 s); a core that does not answer fails that test and, with every
 * request a test makes (`util.core`), the rest of the suite's tests: each of them fails at once with "the core stopped answering earlier in this suite" (not skipped). A core that is not
 * running (a request fails at once: `bar` stops it on purpose) is not a core that stopped answering: the test goes on.
 */
function guardTheCore(mocha: Mocha, retrying: Retrying) {
  const stopped = new Set<string>();
  const failAtOnce = (test: Mocha.Test, why: string) => {
    test.fn = () => {
      throw new Error(why);
    };
  };
  mocha.suite.beforeEach(async function () {
    const test = this.currentTest!;
    const file = test.file ?? "";
    if (retrying.stopped) test.retries(0); // the run is broken, not flaky: no more second tries
    // a test that fails because the core stopped is not retried: the retry would ask the same core again (a second 3 s ping, a second line)
    if (stopped.has(file)) {
      test.retries(0);
      return failAtOnce(test, CORE_STOPPED);
    }
    const api: { ping(): Promise<unknown> } = await vscode.extensions.getExtension("vnoc.vnoc")!.activate();
    try {
      await core("vnoc/ping", () => api.ping(), PING_MS);
    } catch (e) {
      const said = (e as Error).message;
      forgetUnanswered(); // this bundle's copy of the latch is not a suite's
      if (said === noAnswer("vnoc/ping", PING_MS)) {
        stopped.add(file);
        markStopped(file); // the suite's own bundle: its hooks and its waits fail at once from here
        test.retries(0);
        failAtOnce(test, said);
      }
    }
  });
  mocha.suite.afterEach(function () {
    const test = this.currentTest!;
    if (test.state === "failed" && test.err?.message?.startsWith("the core did not answer ")) {
      stopped.add(test.file ?? "");
      markStopped(test.file ?? "");
    }
  });
}

/** How many more tries a failed test gets: `VNOC_RETRIES=<n>` anywhere (0 = none); unset, 1 on CI (`CI` is set) and 0 on a developer's machine. */
function retries(): number {
  const asked = process.env.VNOC_RETRIES;
  if (asked !== undefined && asked !== "") {
    const n = Number(asked);
    if (!Number.isInteger(n) || n < 0) throw new Error(`VNOC_RETRIES must be 0 or a whole number, not "${asked}"`);
    return n;
  }
  return process.env.CI ? 1 : 0;
}

type Api = Record<string, (() => unknown) | undefined>;
const last = (what: unknown, n = 5) => (Array.isArray(what) ? what.slice(-n) : what);

/** The tests retried in this run, and whether the cap on them has been reached (then the guard in `guardTheCore` gives every later test no retry). */
type Retrying = { tests: Set<string>; stopped: boolean; cap: number };

/** How many tests may be retried in one run before the retries stop: `VNOC_RETRY_CAP=<n>`, else 5. */
function retryCap(): number {
  const asked = process.env.VNOC_RETRY_CAP;
  if (asked === undefined || asked === "") return 5;
  const n = Number(asked);
  if (!Number.isInteger(n) || n < 0) throw new Error(`VNOC_RETRY_CAP must be a whole number, not "${asked}"`);
  return n;
}

/** What the window knows at the moment a test failed its first try: the first error, the core's log, the pages' errors and their last reports. Synchronous: it reads before the retry draws anything. */
function diagnose(api: Api, title: string, message: string): void {
  const lines = [`::group::RETRY ${title}`, `first error: ${message}`];
  const show = (name: string, tail: boolean) => {
    try {
      const v = api[name]?.();
      if (v === undefined) return;
      const text = typeof v === "string" ? v : JSON.stringify(tail ? last(v) : v, null, 1);
      lines.push(`--- ${name}${tail ? " (last 5)" : ""}`, text);
    } catch (e) {
      lines.push(`--- ${name}: ${(e as Error).message}`);
    }
  };
  show("logTail", false);
  show("webviewErrors", false);
  for (const name of ["stageDrawn", "toursDrawn", "sidebarPosts"]) show(name, true);
  lines.push("::endgroup::");
  console.log(lines.join("\n"));
}

/** `FLAKY (passed on a later try): n` and a line per test with its first error: to the console, and as Markdown to the step summary when there is one. */
function reportFlaky(flaky: Map<string, string>, stoppedLine: string | null): void {
  const lines = [...flaky].map(([title, err]) => `${title} — ${err.split("\n")[0]}`);
  if (stoppedLine) lines.push(stoppedLine);
  const head = `FLAKY (passed on a later try): ${flaky.size}`;
  if (lines.length > 0) console.log(["", head, ...lines.map((l) => `  ${l}`)].join("\n"));
  const summary = process.env.GITHUB_STEP_SUMMARY;
  if (summary && lines.length > 0) {
    try {
      fs.appendFileSync(summary, `\n### ${head}\n\n${lines.map((l) => `- ${l.replace(/[|`<]/g, "\\$&")}`).join("\n")}\n`);
    } catch (e) {
      console.log(`(could not write the step summary: ${(e as Error).message})`);
    }
  }
}

export async function run(): Promise<void> {
  // the api is got once, here: the `retry` handler reads it without waiting for anything
  const api = (await vscode.extensions.getExtension("vnoc.vnoc")!.activate()) as Api;
  // `VNOC_GREP=text` runs only the tests whose full title has that text (a test that needs an earlier one of its suite fails by itself: say so)
  const mocha = new Mocha({ ui: "tdd", timeout: 60_000, color: false, retries: retries(), grep: process.env.VNOC_GREP || undefined });
  const retrying: Retrying = { tests: new Set(), stopped: false, cap: retryCap() };
  guardTheCore(mocha, retrying);
  for (const name of chosen()) mocha.addFile(path.resolve(__dirname, `${name}.test.js`));
  const firstError = new Map<string, string>(); // tests that failed a try, by full title
  const flaky = new Map<string, string>(); // ... and then passed
  let stoppedLine: string | null = null;
  return new Promise((resolve, reject) => {
    const runner = mocha.run((failures) => {
      reportFlaky(flaky, stoppedLine);
      failures ? reject(new Error(`${failures} tests failed`)) : resolve();
    });
    runner.on("retry", (test: Mocha.Test, err: Error) => {
      const title = test.fullTitle();
      if (!firstError.has(title)) firstError.set(title, err.message);
      diagnose(api, title, err.message);
      retrying.tests.add(title);
      if (!retrying.stopped && retrying.tests.size >= retrying.cap) {
        retrying.stopped = true;
        stoppedLine = `RETRIES STOPPED after ${retrying.cap} tests: this run is broken, not flaky`;
        console.log(stoppedLine);
      }
    });
    runner.on("pass", (test: Mocha.Test) => {
      const title = test.fullTitle();
      if (firstError.has(title)) flaky.set(title, firstError.get(title)!);
    });
  });
}
