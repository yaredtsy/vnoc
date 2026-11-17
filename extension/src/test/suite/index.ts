import * as path from "path";
import Mocha from "mocha";

// The window suites, in the order they run. `bar` stops the core on purpose and restarts it: the suites after it prove it came back.
const SUITES = ["skeleton", "bar", "session", "sidebar", "attention", "refactor", "calls", "edit", "search"];

/**
 * Suites that do not pass alone, and what they need first (found by running each suite alone). `attention` and `calls` wait for the host to read the structure,
 * and the host reads it only while a page is open to ask: `session` shows the STRUCTURE side bar, whose page stays. `skeleton` does that too, with the canvas,
 * but its page does not stay for `attention`. The others (skeleton, bar, session, sidebar, refactor, edit, search) pass alone.
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

export function run(): Promise<void> {
  const mocha = new Mocha({ ui: "tdd", timeout: 60_000, color: false });
  for (const name of chosen()) mocha.addFile(path.resolve(__dirname, `${name}.test.js`));
  return new Promise((resolve, reject) => mocha.run((failures) => (failures ? reject(new Error(`${failures} tests failed`)) : resolve())));
}
