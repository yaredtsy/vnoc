# T1 — One action, end to end: Next on a tour

## Goal
Prove the design on the action that failed on CI (`Next walks to the end card`, `at s1.b3`) before any other test depends on it. Two halves, one commit each.

## T1a — Window: the ticket and `shows`

**What changes**
- `shared/messages.ts`: `viewCommand` carries `seq`; a `handled` note; `seq` on `stageDrawn`.
- `extension/src/extension.ts` (test API): `stageClick` and `stageKey` return a promise of the ticket.
- `webview/src/tour/Stage.tsx`: post `handled` after the drawing that follows the action; put `seq` on `stageDrawn`.
- `extension/src/test/suite/util.ts`: `shows(reports, what, expected, { after, ms })`: clock deadline, times `VNOC_TEST_SLOWDOWN`, fails with the difference. `until` measures the clock.
- `extension/src/test/runTest.ts`: `--force-prefers-reduced-motion`. Check with a test that the page reports reduced motion; if the flag does not reach a webview, write that here and set it through the test API.
- `tour.test.ts`: only "Next walks to the end card…" is rewritten, by [03-rules.md](../03-rules.md) rules 1 and 2. The old helpers stay for the other tests.

**The test that proves it**
The rewritten Next test, 10 times in a row with `VNOC_STRESS`-like load (a busy loop per CPU, by hand for now), retries off: 10 green.

## T1b — Page: the harness and the real button

**What changes**
- `webview/e2e/`: `@playwright/test`, a config (Chromium, reduced motion, 1280×800), the host stand-in (`acquireVsCodeApi`, answers from fixtures, `hold`/`release`), the theme variables.
- `VNOC_RECORD=<dir>` in the window run: saves the traffic of the tour scenario as the first fixture.
- `webview/e2e/tour-next.spec.ts`:
  1. **Next, by the real button:** from step 1, click Next: step 2 is active, its words are visible, its lines are lit. Through every step to the end card, and back with Prev.
  2. **The stale-draw case:** hold the answer for the next step, click Next, resize the viewport (an unrelated redraw). The assertion is still waiting. Release: it passes.
  3. **Same function, two steps:** s1.b2 → s1.b3 (the same code text): the lit lines are the new step's.
- `package.json` script `pnpm --filter vnoc-webview e2e`; a CI step on Ubuntu only.

## Done when
- Both tests green 10 times in a row locally; the page spec runs in under 30 s.
- This file records: does the reduced-motion flag reach the webview; how long the page spec takes; anything the fixture could not express.
- **The decision:** if the harness cannot play the tour page honestly (too much of the host to fake), stop and write why. T3 then keeps the checks in the window with T1a's helpers, and the plan is smaller.

## Not in this step
No other test changes. No `settled` yet: Next touches no file.

---

## Decisions for T1a (2026-11-28)

- **When the ticket resolves:** the page posts `handled { seq }` right after its handler for the action has run. That proves the page got it (an action sent to a page that does not listen fails in 10 s with its own message, not as a wrong drawing later). It does not prove the result is drawn: that is `shows`' job.
- **`seq` on a report** is the last action the page had handled when it drew. `shows(…, { after: n })` reads only reports with `seq >= n` and wants every field it names. Together they shut out the old drawing and the unrelated redraw.
- **Only the stage in T1a:** `stageClick`, `stageKey`, `stageDrawn`. Other actions and reports get it in T4/T5.
- **Built first:** the extension's `test` script runs its own `build` first (the `dist/` trap of T0a3).

## Prompt for the Sonnet session (T1a, two commits)

