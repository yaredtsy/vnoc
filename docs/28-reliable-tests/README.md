# 28 — Tests that give the same answer every time

Status: first draft, 2026-11-28, written after v0.4.0's CI. Open questions for the user are in §7.

## 1. The promise

**A red run means a real bug, and the same commit gives the same result on every run.** No "run it again", no retries hiding a race.

When it is done:
- 10 CI runs in a row on one commit are green on Ubuntu, macOS and Windows, with retries off;
- the stress mode (§5.6) passes 5 times in a row on the user's Mac;
- `sleep(` appears nowhere in `extension/src/test/suite/` (a lint rule keeps it so);
- the CI-only skips of TR24x are gone.

## 2. What happened (the evidence)

PR #26 (tours) went red six times. The Rust failures were real and deterministic: the runners have no git identity, an id test relied on chance, and Windows refuses `*` in file names. TR24w fixed them, and they stayed fixed. The window suite (`pnpm --filter vnoc test`, a real VS Code with the real core) is different:

| Run / attempt (commit) | macOS | Windows | Ubuntu |
|---|---|---|---|
| 46139596300 (c2c9d0a) | 520 px; Main ⇄ tour viewports | 520 px; Main ⇄ tour; Commit and Pin | 520 px wide case; Commit and Pin |
| 46144054511 #1 (2393ee6) | sidebar: long list scrolled into view | TOURS folds; the bar's before-each; gone stop has no Now; **a core that starts again**; Write IDs | green |
| 46144054511 #3 (same commit) | Next walks to the end card (`at s1.b3`) | **a core that starts again**; G9 the grip at 70% | green |

Commit and Pin was a real fault (the fixture repo had no saved identity, fixed in TR24x). Everything else moves from run to run on the same commit. Only **"a core that starts again"** repeats: Windows, twice out of twice, with `Pending response rejected since connection got disposed` and an `ENOENT` copying `crlf.md` into VS Code's History just before it. That one may be a product bug (§6, T0).

## 3. Why the window suite flakes

The suite already waits on what the page reports (`stageDrawn`, `toursDrawn`, …) through `until(what, check, ms)`; the `ms` is only the deadline for giving up. So "use listeners, not timeouts" is half done. The flakes come from what those waits cannot tell:

1. **An action has no "done".** `stageClick`, `stageKey`, `toursClick`, `dispatch`, a file written to disk: each returns at once. The test's next step races the page, the host and the core. Example: Write IDs right after `fresh_one`/`fresh_two` are written; the core has not read them yet ("2 stops are not found in the code now").
2. **A wait can match the old state, or a state halfway through a change.** `last()` is the latest drawing, which may be from before the action. The 520 px failure drew `folded: true` with the unfolded height (268) mid-transition.
3. **Guesses.** 87 `sleep(` calls (tour 45, edit 21, search 8, sidebar 6, calls 4, session 2, attention 1). Each one is a bet on how long something takes. A slow runner loses the bet.
4. **Deadlines are fixed.** Usually 10 s or 20 s, tuned on a fast Mac. Also, `until` counts its 100 ms naps, not the clock.
5. **One window, shared state.** Twelve suites run in one VS Code window, one after the other, on one workspace. `NEEDS` (attention and calls need `session` first) shows that suites lean on each other. A test cleans up with `git reset --hard` and hopes the core and the page have caught up.
6. **The window is the wrong place for most checks.** About 6,500 lines of window tests, and much of it checks what the page draws (sizes, folds, which buttons, wrapping). Those checks need a real page, not a real VS Code plus a real core plus a real file system. Every extra moving part is another race.

## 4. The rethink: each check where it is cheapest and certain

Three layers. Each check moves to the lowest one that can prove it.

| Layer | What runs | What it proves | Speed and certainty |
|---|---|---|---|
| **Core** (Rust, `cargo test`) | the core, drivers, git | every answer, every file on disk, pin, IDs, the index | exists today; deterministic |
| **Page** (new: §5.4) | the real webview bundle in headless Chromium (Playwright), fed host messages from a script; no VS Code, no core | layout, folding, keys, what each drawing shows, at exact sizes, with motion off | seconds; same result every time |
| **Window** (`pnpm --filter vnoc test`, slimmed) | VS Code + host + core + page | the wiring: commands, a message really goes host → core → page and back, files, git, restarts | minutes; deterministic once §5.1–5.3 are in |

The window suite keeps one or two tests per feature that prove the pieces talk to each other. The many "is it drawn right" checks move to the page layer, where the size of the canvas is set exactly and nothing else is running.

## 5. The pieces

