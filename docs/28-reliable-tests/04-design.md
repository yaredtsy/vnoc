# 28.04 — The design: three layers and the pieces that make a wait certain

What is built, and why. The order is in [05-steps.md](05-steps.md). Items marked **(to verify)** are my reading of the code, not yet tried.

## 1. Three layers

| Layer | What runs | What it proves | Where |
|---|---|---|---|
| **Core** | Rust, `cargo test` | every answer, every file, pin, IDs | exists |
| **Unit** | pure TS, `node --test` | rules: order, labels, layout sums | exists |
| **Page** (new) | the built webview in headless Chromium, the host played by a script | what the page shows and does: folds, keys, rows, sizes, buttons | `webview/e2e/` |
| **Window** (slimmed) | VS Code + host + core + page | the wiring, one or two tests per feature | `extension/src/test/suite/` |

About 90 of the 208 window tests are page checks. They move. The window suite gets smaller and each test in it has fewer moving parts.

## 2. Window: every action has a "done", every wait is tied to it

**The ticket.** The host gives each test action (`stageClick`, `stageKey`, `toursClick`, `mapClick`, `dispatch`, `setSearch`, `viewCommand`…) a number `n` and sends it with the event. The page handles the action, and after its next committed drawing it posts `{ type: "handled", seq: n }`. Every report the page posts (`stageDrawn`, `toursDrawn`, `mapDrawn`, `tourBar`, `searchDrawn`, `editState`…) carries `seq`: the last action handled when it was drawn. The action's promise resolves with the ticket `n` on `handled`.

**`shows(what, expected, { after })`.** It looks only at reports with `seq >= after`. It passes when the newest of them holds every field of `expected` (deep equal on the fields named). At the deadline it fails with the difference. It listens for reports; it does not nap.

This is the other model's point, and the code agrees with it: "a drawing was made after my click" is too weak, because an unrelated redraw (a resize, a late answer to an older request) is also a drawing after the click. The ticket removes the old drawings. The whole expected state removes the unrelated ones. Both are needed.

**`pageReady`.** A page posts `pageReady` once its listeners are on. The host holds events for a page until then, in order. This removes the cause of the 14 resend loops (`edit`): "an event sent before the new page listens is lost". It is a product fix too: a user's first command to a new canvas can be lost the same way **(to verify)**.

## 3. `settled`: the core, the host and the pages have caught up

`await api.settled(opts?)` resolves when all of these hold:

1. **The core has read what the test changed.** `api.settled({ paths: [...] })` sends `vnoc/settled` with the paths. The core reads those paths now (it does not wait for the watcher), and answers when its index matches the disk for them and no build is running. With no paths it answers when its queue of changes is empty. A file event the OS has not delivered yet is why "the core is idle" alone is not enough: the test must name what it changed.
2. **The host waits on nothing:** no request to the core in flight, no event to a page queued.
3. **Every open page has drawn the host's latest state:** the host sends each page a `sync` with a number; the page answers after its next drawing. Messages are in order, so everything sent before the `sync` has been applied.

Uses: after a file write, a `vnoc`/`git` command, `restartCore()`, a reset; and before asserting that nothing changed (rule 4).

Helpers on top: `api.write(path, text)`, `api.run(["tour", "add", …])`, `api.git(…)` do the thing and then `settled` with the paths they touched. A bare `fs.writeFileSync` in a suite becomes a review fault.

## 4. The clock

- **Tour ids.** 20 sleeps exist because tours started in the same second cannot be told apart **(to verify in T2: read how `tour start` makes the id and how the list orders)**. If two agents can really start tours in one second, this is a product fault and the fix is in the core. If only the order of the list needs it, the test passes its own start time.
- **Product timers** (the 6 s undo, "moved by the agent", "no news for five minutes"). In the page: Playwright's `page.clock` moves time. In the host or the core: a variable shortens it for the test window, like `VNOC_HOLD_FIRST_BUILD_MS` today.
- **Deadlines.** One constant, times `VNOC_TEST_SLOWDOWN` (3 on CI). Measured on the clock.

## 5. Motion and size

