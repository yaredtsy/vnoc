# 28.01 — The tests we have, sorted and judged

Read at 739a9c7. The numbers per test are in [02-every-test.md](02-every-test.md) (made by a script). This file is the judgement.

## 1. The finding that changes the plan

**The sleeps are not what failed on CI.** Nine window tests failed on the same commit in the v0.4.0 runs. Eight of them have no `sleep(` in their body.

| Test that failed on CI | sleeps in it | What went wrong |
|---|---|---|
| Next walks to the end card (`at s1.b3`) | 0 | waited for the step's id, then checked the lit lines: they were not drawn yet |
| a canvas 520 px wide | 0 | waited for `folded`, then checked the height: the fold was halfway (268 px, not 30) |
| G9 the grip at 70% | 0 | waited for "taller than before", then checked "exactly at the limit" |
| TOURS folds and unfolds | 0 | the same shape: waits for one field, checks others (likely; no log kept) |
| a gone stop has no Now | 0 | the same shape (likely; no log kept) |
| Write IDs on a clean tour | 0 | wrote files and ran commands, then asked the core before it had read them |
| a core that starts again (Windows, 2 of 2) | 0 | a request lost at the restart: maybe a product bug |
| sidebar: a long list scrolled into view | 0 | a real 30 s timeout: the page never scrolled |
| Main ⇄ tour keeps both viewports | 1 | a 600 ms guess, and a window smaller than the test assumes |

So removing every sleep would not have made these runs green. The main fault is **a wait that is weaker than the check after it**: the test waits for one sign of the new state, then asserts five other things about a drawing that may be half made. The sleeps are a second, smaller problem, and most of them hide a different one (§4).

## 2. The tree

Read it top to bottom. Each level asks one question. The leaves are verdicts.

```
every test in the repo
│
├─ Rust: cargo test (core, drivers, search)
│    Does the same input give the same output?  yes
│    └─ GOOD. Keep. The three real failures here (git identity, an id by chance, `*` on Windows)
│       were fixed in TR24w and stayed fixed.
│
├─ TS unit: node --test (shared 17 files, webview about 68, extension 12)
│    Pure functions, no window, no clock.
│    └─ GOOD. Keep. This is where logic belongs. Seconds to run.
│
└─ Window: a real VS Code + host + core + page  (13 files, 208 tests, 6,645 lines)
     │
     │  LEVEL 1: what does the test prove?
     │
     ├─ A. The wiring: a command, a file, git, the core's answer        about 95 tests
     │    │  Needs the real window. Stays here.
     │    │
     │    │  LEVEL 2: how does it know the step is done?
     │    │
     │    ├─ A1. It awaits the thing itself (a command, a request)        GOOD, keep as is
     │    │       skeleton 6 · cli 7 · bend 6 · refactor 6 · bar 3
     │    │       edit: the host's save rows (about 24 of 33)
     │    │
     │    ├─ A2. It changes a file or runs `vnoc`/`git`, then polls the    FIX THE WAIT
     │    │      core until it has caught up                               (→ `settled`, design §3)
     │    │       attention 9 · calls (setup) · sidebar (setup)
     │    │       tour: pin and IDs 9 · rename/share/delete 12 · JSON 7 · no longer matches 5
     │    │
     │    └─ A3. It restarts or holds the core on purpose                  KEEP, with one fix each
     │            waits 2 (own window, good design; one wall-clock assert)
     │            tour "a core that starts again" (T0) · calls "a core restart"
     │
     ├─ B. What the page draws: layout, folds, rows, keys, words        about 90 tests
     │    │  Does not need VS Code or the core. Belongs in the Page layer.
     │    │
     │    │  LEVEL 2: how does it know the drawing is the right one?
     │    │
     │    ├─ B1. It marks "before", acts, and reads only drawings after    GOOD IDEA, done by hand
     │    │      the mark  (`openAtCover`, edit's `shownAfter`,            in 3 places → one helper
     │    │      search's `drawn(from)`)
     │    │
     │    ├─ B2. It waits on the newest drawing for ONE field, then        WEAK: this is what
     │    │      asserts others  (`stageShows`, `sectionShows`,            failed on CI
     │    │      `barShows`, `last()`)
     │    │       tour: on the open tour 11 · TOURS section 8 · map 5 · bar 6 · the screen 4
     │    │       sidebar about 8 · search about 6 · calls about 6
     │    │
     │    └─ B3. It depends on the size of the runner's window             MOVE TO PAGE
     │            tour: 520 px, the chip row, Main ⇄ tour viewports        (two are skipped on CI today)
     │
     ├─ C. Time: "nothing happens", "after 6 seconds", "told apart by the second"
     │    │  52 of the 87 sleeps live here.
     │    │
     │    ├─ C1. Sleep, then assert nothing happened (28 sleeps)           FALSE GREEN
     │    │       a slow runner makes these pass more easily, not less
     │    ├─ C2. A product timer (6 s undo, "moved by the agent")          NEEDS A CLOCK the test owns
     │    ├─ C3. Tour ids made from the clock's second (20 sleeps)         FIX THE CAUSE, not the nap
     │    └─ C4. A speed claim on the wall clock (`took < 1000 ms`,        NOT A TEST: a measurement
     │            search `[measure]`)
     │
     └─ D. The resend loop: do it again every 500 ms until it sticks
            edit's card tests (14 loops)                                   HIDES A PRODUCT GAP
            "an event sent before the new page listens is lost"
```

