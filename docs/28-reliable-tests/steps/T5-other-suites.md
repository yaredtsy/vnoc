# T5 — The other suites, worst first

## Goal
Every window suite follows the rules. `NEEDS` is gone. The sleep count is zero and stays zero.

## Order (one or two commits each)

| # | Suite | Sleeps | The work |
|---|---|---|---|
| 1 | edit | 21 | The 14 resend loops go: with `pageReady` (T2) an action is sent once. The card-state tests (about 9) move to Page. The save rows stay as they are: they are good. |
| 2 | search | 8 | Rows, the note, ↓, dimming → Page. "Typing fast" → Page with held answers: the last answer drawn is the last text's, whatever order they arrive in. `[measure]` leaves the suite (a script that prints numbers). |
| 3 | sidebar | 6 | Rows, scroll into view, selection → Page (a fixed viewport ends the "list has no height" wait). Window keeps: the view opens, a click selects and reveals, the menus' commands. |
| 4 | calls | 4 | The 2.2 s sleep against the 2 s hold → `settled()`. Chevrons, cards, chips → Page. |
| 5 | session, attention | 3 | `settled()` for "nothing happened". Both call the reset. |
| 6 | waits | 0 | `took < 1000 ms` becomes "the code was drawn before the calls' count answered" (an order, not a speed). |
| 7 | bar | 0 | It restarts the core itself at its end and waits `settled()`; no later suite proves it. |

## Done when
- `scripts/no-sleep.mjs` is 0 for every file, and a new `sleep(` fails CI.
- `NEEDS` is deleted from `index.ts`; every suite passes alone.
- Each suite, as it lands, runs on CI with retries off.
