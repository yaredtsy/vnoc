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
function guardTheCore(mocha: Mocha) {
  const stopped = new Set<string>();
  const failAtOnce = (test: Mocha.Test, why: string) => {
    test.fn = () => {
      throw new Error(why);
    };
  };
  mocha.suite.beforeEach(async function () {
    const test = this.currentTest!;
    const file = test.file ?? "";
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

/** What the window knows at the moment a test failed its first try: the first error, the core's log, the pages' errors and their last reports. */
async function diagnose(title: string, message: string): Promise<void> {
  const lines = [`::group::RETRY ${title}`, `first error: ${message}`];
  try {
    const api = (await vscode.extensions.getExtension("vnoc.vnoc")!.activate()) as Api;
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
  } catch (e) {
    lines.push(`(no diagnosis: ${(e as Error).message})`);
  }
  lines.push("::endgroup::");
  console.log(lines.join("\n"));
}

/** `FLAKY (passed on a later try): n` and a line per test with its first error: to the console, and as Markdown to the step summary when there is one. */
function reportFlaky(flaky: Map<string, string>): void {
  const lines = [...flaky].map(([title, err]) => `${title} — ${err.split("\n")[0]}`);
  const head = `FLAKY (passed on a later try): ${flaky.size}`;
  if (flaky.size > 0) console.log(["", head, ...lines.map((l) => `  ${l}`)].join("\n"));
  const summary = process.env.GITHUB_STEP_SUMMARY;
  if (summary && flaky.size > 0) {
    try {
      fs.appendFileSync(summary, `\n### ${head}\n\n${lines.map((l) => `- ${l.replace(/[|`<]/g, "\\$&")}`).join("\n")}\n`);
    } catch (e) {
      console.log(`(could not write the step summary: ${(e as Error).message})`);
    }
  }
}

export function run(): Promise<void> {
  // `VNOC_GREP=text` runs only the tests whose full title has that text (a test that needs an earlier one of its suite fails by itself: say so)
  const mocha = new Mocha({ ui: "tdd", timeout: 60_000, color: false, retries: retries(), grep: process.env.VNOC_GREP || undefined });
  guardTheCore(mocha);
  for (const name of chosen()) mocha.addFile(path.resolve(__dirname, `${name}.test.js`));
  const firstError = new Map<string, string>(); // tests that failed a try, by full title
  const flaky = new Map<string, string>(); // ... and then passed
  return new Promise((resolve, reject) => {
    const runner = mocha.run((failures) => {
      reportFlaky(flaky);
      failures ? reject(new Error(`${failures} tests failed`)) : resolve();
    });
    // the diagnosis is asked at once (the window is as the failure left it); mocha does not wait for it, so the retry may start first: it is read-only
    runner.on("retry", (test: Mocha.Test, err: Error) => {
      const title = test.fullTitle();
      if (!firstError.has(title)) firstError.set(title, err.message);
      void diagnose(title, err.message);
    });
    runner.on("pass", (test: Mocha.Test) => {
      const title = test.fullTitle();
      if (firstError.has(title)) flaky.set(title, firstError.get(title)!);
    });
  });
}
