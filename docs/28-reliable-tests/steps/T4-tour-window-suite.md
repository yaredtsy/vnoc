# T4 — The tour's window suite, by the rules

## Goal
The about 45 tour tests that stay in the window follow [03-rules.md](../03-rules.md). No sleep is left in `tour.test.ts`.

## What changes
- Every action awaited, every wait `shows(…, { after })` or `settled()`. `stageShows`, `sectionShows`, `barShows`, `last()` are deleted.
- Every file write, `tourCmd` and `git` through `api.write` / `api.run` / `api.git`.
- The 9 "sleep, then nothing happened" become `settled()`, then the assert.
- The 6.5 s undo wait: the timer is shortened by a variable for the test window, or the test moved to Page if the timer lives in the page.
- Each suite makes its own tours in its `suiteSetup` after `vnoc.test.reset`. The file-level `tour`, `second`, `pinnedId` go.
- Tests with several claims are split where a claim can fail by itself.
- The ticket and `seq` reach the other reports the suite reads (`toursDrawn`, `mapDrawn`, `tourBar`, the dialogs).

## Done when
- `scripts/no-sleep.mjs`: `tour.test.ts` is 0.
- Each of its suites passes alone by `VNOC_GREP`.
- `VNOC_SUITE=tour` is green 5 times in a row under stress, retries off.
- On CI the tour suite runs with retries off (the first suite to leave the bridge).
