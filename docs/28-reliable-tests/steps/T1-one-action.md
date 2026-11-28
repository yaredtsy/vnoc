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
