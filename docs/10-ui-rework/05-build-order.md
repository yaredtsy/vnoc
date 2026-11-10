# 10.05 Where the files go, and the order to build in

```
build
├── 1. the folders after the rework
├── 2. the webview build (unchanged)
├── 3. the steps (each leaves a working extension)
├── 4. tests
├── 5. where this sits in the 0.1 plan
├── 6. later: the webview sidebar
└── 7. what could go wrong
```

## 1. The folders after the rework

`+` new · `~` changed · `−` deleted · no mark = untouched.

```
shared/                              code the extension AND the webviews import (no vscode, no React)
~ ├── messages.ts                    TreeNode, Requests, Events, Notes            (was an empty file)
+ ├── session.ts                     Session, Intent, reduce, keepEqual           (+ session.test.ts)
  ├── focusStack.ts                  push / pop / jump / prune                    (+ its test, as today)
+ └── tree.ts                        findNode, pathTo, chainAt, nodeAtPath, docTarget, allNodes (for the search)
                                     (today split between extension/src/nodeAt.ts and webview/src/tree.ts, with two findNode)

extension/src/
~ ├── extension.ts                   activate(): start the core, create the pieces, register. ~80 lines instead of 375.
+ ├── core.ts                        startCore, request, binary paths, the file watcher
+ ├── session.ts                     SessionStore: dispatch, save, restore, context keys
+ ├── commands.ts                    every vnoc.* command, each a few lines: nodeFor(arg) → dispatch / show
+ ├── relay.ts                       the Requests handler table; receives Notes
~ ├── panels.ts                      the canvas and docs tabs; html + CSP; showCanvas({ preserveFocus }); `vnoc.canvasOpen`
~ ├── structureView.ts               rows from the store: focus bar row, contextValue flags, vnoc.open, selection both ways
+ ├── focusView.ts                   the FOCUS view
+ ├── search.ts                      the QuickPick
+ ├── cursorMark.ts                  the FileDecorationProvider for the cursor node (step R8)
  ├── docs.ts · addDoc.ts
+ ├── menus.test.ts
− ├── focus.ts                       FocusState → session.ts
− └── nodeAt.ts                      → shared/tree.ts

webview/src/
+ ├── state/
+ │   ├── data.ts                    useData
+ │   ├── session.ts                 useSession, dispatch, accept (keepEqual)
+ │   └── view.ts                    codeOpen, docsOpen, docWidth, the docs tab's pin → vscode.setState
~ ├── host.ts                        typed request(), on(), post()
~ ├── App.tsx                        the canvas: ~60 lines, reads the stores
~ ├── canvas/                        Canvas (no callback props), layout
~ ├── node-card/                     NodeCard draws .selected / .cursor itself; + nodeContext.ts, useNodeActions.ts
~ ├── docs/DocsApp.tsx               reads cursor from the session; pin is local
+ ├── tokens.css                     the only file that names a --vscode-* variable
− ├── tree.ts · types.ts             → shared/
− └── canvas/CanvasActions.ts · canvas/reveal.ts   (reveal moved into shared/session.ts)
```

Rule for `shared/`: if both the extension and a webview need it, or it is pure logic worth a test without a browser, it goes there.

## 2. The webview build

Unchanged: one bundle, and `data-mode` picks the canvas or the docs tab (`main.tsx`). The three-entry split of the first version was only needed to
keep a webview sidebar light (U8, dropped). It comes back with the webview sidebar (§6).

## 3. The steps

Each step leaves an extension you can use. Steps R2–R4 change no behaviour on purpose: they move state to where it belongs while the old UI still
proves it works.

