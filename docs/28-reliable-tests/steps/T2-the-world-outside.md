# T2 — The world outside: settled, the clock, the reset

## Goal
A test that writes a file, runs `vnoc` or `git`, restarts the core, or needs "nothing happened" has a certain wait. Suites stop leaning on each other.

## What changes

**Commit 1 — `settled` and `pageReady`**
- Core: `vnoc/settled { paths? }` ([04-design.md](../04-design.md) §3.1). A Rust test: write a file, call it with the path, the index has the file when it answers; with a build held (`VNOC_HOLD_FIRST_BUILD_MS`) it answers only after.
- Host: counts requests in flight and queued events; `sync` round trip to each page; `api.settled()`.
- Host + pages: `pageReady`, and events held for a page until it. Read first whether a user's command can be lost the same way; if yes, say so in the commit: it is a product fix.
- Test API: `api.write`, `api.run`, `api.git`, each ending in `settled`.

**Commit 2 — the clock and the reset**
- Tour ids: read how `tour start` makes an id and how the list orders tours. Write the finding here. Then either the core makes ids that differ within a second (product), or the suite gives the start time. The 20 `sleep(1100)` go.
- `vnoc.test.reset` ([04-design.md](../04-design.md) §7), and its test: an answer held in one suite, released after the reset, changes nothing in the next.
- `scripts/no-sleep.mjs` with today's counts per file, run by `pnpm --filter vnoc unit`.

## The tests that prove it
- "Write IDs on a clean tour…" rewritten with `api.write`/`api.run` and `settled`: its hand-made "the same revision for 1.5 s" loop is gone. 5 green under stress.
- The reset's own test.
- `VNOC_SUITE=attention` and `VNOC_SUITE=calls` pass alone (they call the reset and open what they need): `NEEDS` is empty.

## Done when
The three above hold, and the sleep count is about 58 (the 20 id sleeps and the 9 poll naps gone).

## Open, to settle in this step
- Is "the core read these paths now" cheap for the core, or does it need a full rescan?
- After `restartCore()`, does `settled` wait for the new core's first build? (T0c's finding decides.)