```
You are working in /Users/yared/Documents/Programing/rust/vnoc on branch `reliable-tests`.
PR #33 (T0) is open from this branch: commit locally, do NOT push until it is merged.
Read first: docs/28-reliable-tests/03-rules.md (rules 1, 2, 7), docs/28-reliable-tests/04-design.md §2 and §5,
docs/28-reliable-tests/steps/T1-one-action.md (all of it), then:
shared/messages.ts (Events.viewCommand, Notes stageDrawn), webview/src/tour/Stage.tsx (the viewCommand handler
and the stageDrawn effect), webview/src/lib/motion.ts, extension/src/extension.ts (the test api: stageKey,
stageClick, stageDrawn, and where notes from pages are received), extension/src/test/suite/util.ts (until, waitFor),
extension/src/test/runTest.ts, extension/src/test/suite/tour.test.ts (lines 1–330: stageShows, toCover,
openAtCover, and the test "Next walks to the end card through a block and its details, and Prev walks back").

Commit 1 — the protocol. Files: shared/messages.ts, webview/src/tour/Stage.tsx, extension/src/extension.ts,
extension/src/test/suite/util.ts, extension/src/test/runTest.ts, extension/package.json.
1. viewCommand gets an optional `seq?: number`. A new note `{ type: "handled"; seq: number }`.
   stageDrawn gets `seq: number` (0 before any action) and `reducedMotion: boolean` (prefersReducedMotion()).
2. Stage.tsx: when it handles a stageKey or stageClick that has a seq, it stores it in a ref and, after the
   handler has run, posts `handled`. Every stageDrawn carries the ref's value. Nothing else in the page changes.
3. The test api: stageKey and stageClick take a host counter `n` (one counter for all actions), emit the
   viewCommand with seq n, and return a Promise<number> that resolves with n when a `handled` note with seq n
   arrives. If none arrives in 10 s (× slowdown, below) it rejects with
   `the page never handled <name> <key> (seq <n>): no page listens, or it threw`. Existing callers that do not
   await keep working.
4. util.ts:
   - `SLOWDOWN = Number(process.env.VNOC_TEST_SLOWDOWN) || (process.env.CI ? 3 : 1)`; export `deadline(ms)` = ms × SLOWDOWN.
   - `until` and `waitFor` measure Date.now(), not the count of naps, and use deadline(ms).
   - `shows<R>(what, reports: () => R[], expected: Partial<R>, opts: { after: number; ms?: number; also?: () => string })`:
     resolves with the newest report with `seq >= after` whose fields equal `expected` (assert.deepStrictEqual on
     only the named fields; for an array or object field the whole value). It checks every 50 ms (a check, not a
     guess: it reads what is there). On the deadline it fails with: what; the expected fields; the newest report
     after the ticket, trimmed to those fields, or "no report after seq <n>"; then `also()` if given.
5. runTest.ts: add `--force-prefers-reduced-motion` to launchArgs.
6. extension/package.json: the `test` script runs `pnpm build &&` first (the extension's own esbuild build).

Commit 2 — Next, by the rules. File: extension/src/test/suite/tour.test.ts.
- Rewrite only "Next walks to the end card through a block and its details, and Prev walks back": each step is
  `const t = await api.stageClick("next"); await shows(row[0], api.stageDrawn, { tour, mode: "step", step, kind,
  position, title, lit, thin }, { after: t, also })` where `also` gives the core's place and the page's errors
  (as stageShows does today). The end card and the way back the same. No `stageShows`, no `last()` in it.
- Before the walk: assert the newest stageDrawn has `reducedMotion: true`. If it is false, the flag does not
  reach a webview: do not work around it; write that in the commit body and in the report.
- The other tests keep the old helpers. Update the Api interface's stageClick/stageKey to return Promise<number>.

Checks between commits: `pnpm --filter vnoc typecheck`, `pnpm --filter vnoc-webview typecheck`,
`pnpm --filter vnoc-webview test`, `pnpm --filter vnoc unit`.
After commit 2, once: `pnpm -r build`, then `VNOC_SUITE=tour pnpm --filter vnoc test` once (the whole tour suite:
the old helpers must still work). Then the proof, once, 10 runs with every CPU busy:
  for i in $(seq $(sysctl -n hw.ncpu)); do (while :; do :; done) & done
  for i in 1 2 3 4 5 6 7 8 9 10; do VNOC_RETRIES=0 VNOC_SUITE=tour VNOC_GREP="Next walks to the end card" pnpm --filter vnoc test || echo "RUN $i FAILED"; done
  kill $(jobs -p)
Also once, to see that the test can fail for the right reason: temporarily change the expected lit lines of
s1.b3 to [11, 12, 13] and run it; the message must show the expected and the drawn `lit`. Revert.

Messages:
 1. `tests T1a: an action on the stage returns a ticket, and a drawing says the last action it saw`
 2. `tests T1a: Next and Prev wait for the whole step their click asked for`
Stage explicit paths only (never `git add .`, never `git add docs`). Do not push.
Report: the two hashes; reducedMotion true or false; the 10 runs (how many green, the time of one); the
failure message of the planted fault; anything in the step file that did not match the code.
```