| Step | What | Proof it is done |
|---|---|---|
| **R0. A fresh build** | Package and install the current source ([`04-menus.md`](04-menus.md) §6). Write down what is really missing from the menus before changing anything. | you know whether the V-NOC submenu problem is a stale VSIX or a real bug |
| **R1. See the selection** | `.node-card.selected` (outline + filled header, [`06-how-it-looks.md`](06-how-it-looks.md) §2) and the syntax-error line moved inside the card: `NodeCard` reads the `selected` prop React Flow already passes. CSS and a class name, nothing else. | click a card: it is obvious which one is selected, also on a broken card |
| **R2. Typed messages** | `shared/messages.ts` (one `TreeNode`, `Requests`, `Events`, `Notes`) and `shared/tree.ts`. `host.ts` becomes the typed `request / on / post`. The `switch` in `extension.ts:342` and the `if` chain in `panels.ts:102` become the handler table in `relay.ts`. Delete the hand-written `TreeNode` copies. | `pnpm -r typecheck` passes; misspell a method name and it fails |
| **R3. The session** | `shared/session.ts` with `reduce`, `keepEqual` and their tests. `extension/src/session.ts` replaces `FocusState`, `setFocus`, `pendingReveal`, `docsTarget`, `docsPinned`. The canvas posts intents and draws the `session` event: delete its `focus` / `selected` / `expanded` / `revealId` / `centerOn` state, `focusLoaded`, the `focusChanged` echo and its `prune`; report `revealed`. The tree reads `store.get()` and refreshes on `store.onChange`. | unit tests · reload the window with a focus, a selection and open cards: all three come back · drag a card, move the editor cursor: the card stays · **measure** click → ring |
| **R4. Split `extension.ts`** | `core.ts`, `commands.ts`, `relay.ts`. Moves only. | the extension suite passes unchanged |
| **R5. Canvas stores** | `zustand`, `state/`, `nodeContext`, `useNodeActions`, `tokens.css`. `CanvasActions` and the eight callback props go. React Flow's own selection off (`elementsSelectable={false}`); the card's ring comes from the session. | the canvas works as before · React DevTools: a click re-renders two cards |
| **R6. The sidebar** | `vnoc.open` with the click rule and `vnoc.sidebar.click` ([`02-sidebar.md`](02-sidebar.md) §4); preview tabs; "Go to Code" beside the canvas; selection both ways (§5); the focus bar row; the FOCUS view; the four states with `viewsWelcome`. Remove the title-line focus text and STRUCTURE's Back / Clear. | with the canvas open, click through the sidebar: the canvas stays in front, the card is centred, no code tab appears in its column · canvas closed: one preview tab, replaced by each click · click a card: the row is selected · focus three deep, resize the FOCUS view, click its middle row |
| **R7. Menus** | The node menu tables ([`04-menus.md`](04-menus.md) §2) on both surfaces. `category` and short titles. `vnoc.revealInSidebar`, `vnoc.revealInExplorer`, `vnoc.openCanvasBeside`, `vnoc.expand*`, Show / Hide Code in Card, `vnoc.copyPath`. `vnoc.explorerMenu`, `editor/title/context`, `vnoc.extensions`, the tab-`Uri` rule in `nodeFor`. Right-click on a card selects. `menus.test.ts`. | right-click the same node on a row and on its card: the same entries in the same order, except the listed ones · right-click a background editor tab: the menu acts on that file |
| **R8. Search and the full look** | `vnoc.search` (QuickPick) with `highlight` on the active item. Everything in [`06-how-it-looks.md`](06-how-it-looks.md): highlighted and cursor cards, reveal pulse, path edges, hidden-selection dots, minimap colours, selection strip, crumb hover, canvas keys; the cursor mark on rows (`cursorMark.ts`). `Ctrl/Cmd+Enter` and `Alt+←` bindings. Tooltips as Markdown. The checks in 06 §8. | find a node by name from the 🔍: arrowing through the list highlights cards without selecting · code and canvas side by side: move the cursor, the mark follows on both · 06 §8 ticked |

