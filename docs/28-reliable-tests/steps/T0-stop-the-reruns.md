# T0 — Stop the reruns, and find out what Windows is saying

Status: T0a `dcef5d3`, T0b `2aec1ca` (2026-11-28, local). Review found four fixes: T0a2 below. T0c waits for `probe.yml` on `main`.

## Goal
A release is no longer blocked by a flake, and no flake is hidden. The one failure that repeats (Windows, "a core that starts again") is understood.

## Why first
Today one flaky test fails the 45-minute job, and the whole workflow is run again by hand, two or three times. T1–T6 take weeks. This takes a day.

## What changes

**T0a — a failed window test is tried once more, and named (CI only).**
- `extension/src/test/suite/index.ts`: `retries: 1` when `CI` is set, 0 locally.
- The runner's `retry` event gives the test and its first error. Collect them. At the end print `FLAKY (passed on a later try):` with each title and its first error, and write the same to `$GITHUB_STEP_SUMMARY`.
- On a first failure, before the retry: print the core's log tail (`api.logTail()`), the page's errors, and the last reports of each page.
- Check that `guardTheCore` still works with a retry: a core that stopped answering must still be one failure, not a retry of every test.

**T0b — a probe job.**
- `.github/workflows/probe.yml`, started by hand (`workflow_dispatch`): inputs `os`, `suite`, `grep`, `times`.
- It builds as the `test` job does, then runs `VNOC_SUITE=<suite> VNOC_GREP=<grep> pnpm --filter vnoc test` `times` times with retries off. It does not stop at the first failure. It prints a count, and uploads each run's output.

**T0c — the Windows restart test** (after T0b is on a pushed branch).
- Run the probe: `os=windows-latest`, `suite=tour`, `grep="a core that starts again"`, `times=5`.
- Read the timeline around `Pending response rejected since connection got disposed` and the `ENOENT` on `crlf.md`.
- Decide, in this file: **product** (a request made during a restart is lost: the host must wait for the new core, or ask again) or **test** (it asks before `restartCore()` has a core that answers). A product fix goes into a 0.4.x patch.

## Done when
- A CI run where one test failed once is green, and its summary names that test.
- The probe has run 5 times on Windows, and this file says which of the two it is, with the lines that show it.

## Not in this step
No test is rewritten. The retry is a bridge: T6 takes it out.

## The honest cost
A retry can hide a real race in the product. That is why each retried test is printed, and why the ledger (T6) counts them. A test on the list twice in a week is a bug to fix, not noise.

---

## Prompt for the Sonnet session (T0a + T0b, one commit each)

```
You are working in /Users/yared/Documents/Programing/rust/vnoc on branch `reliable-tests`.
Read first: docs/28-reliable-tests/README.md, docs/28-reliable-tests/steps/T0-stop-the-reruns.md,
extension/src/test/suite/index.ts, extension/src/test/suite/util.ts, extension/src/test/runTest.ts,
.github/workflows/ci.yml (the `test` job), .github/actions/setup/action.yml.

Do T0a and T0b of the step file. Do not rewrite or move any test.

Commit 1 (T0a), files: extension/src/test/suite/index.ts (and util.ts only if needed).
- Mocha `retries`: 1 when process.env.CI is set and VNOC_RETRIES is unset; VNOC_RETRIES=<n> sets it anywhere (0 turns it off).
- `mocha.run()` returns the runner. On its "retry" event (test, err) remember the full title and err.message,
  and print at once, for that test: the first error, the core's log tail (the extension's test api has `logTail()`),
  `webviewErrors()`, and the last 5 entries of `stageDrawn()`, `toursDrawn()` and `sidebarPosts()` where the api has them.
- At the end of the run print a block `FLAKY (passed on a later try): <n>` with one line per test and its first error.
  When GITHUB_STEP_SUMMARY is set, append the same block to that file as Markdown.
- A test that fails every try is a failure as today.
- guardTheCore: a test failed at once because the core stopped answering must not be retried into a 3 s ping each time
  in a way that changes today's behaviour: one line per test, fast. Read the code, decide, and say in the commit body what you did.
- Check: `VNOC_SUITE=skeleton pnpm --filter vnoc test` is green. Then prove the retry: temporarily make one skeleton test
  fail on its first run only (a counter in the file), run with VNOC_RETRIES=1, see the FLAKY block, and remove the change.
Message: `tests T0a: on CI a failed window test is tried once more, and every such test is named`

Commit 2 (T0b), file: .github/workflows/probe.yml (new).
- `workflow_dispatch` with inputs: os (choice: windows-latest, macos-latest, ubuntu-22.04), suite (string), grep (string, may be empty),
  times (number, default 5).
- Same setup and build steps as ci.yml's `test` job up to the build (reuse ./.github/actions/setup; pin actions by the same SHAs;
  on ubuntu use `xvfb-run -a`). It does not need the Rust test steps, only what the window suite needs built
  (the core binary, the drivers, `pnpm -r build`). Read ci.yml to see what those are.
- Then a bash loop: `times` runs of `VNOC_RETRIES=0 VNOC_SUITE=… VNOC_GREP=… pnpm --filter vnoc test`, each run's output
  to `probe-<i>.log` (and to the console), never stopping at a failure. At the end print `passed X of N` and exit 1 if X < N.
- Upload the logs with actions/upload-artifact (pin by SHA, as the repo's other actions are), `if: always()`.
Message: `tests T0b: a probe job runs one window suite or test several times on a chosen runner`

Rules: stage explicit paths only (never `git add .`, never `git add docs`). No big test run between the commits.
After commit 2, once: `pnpm --filter vnoc typecheck` and `VNOC_SUITE=skeleton,bar pnpm --filter vnoc test`.
Do not push. Report: the two commit hashes, the FLAKY block you saw in the proof, and anything in the step file that did not match the code.
```

