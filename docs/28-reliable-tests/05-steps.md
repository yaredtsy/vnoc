# 28.05 — The steps

Each step is at most two commits by the Sonnet session, from a prompt written when the step starts. Each has its own file in [steps/](steps/). A step is done when its "done when" holds, not when its code is written.

```
T0  stop the reruns, and find out what Windows is saying       ← releases stop hurting here
│
T1  one action, end to end: Next on a tour                     ← the proof that the design works
│     T1a  Window: ticket, shows(), reduced motion
│     T1b  Page: the harness, the real Next button, the stale-draw case
│
T2  the world outside: settled, the clock, the reset
│
T3  the tour's drawing checks move to the Page layer
│
T4  the tour's window suite: every wait by the rules, no sleeps
│
T5  the other suites, worst first: edit, search, sidebar, calls, session, attention
│
T6  prove it: stress, the ledger, retries off, 10 green runs
```

| Step | What it gives | Sleeps left (of 87) | File |
|---|---|---|---|
| T0 | a failed test is tried again by itself and named; a Windows probe job | 87 | [T0](steps/T0-stop-the-reruns.md) |
| T1 | the ticket, `shows`, the page harness; Next proven in both layers | 87 | [T1](steps/T1-one-action.md) |
| T2 | `settled`, `pageReady`, the clock, `vnoc.test.reset` | about 58 (the 20 id sleeps and the 9 poll naps go) | [T2](steps/T2-the-world-outside.md) |
| T3 | about 40 tour tests in the Page layer; the 2 CI skips gone | about 54 | [T3](steps/T3-tour-drawing-to-page.md) |
| T4 | the tour window suite by the rules | about 42 | [T4](steps/T4-tour-window-suite.md) |
| T5 | every other suite by the rules; `NEEDS` gone | 0 | [T5](steps/T5-other-suites.md) |
| T6 | the proof | 0 | [T6](steps/T6-prove-it.md) |

## Rules for every step's prompt

- Between commits: typecheck, the touched packages' unit tests, and the one suite being worked on, once. A change outside `extension/src/test/` needs `pnpm --filter vnoc build` before the window suite (until T1a makes the test script build).
- The big run (all window suites, `cargo test`, clippy) once, after the last commit.
- A repeated run (×5) only where the step's goal is flakiness, and once.
- Stage explicit paths. Never `git add .` or `git add docs`.
- A gap in this plan goes into these files, not into a silent choice.
- A test that is moved is deleted where it was, in the same commit. Nothing is checked twice.

## What can stop after any step

T0 alone ends the manual reruns. T1 proves or disproves the design on one action before 200 tests depend on it. After T2 the helpers exist and new tests can follow [03-rules.md](03-rules.md) even if T3–T5 are spread over weeks.
