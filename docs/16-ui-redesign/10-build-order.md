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

### 8.2b Exact chevrons: the probe (05 §3.1)

After 8.2 and 8.2a.

| Files | What |
|---|---|
| `crates/vnoc/src/protocol.rs` | `CallTreeParams` gains `depth: Option<u32>`, passed to the driver (which already maps it to `render_depth`, `drivers/python/src/call_tree.rs:36`); new `vnoc/hasCalls { keys } → { revision, calls: { key: n } }` |
| `crates/vnoc/src/calls.rs` | `has_calls`: for each key, the depth-1 tree through the **same** merge and filter as `vnoc/callTree`; `n` = the children left. Its marks are dropped |
| `extension/src/callTree.ts` | probes the function and class cards drawn whose calls are unknown, the selected first; cancels what is no longer drawn; drops answers on a new revision |
| `shared/calls.ts` | `Calls` gains `{ state: "probed"; count: number }`: a chevron when `count > 0`, `+count` in the footer, *calls nothing* when 0. Opening a probed card fetches the full tree as before |

Tests: `calls.rs` (a function that calls only a library → `0`; two calls to one callee → `1`; the depth-1 tree's `Cut` never reaches the answer);
`shared/calls.test.ts` (probed → chevron and `+N`; pending → the empty slot). Proof: by hand on httpx, a file of 30 functions gets its chevrons
in well under a second, and none disappears on opening. Commit: `step 8.2b: exact chevrons from a depth-1 probe (vnoc/hasCalls)`.

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
| `NodeCalls.tsx` and `canvas/calls.ts`'s `callRows` (the old "Calls" section, still drawn, 24 + 20 px rows added to the card) | deleted by 8.3 part 2 |
| `symbol-method` for a function inside a class | 8.3 part 2 (needs the parent's kind in `Placed`) |
| `@tabler/icons-react` | still used by the bar and the menus; goes when nothing uses it |

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
| context chips; the canvas draws `session.active` with a viewport per context; the docs panel follows the active context and shows the "called by" line | after 8.1 (contexts) and 8.3 (call cards) |
| Focus and Go to Code as buttons (the strip had them) | **done in 8.3 part 1**, on the card |
| the ⚠'s click for unread files goes to NEEDS ATTENTION, which is hidden when it has no Missing node and no Problem | **done, part 3**: the "Not read" group (`shared/attention.ts`, `attentionView.ts`; docs/11 §6); `hasAttention` counts it. Also fixed: the page's refresh dropped any answer with a lower revision, so a restarted core never recovered (`state/data.ts` now keeps only the newest ask); the window suite's core-break test restarts the core |

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
