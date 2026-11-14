# 16.10 Build order

```
build order
├── 1. rules for every step
├── 2. the steps (8.0 – 8.8)
├── 3. what each step leaves working
└── 4. later
```

On the branch `0.2-call-trees`, after 7.8. Each step is one commit (or a few with the same prefix) and leaves the branch green: `pnpm -r test`,
`cargo test`, and the window suite.

## 1. Rules for every step

- The **R** decisions marked **ask** in the README are answered before 8.1. If one is answered "no", the step that builds it says what changes.
- `tokens.css` stays the only file that names a `--vscode-*` variable: 09's colours are added there as tokens (`--row-selected`, `--edge-call`, …)
  and the components use the tokens.
- No `vscode` import in `shared/`; no state in a page that the session should own (10/01's rules, kept).
- **Steps go in order** (8.1 → 8.8; 8.2a beside them). A step done early leaves holes the later steps were meant to fill, as 8.4 did (its
  "as built" note).
- The window suite needs `ELECTRON_RUN_AS_NODE` unset: a shell started from an Electron app (VS Code's terminal, an agent's tool) inherits it,
  and VS Code then starts as Node and fails with *"Cannot find module …/ws"*. Run `env -u ELECTRON_RUN_AS_NODE pnpm -r test`.
- Each step updates the doc it builds when the code settles something the doc left open, as 14 did ("as built" notes), and adds its row to
  `../README.md`'s tree when the folder is done.

## 2. The steps

### 8.0 Three checks (no product change)

| Check | How | Write the answer into |
|---|---|---|
| does `webview/context` reach a **webview view**? | a throwaway webview view with one `data-vscode-context` element and one menu entry; F5; right-click | 08 §7 (and drop the fallback if yes) |
| is `focusedView == <id>` set for a webview view? | *Developer: Inspect Context Keys* with the throwaway view focused | 06 §10 |
| codicons in a webview under the panel's CSP | load `@vscode/codicons/dist/codicon.css` from `media/`, `font-src ${webview.cspSource}` | 04 §5 |

Proof: the three answers in the docs. Commit: `step 8.0: three checks for the webview sidebar (context menu, focusedView, codicons)`.

**Answers so far (2026-11-13).** Only the third check could run without a person:

| Check | Answer |
|---|---|
| codicons in a webview under the panel's CSP | **works.** The CSP already had `font-src ${webview.cspSource}`. What broke it was Vite's `base` (`/`): the CSS asked for `/assets/index.ttf`, which a webview resolves against its own root, a `NetworkError`. With `base: "./"` in `webview/vite.config.ts` the font loads in the real window (`document.fonts.load("16px codicon")` answered with the face, `check` true on the canvas page; measured with a throwaway probe, removed). The card uses codicons from step 8.3 part 1. |
| `webview/context` reaches a webview **view** | **not answered**: a right-click needs a person with F5; nothing in the window suite can open a context menu. Still open for 8.5, which builds the sidebar webview. |
| `focusedView == <id>` for a webview view | **not answered**: *Developer: Inspect Context Keys* is also manual. Still open for 8.5. |

### 8.1 The session: focus is a path, contexts, call ids

| Files | What |
|---|---|
| `shared/tree.ts` | `lineage(tree, id)` |
| `shared/calls.ts` (new) | `CallId`, `callId`, `parseCallId`, `isCallId`, `stepOf`; `siteText`, `listedSites`, `markBadge`, `isStale` moved here from `webview/src/canvas/calls.ts` |
| `shared/session.ts` | `ContextState.kind`, `.call`; `focus` = lineage; intents `explore`, `activate`, `closeContext`; `select` of a call opens / closes EXPLORE children (03 §4); `reduce(…, calls: CallLookup)`; the 0.1 history → lineage on the first `treeChanged`; `reveal` is canvas-only |
| `extension/src/session.ts` | passes the lookup (from `CallTrees`) to `reduce` |

Tests (`shared/session.test.ts`, `shared/calls.test.ts`, `shared/tree.test.ts`):

- focus on a deep node gives its lineage; Back walks up it; a crumb jumps; focusing the last step does nothing
- a saved 0.1 history `[a, x]` becomes `lineage(x)` on the first `treeChanged`
- select a call in MAIN → an EXPLORE child, not active, focus = the callee's lineage, selected = the callee; select another call → the first child
  closes, a second opens; select a non-call → the child closes; `explore` with `activate` → active; `closeContext` closes the children too and
  hands `active` back to the parent; an unknown callee opens nothing
- call ids: stable when a call is inserted above; `#2` for twin steps; JSON survives `::`, `#`, `>` and spaces in paths

Proof: `pnpm -r test`. The UI still runs (the canvas ignores the new fields). Commit: `step 8.1: the focus is a path; EXPLORE contexts; call ids`.

**As built (2026-11-13).** Everything in the table, plus decisions the doc left open:

| Decision | Why |
|---|---|
| **An EXPLORE child exists only for its parent's selected call** (`closeStale` runs after every intent that can move the selection: select, focus, Back, a crumb, reveal, `treeChanged`) | R4 as one rule instead of a close in each intent. `shared/sessionInvariants.ts` checks it after every test step |
| The id of an EXPLORE context is `explore:` + the call id: **one per call, wherever it was opened from**; a second request for it opens nothing | a stable id the sidebar and the bar can key on, and "no two EXPLORE contexts for one call" holds by construction |
| `focus` on a call id is `explore` with `activate`; `explore` on a callee the project does not know (or whose tree is not held) returns the same session | 03 §3 row 2; 03 §4 last row |
| `reveal` **keeps `canvas: boolean`** | the native sidebar still sends `canvas: false` (`vnoc.revealInSidebar`); the flag goes with the native sidebar in 8.5 |
| The 0.1 history becomes a path on the first `treeChanged` by one rule: the longest start that is a chain from the root (`chainedPrefix`); if the step after it exists but is not a child, it is a history and the lineage of the last existing step wins; if it is gone, the path was cut | one rule covers a 0.1 history, a deleted step and a renamed file |
| `restoreSession` accepts a context saved before EXPLORE (no `kind`, no `call`) as MAIN; an invalid EXPLORE (not a call id, parent missing or after it, a second one for the same call) is dropped **with MAIN and its focus kept**; no usable MAIN gives a fresh session | not losing MAIN for a broken panel. A call that was valid but is gone is closed by the first `treeChanged` (`callExists`: the keys inside the id against the structure; call trees are not saved, so they cannot be asked earlier) |
| `shared/calls.ts` also has `callNodesIn`, `resolveCall`, `callLookup`, `callExists` (8.2 listed `resolveCall`; `callsOf` and `wantedCalls` stay for 8.2) | the lookup `reduce` needs is `resolveCall` over the trees the host holds |
| `siteText`, `listedSites`, `markBadge`, `isStale`, `siteLabel` moved to `shared/calls.ts` with their tests; `webview/src/canvas/calls.ts` keeps `callRows` and `callLabel` | as planned |

Tests: 125 in `shared/` (was 31). The reducer tests are tables from 03 §3 and §4 and run `sessionProblems` after every step; the checker has its own test that each way a session can be wrong is noticed. The window suite is unchanged (35).

### 8.2 The host fetches what is open

| Files | What |
|---|---|
| `shared/calls.ts` | `callsOf`, `wantedCalls`, `resolveCall` |
| `shared/session.ts` | `callsChanged` (05 §5) |
| `shared/messages.ts` | event `callTreeState`; request `showCallTree` removed; `rendered` gains `calls` |
| `extension/src/callTree.ts` | asks for `wantedCalls` after each session or structure change, two at a time, the prefetch last; cancels what is no longer wanted; emits `callTreeState`; refetches stale wanted trees 800 ms after the last index change; `KEEP` counts only unwanted trees; progress on the view, no notification |
| `extension/src/commands.ts` | `vnoc.showCalls` (05 §7) replaces `vnoc.showCallTree` and `refreshCallTree` |

Tests: `shared/calls.test.ts` (`callsOf` through `cut`; `wantedCalls` with two contexts, a focus step, the prefetch; stale and unwanted is not
asked for); window suite `calls.test.ts`: `showCalls` on the fixture → the function in `expanded`, a tree arrives, `callTreeState` went
`loading` → `idle`; collapse before the answer → cancelled.

Proof: the tests; by hand, the old "Calls" section still fills (it reads the same trees). Commit: `step 8.2: the host fetches the calls of what is open; Show Calls`.

**As built (2026-11-13).** Everything in the table, and these decisions:

| Decision | Why |
|---|---|
| The scheduler is **`shared/callScheduler.ts`**, a plain class given `fetch(key, signal)` and a clock; `extension/src/callTree.ts` only gives it the core, `setTimeout`, the progress on the STRUCTURE view and the events | it is tested with a fake of each (13 tests) and no VS Code: two at a time with the prefetch last, a cancelled request's signal and its dropped late answer, out-of-order answers, failure and retry, one refetch 800 ms after the **last** of three index changes, KEEP over unwanted trees only, a restart in the middle of a fetch |
| **`CallLookup` is three-valued**: an object (the callee), `undefined` (the owner's tree is not held: unknown), `null` (the tree is held and the call is not in it: gone) | `callsChanged` must not close anything for a tree that failed to load. 8.1's code treated both falsy values alike, so nothing there changed |
| **`callsChanged { owner }`** is dispatched by the host when a tree arrives; it only looks at call ids of that owner | gone ids leave `expanded`, a gone selection falls back to the owner function, an EXPLORE whose call is gone closes with its children |
| **Core epochs.** `Core.epoch` (1, then one more after each restart, from the language client's state change), `StructureResult.epoch` (added by the `structure` handler) and `CallTreeResult.epoch`; `isStaleIn` is never stale across epochs; the new event **`callTreesReset`** makes every page drop its trees and states | a restarted core counts revisions from the start again: a tree of revision 40 against a structure of revision 3 looked fresh (or, the other way round, stale for ever). The scheduler also drops its trees and cancels its timer on `restarted()` |
| `wantedCalls` wants an **open class or a class that is the last focus step**, like a function | 05 §7 has Show Calls on a class, and its message is decided by the answer. Whether 8.3 draws a class's calls (R7 says they are under `__init__`) is 8.3's question |
| **Interim look**: a selected function is prefetched, so the old "Calls" section now appears under a selected, opened or focused function without Show Calls; its Refresh button is gone (it says *updating…*, *could not update* or *the code changed* while the host works) | 8.3 replaces the section with call cards. A failed fetch has `retry` in the scheduler but no button until the call card has a place for it |
| `rendered` carries `calls` (how many cards draw a Calls section); `callsDrawn` and the window suite's `waitCallsDrawn` are gone; the window suite reads the host's own `callTreeLog` | the host is the one that fetches, so the test asks it |
| `vnoc.showCallTree` is now `vnoc.showCalls` (**Show Calls**) and no longer waits for the answer; `refreshCallTree` and the `showCallTree` request are gone | cancelling by closing must be possible while the command is "done" |

Fixed in the same step: the docs tab (`DocsApp.tsx`) dropped answers by revision, so after a core restart it never updated again (now the newest ask speaks, as `state/data.ts` does); `extension.ts` creates `callTrees` before the `SessionStore` that reads them.

**8.2 part 2 (fixes, 2026-11-13).** (1) 05 §5 item 2b: a wanted key whose last fetch failed is retried once when the code settles (scheduler tests: fail → change → 800 ms → one fetch; fail again → stays failed; an unwanted key is not retried; a later settle tries once more). (2) `wantedCalls` wants a class only when it has no `__init__` child of its own (table rows: with, without, inherited); **`vnoc.showCalls` on a class that has its own `__init__` shows that `__init__`'s calls**, which supersedes the "class" row in the table above and 05 §7's class paragraph for that case. (3) The host sends `callTreeDropped`; the page store's logic is the pure `applyCallEvent` (`webview/src/state/callEvents.ts`: tree, state, dropped, reset, snapshot; table test), so the "a page empties its copy" gap below is covered for the reducer, and the store only calls it. Window suite: 38. A `snapshot` that was computed before a drop can still bring a dropped tree back for one page that asks at that moment; not handled.

Tests: `shared/` 177 (was 125): `callsOf` (12 rows, through a `cut` node), `wantedCalls` (17 rows), `isStaleIn`, the scheduler, `callsChanged` (7). Window suite 37 (was 35): Show Calls (loading → idle, a card draws it), close before the answer (no tree kept), refetch after a code change, a core restart with a function open. **Not tested**: that a page really empties its copy on `callTreesReset` (the webview stores have no test harness; the host side is covered).

### 8.2a The driver keeps the project open (Python driver)

Independent of the UI steps; it can go beside 8.1–8.3. From the analyzer's `docs/05-python-migration/11-driver-needs.md` §8.

| Files | What |
|---|---|
| `drivers/python/Cargo.toml` | the analyzer's `rev` moves past PY9.4 (`c8d18aa` or later; all three crates at one rev) |
| `drivers/python/src/call_tree.rs`, `main.rs` | **one worker thread** owns one `Project` per root (opened on first use, with the stack size of the analyzer's `ENGINE.md`); each `vnoc/callTree` calls `refresh()`, then the solve. Outline, `planIds`, `planFileId` stay on their own threads |
| the same | cancel: the request's flag becomes the analyzer's `Cancel` (`with_deadline(received + 20 s)`), so a queued or cancelled request stops; a request cancelled while queued is answered `-32800` without running |
| `crates/vnoc-driver-protocol` | `CallMark::Likely` (additive), mapped from `"likely"` instead of dropped |

Proof: 14 §11's measurement again on httpx, written there: the project's open, the first request, then 20 functions warm; the lifecycle
tests still pass. Commit: `step 8.2a: the Python driver keeps the project open; cancel stops the solve`.

**As built (2026-11-13).** Everything in the table, and:

| Decision | Why |
|---|---|
| **Pinned at `c989c0f`** (the tip of `origin/main` on GitHub today; `c8d18aa`, PY9.4, is 21 commits before it), all three crates; the only lockfile lines that changed are the analyzer's own source strings, no third-party version moved | one rev for the three crates; the driver built without any code change for the new rev |
| **The wire version at this rev is 2, not 3.** `analyzer_core::wire::WIRE_FORMAT_VERSION` is 2 at `c989c0f`, at `origin/main` and at the analyzer's local `main` (7 unpushed commits further on); the analyzer's notes (11-driver-needs §8 item 5, the TS5 doc §8) say it becomes 3 after TS5 step 2, which has not landed in the constant yet | the driver **checks** it (`WIRE_FORMATS_READ = [2, 3]`, one test, and a log line at start): any other number fails the test, which is a prompt to read what changed. Nothing is *reported*: the driver protocol has no field for it (`initialize` capabilities are booleans), and none was added |
| `CallMark::Likely` (additive, protocol crate) and `"likely"` mapped; the core's merge keeps it in the union; the page's `CallMark` has `likely` with a badge `≈ likely` (`shared/calls.ts`) | before, a guess looked like a certainty. The badge is there so the old section does not break on the new word; the call card's own look is 8.3 |
| One **worker thread** (`worker.rs`), **8 MiB stack** (the analyzer's ENGINE.md: a 256-level chain needs just over 2 MiB, an overflow aborts the process; a test runs a 300-level chain through the worker). `Projects` holds one `Project` per root, keyed by the root string as given (two spellings of one folder would be two projects), `refresh()` before each solve | the engine is not shared between threads; refreshing is what makes an edit, a new file or a deleted one visible |
| **Cancel is the analyzer's `Cancel`**, built when the request arrives with `with_deadline(received + 20 s)`: `$/cancelRequest` fires it, so a running solve stops and the tree it would have made is dropped (`-32800`); a request **cancelled while queued is answered `-32800` without being run** (the project is not even opened); the deadline counts the queue. `Limits::deadline` is no longer used | §8 item 2 |
| A **panic inside a request** is caught on the worker, answered `-32603` "the analysis crashed: …", that root's project is dropped (it may be half way through a change) and the worker goes on. For that to work `[profile.release] panic = "abort"` is **removed**: with it one bad request would end the whole driver | tested through the same wrapper a call tree runs in |
| **Test hooks** (`VNOC_TEST_HOOKS=1`; a driver in a user's window answers none): `vnoc/test/stats` (solves, opens, reopens, projects), `hold { ms }`, `panic { root? }`, `deadline { ms? }` | "the solver never ran", a queued request and a request past its deadline cannot be built any other way; they go through the worker's own queue, so they are ordered with the call trees |

Tests: `drivers/python/tests/kept_project.rs` (8): an edit, a deleted and a new file, an environment input (`pyproject.toml`) reopening, two roots, a cancel while queued (solver count 0), a request past its deadline (answered, `incomplete`, the next one whole), a panic, a deep chain; unit tests for the mark words and the wire version. The driver crate has 47 tests, the core's 74, the window suite still 38.

Proof: 14 §11.1 (release build, httpx). **Not done here:** `vnoc/environmentInputs` (§8 item 3, capability still `false`), and no memory measurement of a kept project. The debug build directory of the driver was deleted to make room for the release build (the disk had 2.6 GiB free); the next step starts with `cargo build` in `drivers/python`.

**8.2a part 2 (`python-v0.3.0`, 2026-11-14).** The analyzer's handoff for that tag asked for the six items of its `11-driver-needs.md` §8; items 1–3, 5 and 6 (the worker, `refresh`, cancel, `likely`, `depth` as `render_depth`) were already built above, so this part is the pin and the one item left.

| Decision | Why |
|---|---|
| **Pinned at `c1e1d7e`** (= the tag `python-v0.3.0`; the `rev`, not the tag, as before), all three crates. The driver built with no code change; the only lockfile lines that moved are the analyzer's own source strings, and the analyzer's `languages/python/Cargo.lock` is the same at `c989c0f` and `c1e1d7e` | `Options` and `Limits` were already built with `..Default::default()`; `Entry` is still the struct (the enum, D32, comes with a later tag and needs one line per call site) |
| What users see (the analyzer's numbers, not measured here): effects are on for every request (CS4b), so class-heavy code gets its calls (FastAPI `include_router` 458 → 1019 nodes); one known `incomplete` on FastAPI; some context digests moved, names / ids / sites did not | no driver code; the wire is still version 2 |
| **`vnoc/environmentInputs { root } → { files }`** (the shape of `plan_vs/07-drivers/01` §5; `EnvironmentInputsParams` / `EnvironmentInputsResult` in the protocol crate), capability `environmentInputs: true`. It goes through the **worker** (`Task::EnvironmentInputs`): the list is the kept project's, asked after its `refresh`, so a config written since the last request is already in it. It does not count as a solve | the list lives in the `Project`, which lives on that thread |
| The entries are the analyzer's, **passed as they are**: relative to the root, or absolute when the environment is outside it (kept: it is the virtual environment), the glob `requirements*.txt`, always `/` | the core compares strings and never rewrites an entry (the analyzer's §8 item 3) |

Tests: `drivers/python/tests/environment_inputs.rs` (6): the capability; the fixed entries, sorted; an environment under the root (`env/pyvenv.cfg`); one outside it (absolute, the exact path on Unix; on every OS no entry holds `\`, which CI's Windows job checks); an environment named in `ty.toml` after the first call tree is in the next list (one open, one reopen, one solve); a missing root is `-32603` and the next request is answered. The environment is named in `ty.toml` so that the test machine's `VIRTUAL_ENV` / `PATH` cannot choose it. The driver crate has 54 tests; the core's Rust tests and the window suite (46) pass.

**Not done here:** the core does not ask for the list yet (nothing hashes the inputs; that is the tree cache of 14 T6 / `04-storage/02-index.md`), and 14 §11.1's httpx measurement was not taken again at this rev (it needs a release build).

### 8.2b Exact chevrons: the probe (05 §3.1)

After 8.2 and 8.2a.

| Files | What |
|---|---|
| `crates/vnoc/src/protocol.rs` | `CallTreeParams` gains `depth: Option<u32>`, passed to the driver (which already maps it to `render_depth`, `drivers/python/src/call_tree.rs:36`); new `vnoc/hasCalls { key } → { revision, count }` (one key per request, 05 §3.1); every full `vnoc/callTree` the core sends the driver carries `depth: 32` (05 §3.1, the depth cap) |
| `crates/vnoc/src/calls.rs` | `has_calls`: the key's depth-1 tree through the **same** merge and filter as `vnoc/callTree`; `count` = the children left. Its marks are dropped |
| `shared/callScheduler.ts` | probes as the lowest kind of work: one in flight, started only while no open / cut / prefetch fetch is waiting; cancelled when the card is no longer drawn |
| `extension/src/callTree.ts` | probes the function and class cards drawn whose calls are unknown, the selected first; cancels what is no longer drawn; drops answers on a new revision |
| `shared/calls.ts` | `Calls` gains `{ state: "probed"; count: number }`: a chevron when `count > 0`, `+count` in the footer, *calls nothing* when 0. Opening a probed card fetches the full tree as before |

Tests: `calls.rs` (a function that calls only a library → `0`; two calls to one callee → `1`; the depth-1 tree's `Cut` never reaches the answer);
the core's lifecycle tests (a 70-function chain: the full tree comes back cut at 32 and parses; opening the cut node fetches the rest);
`shared/callScheduler.test.ts` (a click while three probes wait goes first; never two probes at once; a probe for a card no longer drawn is cancelled);
`shared/calls.test.ts` (probed → chevron and `+N`; pending → the empty slot). Proof: by hand on httpx, a file of 30 functions gets its chevrons
in well under a second, and none disappears on opening. Commit: `step 8.2b: exact chevrons from a depth-1 probe (vnoc/hasCalls)`.

**As built (2026-11-14).** Everything in the table, plus:

| Decision | Why |
|---|---|
| `vnoc/hasCalls { key } → { revision, count }`, one key per request; the core's `call_tree` and `has_calls` share `driver_tree(core, key, depth)`; `calls::count_calls` is the merge's children; `calls::DEPTH_CAP = 32` is the one constant every full tree carries | one path for both, so the count is exactly what opening shows |
| **The depth cap is proved by removing it**: with `depth: None` the 70-function chain test fails with *"the driver stopped: … recursion limit exceeded at line 1 column 7532"*. A parse error in a driver's answer used to be treated as the driver **dying** (the core dropped it); **8.2b part 2 fixed that**: a well-framed body that cannot be read (not JSON, too deep, the wrong shape) fails only its own request with *"the driver's answer could not be read: …"* (`HostError::BadAnswer`) and the driver is kept. Framing errors (no `Content-Length`, stray output, a closed pipe) still end it | the cap is not just a size bound |
| **Scheduler: probes are the lowest work**: one at a time, started only when no tree is in flight and none waits for a place; **a probe in flight is cancelled when a tree becomes wanted** (beyond the design: otherwise a click queues behind it on the driver's one worker) and asked again afterwards; a failed probe is not asked again until the code settles; a held probe for an old revision is asked again 800 ms after the last change while its card is drawn (the old count stays until then) and dropped when it is not; a restart drops them all | 13 + 9 scheduler tests (25 in all) |
| The **page tells the host what it draws**: note `cardsDrawn { keys }` (function and class cards with no structure children, the selected one first, then reading order). A page that opens later is sent the probes the host already holds for those keys | the layout lives in the page; the host cannot know |
| Page: `Calls` gains `probed`; `callsLook(calls, open)` is the one rule (unknown: empty slot; probed or known with calls: chevron, `+N` while closed; 0: *calls nothing*; loading / failed: the chevron stays); `Placed` gains `callsChevron`, `plus`, `callsNothing`, and `footerChips` counts `plus` (structure children **or** calls). A class **with methods** keeps its structure chevron: a probe does not change it | the function card's chevron opens through the same `toggle` as any card; the host then fetches the tree and the old "Calls" section draws it (call cards are 8.3 part 2) |
| `keys.ts`: `→` on an open card goes to its first child only when a card one level deeper is drawn right after it | a function open with its calls not drawn as cards used to select its next sibling |
| `core.ts`: from source it **never falls back to `bin/`**; `chooseBinary` (pure, `shared/binaryChoice.ts`, 8 rows) gives an error that says how to build (`cargo build` in `drivers/python`); the driver is checked too (it was not before) | the stale copy from 11 November made the window suite fail with "the driver has no call trees" |

Tests: core 76 unit + 33 lifecycle (`has_calls_counts_what_opening_would_show…`, `a_function_that_calls_only_library_code…`, `a_deep_chain_comes_back_cut_at_the_cap…`); `shared/` 214; webview 90; window suite 40 (a file's function cards get their chevrons from probes, `leaf` does not; opening a card with a chevron fetches its tree and a card draws it).

**Proof, honestly scoped.** Not by hand in a real window. Measured at the driver (debug build, httpx): the first 30 functions of a file as depth-1 probes, one after the other: `_client.py` 345 ms (first 147 ms, median 2.0), `_models.py` 200 ms, `_urls.py` 132 ms. A click is never behind more than the one probe in flight, and the scheduler cancels that one (tested with fakes, not timed end to end). Held probes are not bounded until they go stale.

### 8.3 Calls are cards; the new card

| Files | What |
|---|---|
| `webview/src/canvas/layout.ts` | call cards after nested defs (03 §1), `Placed` of 04 §6, 72 px cards, no Calls heights |
| `webview/src/canvas/Canvas.tsx` | dashed call edges (03 §6) |
| `webview/src/node-card/*` | 04 §1–§5: `NodeHeader`, `NodeMeta` (new), `ActionStrip` (new), `NodeFooter` (rewritten: the counts), `nodeContext` (08 §6); delete `NodeCalls.tsx`, `NodeBody.tsx`, `canvas/calls.ts`'s `callRows` |
| `webview/src/canvas/keys.ts` | `Enter` / `Ctrl+Enter` on a call (03 §7) |
| `webview/package.json`, `webview/src/node-card/kinds.ts` | codicons |

Tests: `layout.test.ts` (a function open with calls known → call cards after its defs, edges marked `call`; calls unknown → chevron, no `+N`;
`recursive` → no chevron; heights 72 / +220); `keys.test.ts`.

Proof: by hand on `minicart`: focus `checkout` → one call card; open it → six; open `reserve_stock` → its own. Commit:
`step 8.3: calls are cards on the canvas, one level at a time; the card is header and one meta line`.

**As built, part 1 (2026-11-13, before 8.1 and 8.2).** In: the structure card of 04 §1 only: 260 × 72, header / meta line / footer, the buttons
always shown (dim, full colour on the selected card), the footer counts, codicons (`kinds.ts`, `Codicon.tsx`), `NodeBody.tsx` deleted. Each rule is
a pure function with a table test beside its component: `cardButtons.ts` (04 §3), `footerChips.ts` (04 §1, "1 doc" / "2 docs"), `whereOf.ts`; the
components only draw. Also: `foldCrumbs.ts` and `app/pageBody.ts` (02 §4–§5) got the same treatment, and `warningOf` got two more cases.

| Left | When |
|---|---|
| call cards, dashed call edges, `Placed.role` / `key` / `call`, `childCount: number \| null`, `+N` counting calls, `calls nothing`, call-card buttons (`◇ Explore`) | after 8.1 (call ids, contexts) and 8.2 (the host fetches what is open) |
| `NodeCalls.tsx` and `canvas/calls.ts`'s `callRows` (the old "Calls" section, still drawn, 24 + 20 px rows added to the card) | **done in 8.3 part 2** |
| `symbol-method` for a function inside a class | **done in 8.3 part 2** (`Placed.inClass`, `kindIcon`) |
| `@tabler/icons-react` | still used by the docs panel, the docs tab and the kit's select and menu; goes when nothing uses it (not 8.3) |

**As built, part 2 (`6bd3413`, 2026-11-14, after 8.2b).** In: call cards, one level at a time, at `CARD_W` 300; the old "Calls" section is gone.

| Decision | Why |
|---|---|
| `Placed` has `role` (`node` / `call`), `key`, `parentId`, `inClass`, `call: CallInfo \| null` (count, sites, lines, callerName, callerPath, look), `siteRows`. `Edge.kind` is `node` or `call`; call edges are dashed (`--edge-call`) | a call card is a row of the layout like any other (03 §1); ids are `callId(owner, steps)` |
| `callsLook` of 8.2b stays; `childCount: number \| null` was **not** added | the three-valued answer is already in `callsChevron` / `plus` / `callsNothing`; a second field would say the same thing twice |
| `callLook(node, open)` is the one rule for chevron, `+N`, mark and unknown callee (05 §6); `MARK_ORDER` picks the one mark to show. The `cut` text is now "… calls not loaded" | one place, tested as a table |
| `callMeta(sites, parentPath, right, chars)`: up to 2 sites ("line 16", "base.py:12" when the site is in another file than the caller), then "N more ▾"; the right text (callee file, mark, or "not in the index") is shown only if `left + 2 + right <= metaChars()` (`rows.ts`: card width minus the name column, 6.6 px per monospace char) | the file name is whole or hidden, never cut. `PublicFilesResourceHandler`-style long names still cut in the **header**: the meta rule does not touch it |
| **×N** = the distinct call sites on the card, shown when N > 1 (8.4 part 5 changed this; the first rule was "only when the sites back the core's count") | the core's `call_count` is "calls reaching the node" (it multiplies down a merged parent), so it is not the places a call is written. First built as a rule that hid the badge when the two differed; that hid real information (count 4, two sites: the card should say ×2). The core is **not** changed; the doc comment of `call_count` now says what it counts |
| `rendered` is `{visible, hidden, callCards, chevrons}`; `hidden` counts structure nodes only | the window suite counts call cards apart from structure cards |
| request `pickCallSite`: several sites → a QuickPick (label `siteChoice`: "line 16" / "base.py:12" + the line's text), each item previews in the editor with `preserveFocus`; one site goes straight to `openAt` | 03 §7 |
| webview section `callUnknown` (08 §6) for an unknown callee; `nodeKey: key ?? id`, `role`, `calleeKnown`, `sites`, `callId` in `nodeContext` | menus (8.7) read them; a call card's Go to Code and Add Doc use the callee's `key`, not the card id |
| keys: `Enter` on a call = Explore (activates), on an unknown call nothing; `Ctrl/Cmd+Enter` = Go to Call Site (only sites that name a file); double-click on a call = Explore and activate, on an unknown call = select | `canvasChord` and `doubleClickIntent` are pure with tables |
| the card's `NodeFooter` had no case for the new `times` chip and drew nothing, silently | the switch now ends in `const unhandled: never = c.kind` |
| card heights: 72, +220 with code, + `siteRows × 18 + 8` when "N more ▾" is open; a class with no own `__init__` shows methods first, then its calls | `SITE_ROW_H`, `SITE_LIST_PAD` in `layout.ts` |

Tests: `shared` 217, `webview` 147, window suite 43 (new: call cards drawn exactly 2 after `collapseAll`; selecting a call card opens `explore:<id>` without activating it and an Explore dispatch activates it; a 40-deep chain cut at the depth cap, opened, fetches the callee's tree and draws more than 33 cards).

**Proof by hand, and its limits.** The four minicart steps were run on the real page (the real `reduce`, `wantedCalls`, `callLookup`) in a browser with a **mock host**, fed the **real core and driver's answers** for `/tmp/minicart` (probes: `checkout` 1, `checkout_order` 6, `reserve_stock` 2). It was **not** a real VS Code window. Counts of cards on the canvas: start 9; focus `checkout` 2 (the function and one call card); open `checkout_order` 8 (six call cards); open `reserve_stock` 10 (its own two: `_lookup`, `_log`). Nine dashed call edges at the end. `check_stock` ("line 19 · line 20 · 2 more ▾", four sites, ×4) opened its site list; the callee's file name is hidden there by the 36-character rule. Real clicks on the chevron toggled 8 ↔ 2 cards. The harness and its data are not kept. Checked by the user in a real window (2026-11-14): **Ctrl/Cmd+Enter and the QuickPick with preview work.** Still to check: the dim unknown callee.

**As built, part 3 (2026-11-14, a small step between 8.3 part 2 and 8.4 part 5).** In: the card's rows of 04 §1 as rewritten that day.

| Decision | Why |
|---|---|
| `CARD_H = 80` = `HEADER_H` 32 + `META_H` 24 + `LINE_H` 1 + `FOOTER_H` 23 (`layout.ts`; the components take their heights from them); `ROW_H` is 96. A 1 px line (`--card-border` at 60%) runs the card's full inner width above the footer | one place for every row |
| **The card's 1 px border is inside the 80**, so the footer draws **21** px, not 23 (measured on the page: rows 32 / 24 / 1 / 21, the card 300 × 80). The footer is `flex: 0 1 auto` with a 21 px minimum, so with the code open it keeps its 23 | a spec gap: 32 + 24 + 1 + 23 = 80 leaves nothing for the border. Kept the 80 (the layout's number) and let the footer give the 2 px up; the text is centred in either |
| `metaOf(node)` (`node-card/metaOf.ts`, 17 table rows) replaces `whereOf`; it reads only `parentId` (null = the root of the view), `kind`, `line`, `path`, `syntaxError`, `contents`. `Placed.contents` = what a node holds **directly**, by kind (a file's top-level classes and functions; a folder's folders and files); `contentsText` leaves a part that is 0 out and says `empty` when none is left | 04 §1. A nested def under a def says `line N` like any other def under its parent card. The opened folder (`/`) has no path: its meta line is empty |
| `callsFooter(calls, open)` (`node-card/callsFooter.ts`, 9 rows) and the `calls` chip of `footerChips`, last, on the right: `calls nothing` · `calls N` (closed) · a spinner while fetching · `couldn't read calls · Retry`. `Placed.callsFooter` replaces `callsNothing`; `callsLook.callsNothing` (shared) is gone with it | `calls nothing` left the meta line. A class that has methods does not say it (its methods are what is under it); a call card says it too, **except** a recursive call (it calls itself) and a callee the project does not know |
| **Retry** is a new request `retryCalls {key}` served by `callTrees.retry(key)` (the scheduler's); on a call card whose callee's own tree failed, the key is the callee's (`Placed.key`) | the scheduler had `retry`, the page had no way to reach it. Not exercised by the window suite (it cannot click in the page) |
| the spinner is the codicon `loading` with `codicon-modifier-spin`; `prefers-reduced-motion` stops it | 09 |

**Seen on the mock host** (a browser page fed the real core's answers for the minicart, with one function loading and one failed; not a real window): meta lines `empty`, `2 functions`, `4 functions`, `1 class · 3 functions`, `line 13`, the root `app/`; footers `calls nothing`, `calls 2`, `calls 6`, the spinner, `couldn't read calls · Retry`. **Open point for you:** a function with only calls shows both `+2` (left, "hidden: open to see them") and `calls 2` (right); 04 §1 asks for both, and they say the same number there. Dropping `+N` when it equals the calls' count would be one rule in `footerChips`.

Tests: `shared` 225, `webview` 193, window suite 44. The ×N fix and the context chips (8.4 part 5) were already in before this step was asked.

### 8.4 The canvas's one bar; contexts on one canvas

| Files | What |
|---|---|
| `webview/src/canvas/TopBar.tsx` (new), `webview/src/App.tsx` | 02 §2–§5; delete `SelectionStrip.tsx` |
| `webview/src/canvas/Canvas.tsx` | draws `session.active`; per-context viewport; minimap off by default, toggled from `⋯` |
| `webview/src/docs/DocPanel.tsx` | follows the active context; the "called by" line for a call |

Proof: by hand, the four states of 02 §4, and switching contexts keeps each one's pan and zoom. Commit:
`step 8.4: one bar on the canvas (back, contexts, crumbs, reveal, docs, more)`.

**As built, part 1 (`dd09633`, 2026-11-13, before 8.1–8.3).** In: the bar (Back, crumbs with the project crumb, Reveal, Docs, `⋯` with Fit View,
Collapse All, Show Minimap, Refresh Structure), the minimap off and saved, the progress line, `CanvasMessage`, `SelectionStrip` deleted,
`App.tsx` split into `webview/src/app/` hooks. Also in that commit: the webview moved to **React 19** (19.3.1), not planned here; the window
suite passes on it (32).

| Left | When |
|---|---|
| `⚠` (02 §3 #4): truncated / unread files are shown **nowhere** since part 1 | **done, part 2**: `Warning.tsx`; the click is `focusAttention` |
| an error after a good load draws the message beside the canvas | **done, part 2**: 02 §4's two rows; `CanvasMessage` only when nothing is drawn, else the ⚠ turns red with Try Again · Show Log |
| **Show Log** next to Try Again | **done, part 2**: request `showLog` |
| the progress line: VS Code's (2 px, indeterminate, `--vscode-progressBar-background` as a token), not a pulse in the accent colour | **done, part 2**: `ProgressLine.tsx`, `--progress` |
| narrow canvas (02 §5): crumbs collapse into `… ›`, keeping the last two | **done, part 2**: under 520 px, `FoldedCrumbs.tsx` |
| context chips; the canvas draws `session.active` with a viewport per context; the docs panel follows the active context and shows the "called by" line | **done, part 5** (below) |
| Focus and Go to Code as buttons (the strip had them) | **done in 8.3 part 1**, on the card |
| the bar's sizes (02 §2 "Sizes") and the card's rows under the name (04 §1) | **done, part 4** (2026-11-14): 35 px bar, `BarButton` (codicon 16 px in a 22 px square, `--toolbar-hover`), crumbs 13 px, separators 16 px codicons, body font from `--vscode-font-size`; the bar uses no Tabler icon (the package stays: the doc panel, the docs tab and the kit's select and menu still use it); cards 300 px wide (`CARD_W`, `COL_W = CARD_W + 40`), the meta line and footer start at `NAME_X` (`rows.ts`). Seen in a browser on the mock host; **the bar checked by the user in a real window (2026-11-14): good** |
| the ⚠'s click for unread files goes to NEEDS ATTENTION, which is hidden when it has no Missing node and no Problem | **done, part 3**: the "Not read" group (`shared/attention.ts`, `attentionView.ts`; docs/11 §6); `hasAttention` counts it. Also fixed: the page's refresh dropped any answer with a lower revision, so a restarted core never recovered (`state/data.ts` now keeps only the newest ask); the window suite's core-break test restarts the core |

**As built, part 5 (2026-11-14, after 8.3 part 2).** In: the context chips, a pan and zoom per context, the docs panel following the active context, and the `×N` fix.

| Decision | Why |
|---|---|
| `chipsFor(session, width, facts)` (`canvas/top-bar/chipsFor.ts`, a table test): `null` with one context; else MAIN and one chip per EXPLORE in `session.order`; the active one filled (`--chip-active`, from `--vscode-button-secondaryBackground`); tooltip *"Explore: reserve_stock, from checkout_order (line 16)"* (*lines 16, 31* when several; just *"Explore: <name>"* while the owner's tree is not held); under 360 px one chip with the active name and a ▾ menu of all | 02 §2, §3 #2, §5. `ContextChips.tsx` only draws; click = `activate`, ✕ = `closeContext` |
| **Fold rule (a spec gap, decided here):** more than three EXPLORE chips → the earliest in `session.order` fold into `+N ▾`, **except the active one, which is never hidden** (the next earliest goes instead); the `+N ▾` menu sits right after Main | the session keeps no opening time, so "oldest" is the order's own. A hidden active chip would leave the canvas showing something the bar does not name |
| `callFacts(trees, id)` and `linesText` in `shared/calls.ts`: the callee, the caller's name (the owner for a first step, else the step before) and the 1-based lines | the chip's tooltip and the docs panel's line read the same data; `footerChips` uses `linesText` too |
| viewports (`canvas/viewports.ts`, a table test; the one copy in `state/viewports.ts`): `moved`, `viewportFor`, `forgetClosed`. A viewport is remembered **with the drawing it belongs to** (`at` = the structure load + the drawn root); a different `at` is fitted again | the canvas is remounted per context (`key` has the context) and per refocus; a pan from before a refocus would frame nothing. Saved as the canvas is panned (`onMoveEnd`), not on leaving, so leaving needs no hook. A closed context is forgotten when the session says so. Not in `vscode.setState`: a new window fits every context (03 §2) |
| `docSubject(session, tree, facts)` (`shared/docSubject.ts`, a table test): the active context's selected node; a selected **call** → its callee with `calledBy {callerName, lines}`, drawn by `DocPanel` as *"called by checkout_order · line 16"* above the path; nothing selected → the focused node; a call whose callee is unknown or whose tree is not held → **no node**, never the focused node's docs | before this, a selected call card made the panel show the *focused* node (a call id is not in the structure) |
| `reveal` is ignored by a canvas that draws another context | 03 §5; it waits for that context's own canvas |
| the pages say what they drew: `rendered.context`, and a note `chipsDrawn {chips: [{id, label, active}]}` | the window suite cannot see the page |
| the `webview` test script listed the deleted `canvas/calls.test.ts` and `node --test` did not mind; removed | |

Tests: `shared` 225, `webview` 158, window suite 44, Rust unchanged. The new window test: *a double-click on a call card draws a chip, active; the Main chip brings the canvas back; the ✕ closes the context and the chip goes* — and closing the **active** context returns the canvas to MAIN. **Its limit:** the suite cannot click in the page, so it sends the intent a click sends (`explore` + activate, `activate`, `closeContext`) and reads `chipsDrawn` and `rendered.context`; the click handlers, the ✕'s look, the folded `+N ▾` menu, the narrow one-chip menu and the viewports are **not** seen in a real window, nor in a browser (this step had no mock-host proof; the viewport restore relies on React Flow firing `onMoveEnd` after a pan).

**For the hand check (real window):** double-click a call card → a chip appears, filled, and the canvas shows the callee; click `Main` → the canvas is back and its pan and zoom are as you left them; pan the EXPLORE view, go to Main and back → its own pan and zoom are back; ✕ on the chip closes it (when it was active, the canvas is on Main); the docs panel (`ⓘ`) on a selected call says *called by …* and shows the callee's docs, and follows the chip you switch to; open four explores → `+1 ▾` appears and the active chip is never in it; make the bar narrower than 360 px → one chip and a ▾ menu.

### 8.4b Big drawings (before 8.5)

From 03 §2.1 and §6.1, after the user saw hundreds of cards and a delay when clearing the focus.

| Files | What |
|---|---|
| `shared/session.ts` | `ContextState.open: { [root]: string[] }` replaces `expanded`; every intent of 03 §2.1's table; the 20-root cap; `restoreSession` turns a saved `expanded` into `open[its root]` |
| `shared/calls.ts` | `wantedCalls` reads `open[current root]` only |
| `webview/src/canvas/layout.ts` | `MAX_CARDS` = 200, the whole-family rule; the layout says how many cards there would be (`total`) |
| `webview/src/canvas/top-bar/` | the "200 of 640 cards" note |
| `webview/src/canvas/Canvas.tsx` | `onlyRenderVisibleElements`; reports the cards on screen after a pan settles |
| `extension/src/callTree.ts`, `shared/callScheduler.ts` | probes and the prefetch for cards on screen only |

Tests (table tests, `sessionProblems` after every step):
- focus `X` the first time → the open ids under `X` carry in; open things inside `X`; clear the focus → the root's set is as it was; focus `X`
  again → `X`'s set is as left;
- Reveal while focused → only `open[current root]` grows; Collapse All → only `open[current root]` empties;
- a 21st root drops the one used longest ago; the code changed → gone ids and roots are dropped; a saved `expanded` is converted;
- layout: 199, 200 and 201 cards; an open node whose children do not all fit is drawn closed, and so is every open node after it; the root is
  always drawn; `total` counts what would be drawn without the budget;
- wantedCalls: an open function under another root is not wanted;
- scheduler: a card scrolled off screen stops being probed; one scrolled on is probed.

Proof by hand: the user's case. Open many things while focused, Reveal a few deep nodes, clear the focus: the root draws as it was left, with
no delay. On a big project: Expand All Below on a large folder → at most 200 cards and the note.
Commit: `step 8.4b: what is open belongs to the focus root; a card budget; only what is on screen asks for work`.

### 8.5 The sidebar becomes a webview (structure rows)

| Files | What |
|---|---|
| `extension/package.json` | `vnoc.structure` becomes `"type": "webview"`; `vnoc.focusStack` view removed; title buttons of 06 §9; `viewsWelcome` for the view removed |
| `extension/src/sidebarView.ts` (new) | the provider (06 §10) |
| `extension/src/panels.ts` → `surfaces.ts` | `emit` reaches the sidebar |
| `webview/vite.config.ts` | a second entry, `src/sidebar/main.tsx` → `assets/sidebar.js` |
| `webview/src/sidebar/` (new) | `Sidebar`, `FocusBar`, `Tree` (flatten + virtualise + keys + type-ahead), `Row` (structure rows), `SearchResults`, the four states |
| `extension/src/commands.ts`, `shared/messages.ts` | `rowClick` (07 §3) |
| removed | `structureView.ts`, `focusView.ts`, `cursorMark.ts`, the tree-mirror code of 12, `vnoc.focusDrop`, `vnoc.focusSelection` |

Tests: `webview/src/sidebar/tree.test.ts` (flatten with open set and focus; type-ahead), `keys.test.ts` (06 §7 map); the window suite's
`sidebar.test.ts` rewritten on notes: a click with the canvas open reveals and opens no code tab; with it closed opens a preview tab; the selected
row is reported after a card click.

Proof: the tests; by hand with a 1,000-file folder (scrolls smoothly, opens in under a second). Commit:
`step 8.5: the STRUCTURE view is a webview: search, focus bar, keyboard, the click rule`.

### 8.6 Context panels in the sidebar

| Files | What |
|---|---|
| `webview/src/sidebar/ContextPanel.tsx` (new), `Sidebar.tsx` | panels, splitters (06 §4.1: drag, limits, double-click, sizes per depth), headers, activate on click, preview italics (09 §4) |

Proof: the J3 and J4 walk by hand, both surfaces side by side: select the `reserve_stock` card under `checkout_order` → an EXPLORE panel opens, not
active; double-click it → active, the canvas shows `inventory.py › reserve_stock`; `←` twice → `app`; select another call in MAIN → the panel
closes; the sidebar never shows a call row. Commit: `step 8.6: EXPLORE panels in the sidebar`.

### 8.7 Menus

| Files | What |
|---|---|
| `extension/package.json` | 08 §2–§5 and §8: one `webview/context` list, the slimmer editor and Explorer submenus, new / removed commands, `contributes.keybindings` |
| `extension/src/commands.ts` | `openInCanvas`, `explore`, `goToCallSite`, `closeContext`, `closeOtherContexts`; `callFor(arg)` |
| `extension/src/menus.test.ts` | 08 §9 |
| `extension/CHANGELOG.md` | the removed commands |

Proof: `menus.test.ts`; by hand, right-click a card, a row, a call card, the canvas background, a panel header, the editor, a tab,
the Explorer. Commit: `step 8.7: one menu list for cards and rows; the editor's V-NOC menu has five entries`.

### 8.8 States, themes, words

09 §6's checks, the README's screenshots, `../README.md`'s tree gets `16-ui-redesign/`, and `../10-ui-rework/README.md` points here. Commit:
`step 8.8: states checked in four themes; docs point to the redesign`.

## 3. What each step leaves working

| After | The user sees |
|---|---|
| 8.1 | nothing new; Back goes up one level |
| 8.2 | the old Calls section, filled by the new fetching |
| 8.2a | nothing new; Show Calls is faster after the first request |
| 8.2b | chevrons only on functions that call something; `+N` on closed function cards |
| 8.3 | calls as cards, one level at a time; the new card |
| 8.4 | one bar on the canvas |
| 8.5 | the webview sidebar (structure only) |
| 8.6 | EXPLORE panels in the sidebar |
| 8.7 | the new menus |

## 4. Later

| Step | When | What |
|---|---|---|
| **a summary on the card** | when the driver's outline returns the docstring's first line | a `summary` field in `vnoc/structure`; the card's meta line becomes two lines (summary, then path), as the web's card |
| **callers** | its own plan | "who calls this", as a second kind of call node |

Next: back to [`README.md`](README.md)