### 5.1 Every action says when it is done
Each test-API action (`stageClick`, `stageKey`, `toursClick`, `mapClick`, `dispatch`, `pinDialogClick`, `askDialogClick`, `canvasWidth`, …) gets a sequence number from the host. The page echoes the number in the first drawing made after it has handled the action (`drawnAfter: n`). The action returns a promise that resolves with that drawing. Tests write `const d = await api.stageKey("Enter")`, then check `d`. A drawing from before the action can no longer match.

### 5.2 `settled()`: nothing is still moving
One call, `await api.settled()`, resolves when all of these hold:
- the core has read every file change it has been told of (its index revision is the latest), and no request is in flight;
- the host has no request waiting on the core;
- every open page has drawn the host's latest state.

The core side is a small request (`vnoc/settled`) that answers once its index has caught up. A test that writes a file, runs a CLI command, restarts the core or resets git awaits `settled()` before its next step.

### 5.3 Motion off, deadlines that scale, no sleeps
- The test window starts with Electron's `--force-prefers-reduced-motion`. `webview/src/lib/motion.ts` and `style.css` already make every transition instant then, so a fold is drawn at its final size. Tests that are about motion turn it back on for themselves.
- One deadline constant, multiplied by `VNOC_TEST_SLOWDOWN` (3 on CI, 1 locally). `until` measures the clock, not its naps.
- `sleep(` is banned in `suite/` by a lint rule. Each of the 87 is replaced by the event it was guessing at. One kind is legitimate: "the tours are told apart by the second they were started in" (`tour.test.ts`, 1.1 s); it becomes a test-only clock or id hook, not a nap.

### 5.4 The page layer
- A small harness in `tools/` or `webview/`: it serves the built webview bundle, stands in for the host (`acquireVsCodeApi` and the message channel), and plays a script of host messages. The messages come from fixtures, and the core's own goldens can feed them, so they match what the core really sends.
- Playwright drives Chromium at exact viewport sizes (520, 640, 1024 px…). The repo already uses Playwright for screenshots (`tools/media`).
- The same `drawnAfter` echo as §5.1, so a page test waits on facts, not time.
- It runs in CI on Ubuntu only: the page does not depend on the OS. The window suite stays on all three.

### 5.5 Each test starts from a known state
- A host test command, `vnoc.test.reset`, closes the tours, the canvas and the dialogs, restores the workspace from a saved copy (not `git reset` alone), waits for `settled()`, and returns.
- Every window suite passes alone (`NEEDS` goes away). A test that needs the STRUCTURE page opens it itself.

### 5.6 Find flakes before CI does
- `VNOC_STRESS=1`: runs the window suite with every CPU core busy (a busy-loop child per core) and `VNOC_TEST_SLOWDOWN=1`. This reproduces a slow runner on the user's Mac.
- A scheduled CI job (nightly, or by hand) runs the window suite 3× per OS with retries off and lists every test that failed at least once: the flake ledger. A test on the ledger gets fixed or deleted, never retried.

## 6. Steps (≤2 commits each, as for tours)

- **T0 — the Windows restart test**, before anything else: "a core that starts again: … answered from the record". On Windows: is it a lost request after a restart (product: the host must re-ask the new core) or a test that asks before the new core is up (test: await the restart)? Needs a Windows run. The fix goes in a 0.4.x patch if it is a product bug.
- **T1 — the protocol:** `drawnAfter` echoes and promise-returning actions (§5.1), `vnoc/settled` and `api.settled()` (§5.2), reduced motion, scaled deadlines and a clock-based `until` (§5.3), the lint rule (it starts as a warning listing all 87).
- **T2 — the page harness** (§5.4), with the tour stage's layout tests moved into it first (the 520 px test, the mini bar, the fold, G9 the grip): the tests that flaked most.
- **T3 — the tour window suite:** every action awaited, `settled()` after every disk/CLI/git step, its sleeps gone, `vnoc.test.reset` (§5.5); the drawing checks that moved in T2 deleted from it.
- **T4 — the other suites** in the same way (edit 21 sleeps, search, sidebar, calls…); `NEEDS` removed; the lint rule becomes an error.
- **T5 — prove it:** the stress mode and the scheduled flake job (§5.6); remove the TR24x CI skips; 10 green CI runs in a row (§1).

## 7. Open questions for the user

1. Moving the "drawn right" checks out of the VS Code window into a page harness (§4, §5.4): agreed? It is the biggest change, and the biggest win.
2. The scheduled flake job (§5.6) costs CI minutes, about 3 window runs × 3 OSes per night. Nightly, weekly, or only by hand?
3. Until T5, keep a CI-only `retries: 1` that prints every retried test, so releases are not blocked meanwhile? (Retries off is the goal, not the bridge.)
4. T0 needs a Windows machine, or debugging through CI runs (slow). Is there a Windows machine or VM to use?
