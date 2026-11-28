# 28 — Tests that give the same answer every time

Status: second draft, 2026-11-28. The first draft was one file; this is the plan as a tree, one file per branch. Nothing is built yet. T0 is next.

## The promise

**A red run means a real bug, and the same commit gives the same result on every run.** No "run it again".

Done when:
- 10 CI runs in a row on one commit are green on Ubuntu, macOS and Windows, with retries off;
- the stress mode passes 5 times in a row on the Mac;
- `sleep(` is nowhere in `extension/src/test/suite/`, and a check keeps it so;
- the two tests skipped on CI (TR24x) run again.

## The tree

```
the promise
│
├─ What do we have?                      01-review.md       every test sorted: good, weak, misplaced
│    └─ the numbers per test             02-every-test.md   208 window tests, counted by a script
│
├─ How must a test be written?           03-rules.md        pick the layer; seven rules; what is banned
│
├─ What do we build to make that possible?   04-design.md
│    ├─ three layers: Unit · Page (new) · Window (smaller)
│    ├─ Window: the ticket, shows(), pageReady
│    ├─ settled(): the core, the host and the pages have caught up
│    ├─ the clock, motion, size
│    ├─ the Page layer: real page, scripted host, recorded fixtures
│    ├─ each suite on its own: the reset
│    └─ tests of the tests; what a failure leaves behind
│
└─ In what order?                        05-steps.md
     ├─ T0  stop the reruns; the Windows probe      steps/T0-stop-the-reruns.md   (prompt inside)
     ├─ T1  one action end to end: Next             steps/T1-one-action.md
     ├─ T2  settled, the clock, the reset           steps/T2-the-world-outside.md
     ├─ T3  the tour's drawing checks → Page        steps/T3-tour-drawing-to-page.md
     ├─ T4  the tour's window suite by the rules    steps/T4-tour-window-suite.md
     ├─ T5  the other suites                        steps/T5-other-suites.md
     └─ T6  prove it                                steps/T6-prove-it.md
```

A change is made at the highest file it belongs to, then pushed down.

## The three things to know

1. **The sleeps are not what failed.** Of the nine tests that failed on CI for v0.4.0, eight have no sleep. Five waited for one field of a drawing and then checked others, on a drawing half made. ([01](01-review.md) §1)
2. **The 87 sleeps are six different things.** 28 are "sleep, then assert nothing happened": they pass more easily on a slow runner, so they hide bugs and never flake. 20 exist because tour ids come from the clock's second. 14 are loops that resend an action because a new page can lose an event. Only 12 are plain guesses. ([01](01-review.md) §4)
3. **About 90 of the 208 window tests check what a page draws.** They do not need VS Code or the core. In a page-only layer they run in seconds at an exact size. ([04](04-design.md) §6)

## The evidence (CI, PR #26)

| Run / attempt (commit) | macOS | Windows | Ubuntu |
|---|---|---|---|
| 46139596300 (c2c9d0a) | 520 px; Main ⇄ tour viewports | 520 px; Main ⇄ tour; Commit and Pin | 520 px wide case; Commit and Pin |
| 46144054511 #1 (2393ee6) | sidebar: long list scrolled into view | TOURS folds; the bar's before-each; gone stop has no Now; **a core that starts again**; Write IDs | green |
| 46144054511 #3 (same commit) | Next walks to the end card (`at s1.b3`) | **a core that starts again**; G9 the grip at 70% | green |

Commit and Pin was a real fault (no git identity in the fixture repo; fixed in TR24x). Only "a core that starts again" repeats: Windows, twice of twice. It may be a product bug (T0).

## Decisions

The first draft asked four questions. These are my calls; say so if one is wrong and the files change.

| Question | Call | Why |
|---|---|---|
| Move the "drawn right" checks out of the window into a Page layer? | **Yes**, proven first on one action (T1). If the harness cannot play the page honestly, T1 says so and the plan shrinks. | The biggest win, and the biggest risk: so it is tested small. |
| How often does the flake-finding job run? | **By hand, and nightly when `main` changed.** Weekly after a month with nothing found. | It costs about 9 window runs a night. |
| A retry on CI until the end? | **Yes: one retry per test, CI only, each retried test named** (T0). Off suite by suite as each is fixed (T4, T5), off everywhere in T6. | Today there is no retry: the whole 45-minute job is run again by hand. |
| A Windows machine for T0? | **Not needed.** A probe job runs one test 5 times on a Windows runner and uploads the logs (T0b). | One push gives five tries. |

## Where things are

- The window suite: `extension/src/test/suite/` (13 files), run by `extension/src/test/runTest.ts`; `VNOC_SUITE`, `VNOC_GREP`.
- The page's reports the suite reads: `shared/messages.ts` (`Notes`), posted from `webview/src/`, kept in `extension/src/extension.ts`.
- CI: `.github/workflows/ci.yml`, job `test`.
- The Page layer will be `webview/e2e/`.