The counts for A and B are my estimate from the titles and the code I read. T3 and T5 decide test by test.

## 3. Suite by suite

| Suite | Tests | Sleeps | Verdict |
|---|---|---|---|
| skeleton | 6 | 0 | **Good.** Each step is awaited. The model of a wiring test. |
| cli | 7 | 0 | **Good.** Every step is an awaited command; files are checked after. |
| bend | 6 | 0 | **Good.** |
| refactor | 6 | 0 | **Good.** Waits on the core's own answer after each rename. |
| bar | 3 | 0 | **Good, one risk:** it stops the core, and the suites after it "prove it came back". That is another suite's job done by order. |
| session | 5 | 2 | **Mostly good.** Two tests sleep to prove nothing happened (C1). |
| attention | 9 | 1 | **Good waits, not independent:** fails alone (`NEEDS: session`). One 1.5 s sleep to see "no crash". |
| calls | 16 | 4 | **Mixed.** Fails alone. A 2.2 s sleep races a 2 s hold: the gap is 200 ms on a slow runner. |
| sidebar | 16 | 6 | **Mixed.** Half is page drawing (rows, scroll, selection). The last test is three sleeps and nothing else. |
| search | 14 | 8 | **Mixed.** `drawn(from)` is right. "Typing fast" sleeps 1 s twice to see no later answer. `[measure]` is a benchmark inside a test run. |
| edit | 33 | 21 | **Two halves.** The save rows call the host and await its answer: good. The card tests are 14 resend loops (D): the worst pattern in the suite. |
| waits | 2 | 0 | **Good design:** slowness set by a variable, a window of its own. One assert on the wall clock (`took < 1000`) will fail on a slow runner. |
| tour | 85 | 45 | **The problem.** 40% of the lines, 8 of the 9 CI failures. Its suites share tours through file-level variables (`tour`, `second`, `pinnedId`), so they only pass in order. |

## 4. The 87 sleeps, by why they are there

| Kind | Count | Example | What it really is |
|---|---|---|---|
| Sleep, then assert nothing happened | 28 | `session:47` "time enough for a replay, if there were one" | No signal for "done and nothing changed". Passes when the product is broken but slow. |
| Tour ids by the clock's second | 20 | `tour:564` `sleep(1100)` | Two tours started in one second cannot be told apart. 22 s of every run. |
| Resend loop | 14 | `edit:129` reveal + showCode every 500 ms | The page can lose an event sent before it listens. The loop covers it, and can fire the action twice. |
| Guess before the next step | 12 | `tour:775` `sleep(600)` before `setView` | An action with no "done". These lose on a slow runner. |
| Nap inside a hand-made poll | 9 | `tour:237` until the structure holds `shop.py` | Fine in spirit. Six copies of one loop. |
| Debug | 2 | `VNOC_SLOW`, `VNOC_SHOT` | Not a test's wait. |
| A product timer | 1 | `tour:2065` `sleep(6500)` for the 6 s undo | Real time the test does not own. |
| A person's typing pace | 1 | `search:116` 120 ms between keys | A scripted clock, not a nap. |

Only the 12 guesses and the 14 loops can turn a good commit red. The 28 "nothing happened" sleeps do the opposite harm: they stay green when they should not.

## 5. Signs of a weak test plan

These are not about timing. They make a failure harder to trust or to read.

1. **One test, many claims.** Titles join three to five claims with `;`. "Ask for a Tour…" is 92 lines and 11 asserts. The first failure hides the rest, and one retry costs the whole chain.
2. **Tests that need the test before.** `NEEDS` in `index.ts`, the tour ids in file-level variables, `bar` leaving a restarted core for the next suite. `VNOC_GREP` on one test often fails for this reason alone.
3. **A click that is not a click.** `stageClick("next")` calls the page's handler. It never proves the Next button exists, is enabled, or can be reached. Only the layout report (`outside: []`) stands in for that.
4. **Asserts that may not run.** `if (parts.includes("pin")) { assert… }` (`tour:460`): the test passes when the pin line is missing.
5. **"No crash" by sleeping.** `sleep(500); assert webviewErrors() is empty` (5 places). An error after 501 ms is missed.
6. **Test code in the product.** `viewCommand` `stageClick`/`stageKey`/`canvasWidth` (it sets `body.style.width`), `stage/measure.ts`. Each is a path a user never takes.
7. **Deadlines counted in naps.** `until` adds 100 per loop, whatever the loop took. A 20 s deadline is 40 s on a busy runner. Harmless, but the failure message lies.
8. **A speed limit as a pass mark** (`waits`: under 1000 ms). A slow runner is not a bug.

## 6. What is already right (keep it)

- `until("what", …)` names what it waits for, and `stageShows` adds the core's place, the last drawing and the page's errors to the failure. Good messages.
- `core()` / `guarded()` / `soft()`: a core that stops answering is one failure in 10 s, not sixty.
- The tour file's `setup` puts the workspace back before every test.
- `VNOC_SUITE` and `VNOC_GREP` run a part of the suite.
- The `waits` window: slowness made on purpose by a variable, not hoped for.
- Three helpers already read "only drawings made after my action". The plan makes that the only way.
