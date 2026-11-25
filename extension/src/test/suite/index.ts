import * as path from "path";
import Mocha from "mocha";
import * as vscode from "vscode";
import { CORE_STOPPED, core, forgetUnanswered, markStopped, noAnswer } from "./util";

// The window suites, in the order they run. `bar` stops the core on purpose and restarts it: the suites after it prove it came back.
const SUITES = ["skeleton", "bar", "session", "sidebar", "attention", "refactor", "calls", "edit", "search", "cli", "bend", "tour"];

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
  const asked = (process.env.VNOC_SUITE ?? "").split(",").map((s) => s.trim()).filter(Boolean);
  const unknown = asked.filter((s) => !SUITES.includes(s));
  if (unknown.length > 0) throw new Error(`VNOC_SUITE names no suite: ${unknown.join(", ")} (they are ${SUITES.join(", ")})`);
  if (asked.length === 0) return SUITES;
  const wanted = new Set(asked.flatMap((s) => [s, ...(NEEDS[s] ?? [])]));
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
    if (stopped.has(file)) return failAtOnce(test, CORE_STOPPED);
    const api: { ping(): Promise<unknown> } = await vscode.extensions.getExtension("vnoc.vnoc")!.activate();
    try {
      await core("vnoc/ping", () => api.ping(), PING_MS);
    } catch (e) {
      const said = (e as Error).message;
      forgetUnanswered(); // this bundle's copy of the latch is not a suite's
      if (said === noAnswer("vnoc/ping", PING_MS)) {
        stopped.add(file);
        markStopped(file); // the suite's own bundle: its hooks and its waits fail at once from here
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

export function run(): Promise<void> {
  const mocha = new Mocha({ ui: "tdd", timeout: 60_000, color: false });
  guardTheCore(mocha);
  for (const name of chosen()) mocha.addFile(path.resolve(__dirname, `${name}.test.js`));
  return new Promise((resolve, reject) => mocha.run((failures) => (failures ? reject(new Error(`${failures} tests failed`)) : resolve())));
}