- The test window starts with `--force-prefers-reduced-motion`; the page already makes every transition instant then (`webview/src/lib/motion.ts`, `style.css`) **(to verify that the flag reaches a VS Code webview; if not, the test API sets the same switch)**.
- Window tests assert no pixel sizes. Page tests set the viewport.

## 6. The Page layer

- **Runner:** `@playwright/test` in `webview/e2e/`, Chromium only. The repo already uses `playwright-core` in `tools/media`.
- **The host's stand-in:** an init script defines `acquireVsCodeApi()`. Requests from the page go to a script in the test, which answers from fixtures. Events are sent by the test. The VS Code theme's CSS variables come from a saved copy of the default dark theme.
- **Fixtures are recorded, not written by hand.** `VNOC_RECORD=<dir>` on a window run saves the requests, answers and events of a scenario. A page test replays them. They are typed by `shared/messages.ts`, so a changed message fails to compile. Re-recording is one command.
- **Real input.** The test clicks the real Next button and presses real keys. `expect(locator)` retries until the deadline. The `viewCommand` test paths in the product can then be deleted where no window test needs them.
- **Held answers.** `host.hold("vnoc/tour")` keeps an answer back; `host.release()` sends it. This is how a race is tested on purpose (below).
- **CI:** Ubuntu only. A page does not depend on the OS. Seconds, not minutes.

**The risk:** a fixture that no longer matches what the core sends. Three guards: the shared types, the one-command re-record, and the window test each feature keeps.

## 7. Each suite on its own

- `vnoc.test.reset` (a test-only host command): closes tours, the canvas, dialogs and code tabs, puts the workspace back from a saved copy, drops the host's test logs, then `settled()`.
- Every suite calls it first and makes its own data. `NEEDS` goes. No variable is shared between suites.
- **The reset has its own test:** hold an answer in suite A, reset, start B, release A's answer. B's state must not change.

## 8. Tests of the tests

- **The stale-draw case** (Page): on step 1, hold the answer for step 2, click Next, cause an unrelated redraw (resize). The wait must still be waiting. Release the answer: it passes. This proves the fix for the failure we had, where ten green runs only suggest it.
- **Stress:** `VNOC_STRESS=1` runs the window suite with every CPU busy. It makes CI's slowness on a Mac.
- **The ledger:** a CI job run by hand or by night runs the window suite 3 times per OS with retries off and lists each test that failed at least once.
- **The count:** `scripts/no-sleep.mjs` counts `sleep(` per suite file against a saved number. More is a failure; fewer lowers the number. (The repo has no linter; this is 30 lines.)

## 9. What one failure leaves behind

On a failed test: the last 30 reports of each page with their `seq`, the host's requests in flight, the core's log tail, the page's errors, and a screenshot. Printed, and on CI uploaded. A failure that passes on retry keeps its first attempt's output.

## 10. Notes from the other model: what I took

| Its point | Verdict |
|---|---|
| "A draw after my click" is too weak; wait for the result of that click | **Taken.** It is §2, and it matches the CI failures ([01](01-review.md) §1). |
| Carry a request id or revision; a core "idle" signal leaves a gap | **Taken.** The ticket (§2) and the `sync` (§3.3). I add one gap it did not name: the OS's file event, so `settled` takes the paths (§3.1). |
| Layout checks to Page; one Window test that the real sidebar answers | **Taken.** |
| Sort the sleeps by why they exist; a controlled clock for timers | **Taken**, with the real sort in [01](01-review.md) §4. The largest group is one it did not name: sleep, then assert nothing happened. |
| Test the reset; add a case that must keep waiting | **Taken** (§7, §8). |
| `retries: 1` for now, off suite by suite | **Taken** as T0. Today CI has no retry at all: the whole 45-minute job is run again by hand. |
| Stress job nightly and by hand; print the seed | **Taken**, without the seed: nothing in the suite is random. |
| T0 on a Windows CI runner, repeated in one job | **Taken.** No Windows machine is needed. |
| T0 first, then one tour action through T1 and T2 together | **Taken.** T1 is Next, in both layers. |