---

## Review of T0a and T0b (2026-11-28)

Good: retries only on CI or by `VNOC_RETRIES`; a stopped core is never retried (`retries(0)` in the guard); the FLAKY block goes to the console and the step summary; the probe never stops at a failure and uploads every log.

To fix (T0a2):
1. **The probe cannot run from a branch.** GitHub starts a `workflow_dispatch` workflow only when the file is on the default branch. `probe.yml` must reach `main` (with T0a, which helps releases at once). Then `gh workflow run probe.yml --ref reliable-tests -f os=windows-latest -f suite=tour -f grep="a core that starts again" -f times=5` runs the branch's copy.
2. **The diagnosis can describe the retry.** `diagnose` is not awaited and calls `activate()` first, so the retry can draw before the last 5 reports are read. Fix: get the api once in `run()` before `mocha.run`, and read synchronously in the `retry` handler.
3. **No cap.** A real break that fails 30 tests doubles to 60 timeouts and can pass the job's 45 min, losing the report. Fix: after 5 retried tests in one run, the guard gives every later test `retries(0)`, and the summary says `retries stopped after 5: this is a break, not a flake`.
4. **`ELECTRON_RUN_AS_NODE`** (set when the suite is started from a terminal inside an Electron app) makes VS Code run the workspace as a script. `runTest.ts` deletes it, as `tools/media` does.

## Prompt for the Sonnet session (T0a2, one commit)

```
You are working in /Users/yared/Documents/Programing/rust/vnoc on branch `reliable-tests`.
Read first: docs/28-reliable-tests/steps/T0-stop-the-reruns.md (the review at its end),
extension/src/test/suite/index.ts, extension/src/test/runTest.ts.

One commit, files: extension/src/test/suite/index.ts, extension/src/test/runTest.ts.
1. index.ts: in run(), before mocha.run, `const api = await vscode.extensions.getExtension("vnoc.vnoc")!.activate()`
   (run() becomes async; keep its returned promise the same). `diagnose(api, title, message)` is synchronous:
   it reads logTail, webviewErrors and the last 5 of stageDrawn/toursDrawn/sidebarPosts at the moment of the
   retry event, and prints them. No await anywhere in it.
2. index.ts: a cap. Count the tests retried in this run (a test counted once). When the count reaches 5,
   the beforeEach in guardTheCore gives every later test `test.retries(0)`. Print once, at that moment:
   `RETRIES STOPPED after 5 tests: this run is broken, not flaky`, and add the same line to the FLAKY block
   (console and GITHUB_STEP_SUMMARY). VNOC_RETRY_CAP=<n> changes the 5.
3. runTest.ts: `delete process.env.ELECTRON_RUN_AS_NODE` at the start of main(), with a one-line comment why
   (a terminal inside an Electron app sets it; VS Code then runs the workspace folder as a script).
Check: `pnpm --filter vnoc typecheck`; then prove the cap: temporarily make the 6 skeleton tests fail on their
first try (a counter), run `VNOC_RETRIES=1 VNOC_RETRY_CAP=2 VNOC_SUITE=skeleton pnpm --filter vnoc test`
from a terminal that has ELECTRON_RUN_AS_NODE=1 set (proves 3): 2 tests pass on the retry, 4 fail, and the line
is printed. Remove the temporary change. Then `VNOC_SUITE=skeleton,bar pnpm --filter vnoc test` once, green.
Message: `tests T0a2: the diagnosis is read at the failure, retries stop after 5 tests, the suite runs from an Electron terminal`
Stage explicit paths only (never `git add .`, never `git add docs`). Do not push.
Report: the hash, the output of the cap proof, anything that did not match.
```

## T0c, after T0a2

Push `reliable-tests`, open a PR with T0a, T0b, T0a2 and the docs, merge it (the retry bridge then helps every release). Run the probe on Windows from the branch (command in item 1 above). Read the five logs; write the verdict (product or test) here.