Order that cannot be swapped: R2 before R3 (the session travels in typed messages), R3 before R5 and R6 (the canvas stores and the tree both draw the
session), R6 before R7 (the row's `contextValue` flags come with the new rows). R1 can be done today, before anything else.

## 4. Tests

| Test | Kind | Checks |
|---|---|---|
| `shared/session.test.ts` | `node --test` | focus selects · select does not focus · focus on a lower entry jumps back · Back keeps or moves the selection · reveal steps back, opens ancestors, selects, sets `reveal` only with `canvas: true` · `revealed` clears it · `treeChanged` prunes every context · `highlight` changes nothing else, and `select` / `focus` clear it · an intent that changes nothing returns the same object · unchanged arrays are kept · `keepEqual` reuses equal arrays |
| `shared/tree.test.ts` | `node --test` | `chainAt`, `nodeAtPath`, `docTarget` (moved) · `allNodes` |
| `extension/src/menus.test.ts` | `node --test` | the four checks in [`04-menus.md`](04-menus.md) §6 |
| extension suite (the real window, exists) | `@vscode/test-electron` | `vnoc.showOnCanvas` twice before the canvas is up reveals the **second** node · a reveal, close the canvas, reopen it: nothing is revealed again · `vnoc.focus` then *Reload Window*: the session is the same · `vnoc.open` with the canvas open leaves no new text editor in the canvas's column · with the canvas closed it opens one preview tab · `treeView.reveal` → `onDidChangeSelection` → no second broadcast |
| `webview` layout and markdown tests | exist | unchanged |

`extension/package.json`'s `unit` script runs `src/nodeAt.test.ts` today; point it at `shared/*.test.ts` and `src/menus.test.ts` when those move.

## 5. Where this sits in the 0.1 plan

`02-mvp-and-releases.md` lists Steps 0–6 for 0.1. Steps 1–4 are built: Step 4, files and folders, is commit `dcdeed6` (docs on files and folders,
`# FileID:`, `.vnoc-id`). **Do this rework now, before Step 5 (Missing, Relink, Remove):** Step 5 adds a view, menu entries and state, and would
otherwise be written against the old state and then again.

The rework must keep what Step 4 added: Add Doc on files and folders from every menu (`04-menus.md` §2, §3), and the "not on the opened folder
itself" check in `vnoc.addDoc`.

Nothing here changes what 0.1 contains, and nothing touches a committed file format (`.vnoc/`, IDs, markers), so all of it can still change in any
later release.

## 6. Later: the webview sidebar

Postponed by U1. **When to decide:** with EXPLORE contexts in 0.2, after using the native EXPLORE view (`03-focus-and-contexts.md` §5). If stacked,
resizable context panels or an in-view search box turn out to matter, build it then.

What it would reuse, unchanged: the session, the intents, the reducer and its tests, the message map, the card's `nodeContext` and
`useNodeActions`, the click rule (it is a command in the extension).

What it would add or change:

| Piece | Change |
|---|---|
| `extension/src/sidebarView.ts` | a `WebviewViewProvider` for a `"type": "webview"` view, replacing STRUCTURE, the focus bar row and the FOCUS view |
| `webview/src/sidebar/` | ported from the web Sidebar: `SearchInput`, `TabContextStack` → `ContextStack`, `ContextPanel`, `FocusBreadcrumb` → `FocusBar`, `ProjectTree` → `Tree` / `TreeRow`; `filterTree` and `visibleRows` as pure functions |
| keyboard and accessibility | everything the native tree gives for free: roving tabindex, `role="tree"` and `treeitem` with `aria-level` / `aria-expanded` / `aria-selected`, arrow keys, Home / End, type-ahead. Arrow keys move with `select` only; a click or Enter runs `vnoc.open`. |
| open / closed | shared with the canvas (U5 becomes possible) |
| menus | the row moves to `webview/context`: one list, and the test's exceptions shrink to Show on Canvas / Reveal in Sidebar and code in card |
| build | three entry files (sidebar, canvas, docs) so the sidebar does not load React Flow and Monaco; `script-src` gains `${webview.cspSource}` for the shared chunks, and CSS is one file (`cssCodeSplit: false`) so a shared chunk's styles are not lost **[check both in a fresh build]** |
| lost | the user's file icon theme (VS Code does not give it to webviews) |

## 7. What could go wrong

| Risk | How you would notice | What to do |
|---|---|---|
| The round trip makes card clicks feel late | the measurement in step R3 is above ~30 ms, or you feel it | draw the ring locally at once, let the session correct it (`01-state.md` §3) |
| Cards snap back after dragging | step R3's proof fails | `keepEqual` is not applied, or `reduce` copies arrays it did not change |
| The tree and the canvas chase each other's selection | a selection flickers, or the extension log shows a burst of `select` | `reduce` must return the same object for a `select` of the selected id; the test in §4 |
| Arrow keys in the tree open files | step R6 | VS Code ran `item.command` on the key: dispatch from `onDidChangeSelection` only and drop the command for keyboard (keep it for the mouse) |
| A user wants the old "click opens code" | they say so | `vnoc.sidebar.click: "code"` |
| The saved session from before the rework has the old shape | a crash at activation after updating | read `vnoc.session.v1`, convert `vnoc.focus` once, drop what does not parse (`01-state.md` §6) |
| The cursor decoration does not show on custom rows | step R8 | the description fallback (`02-sidebar.md` §7) |

Next: [`06-how-it-looks.md`](06-how-it-looks.md)
