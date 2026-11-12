# 16.08 Menus: one use case per entry

```
menus
├── 1. one list for a card and a row (R12)
├── 2. the node menu (a structure node)
├── 3. the call menu (a call card; the sidebar has no call rows)
├── 4. V-NOC in VS Code's menus: code editor, editor tab, Explorer (R11)
├── 5. the other menus: canvas background, panel header, view titles, the bar's ⋯
├── 6. what a card or a row tells the menu (nodeContext)
├── 7. if webview/context does not reach a webview view
├── 8. commands: new, renamed, removed · context keys · keys
└── 9. the test that keeps it honest
```

Replaces 10-ui-rework/04. The rule for every entry below: **it serves one job of 01 §2, and its name says what happens.** Entries that only
differed by "also opens the canvas" are merged.

## 1. One list (R12)

The card (canvas) and the row (sidebar) are both webviews now, so **one `webview/context` list** serves both. An entry that only makes sense on
one surface tests `surface`:

```jsonc
"when": "webviewId =~ /^vnoc\\.(canvas|structure)$/ && webviewSection == 'node' && …"
```

Groups, in this order everywhere: `1_nav` · `2_open` · `3_calls` · `4_code` · `5_docs` · `9_copy`.

## 2. The node menu (folder, file, class, function)

`S` sidebar row, `C` canvas card.

| Group | Entry | Job | What happens | S | C | `when` (besides `webviewSection == 'node' && role == node`) |
|---|---|---|---|---|---|---|
| `1_nav@1` | **Focus** | J2 | the focus becomes its lineage; the canvas follows if open | ✓ | ✓ | `!isFocusRoot` |
| `1_nav@1` | **Back** | J2 | up one level | ✓ | ✓ | `isFocusRoot` |
| `1_nav@2` | **Open in Canvas** | J1 | reveal it on the canvas, opening the canvas if closed | ✓ | | `surface == sidebar` |
| `1_nav@2` | **Reveal in Sidebar** | J1 | select it in the sidebar and bring the sidebar up | | ✓ | `surface == canvas` |
| `2_open@1` | **Expand** / **Collapse** | J1 | one level | ✓ | ✓ | `hasChildren && !open` / `open` |
| `2_open@2` | **Expand All Below** | J1 | up to 500 structure nodes; never calls (05 §4) | ✓ | ✓ | `kind != function` and `hasChildren` |
| `3_calls@1` | **Show Calls** | J3 | open its calls one level (the canvas comes up if closed) | ✓ | ✓ | `kind == function \|\| kind == class` and `vnoc.callTree` |
| `4_code@1` | **Go to Code** | J5 | the definition in an editor | ✓ | ✓ | `kind != folder` |
| `4_code@2` | **Show Code in Card** / **Hide Code in Card** | J5 | the code inside the card | | ✓ | `hasCode && !codeOpen` / `codeOpen` |
| `4_code@3` | **Reveal in Explorer** | J5 | VS Code's Explorer at the file / folder | ✓ | ✓ | `kind == file \|\| kind == folder` |
| `5_docs@1` | **Show Docs** | J6 | the docs panel (canvas visible) or tab | ✓ | ✓ | — |
| `5_docs@2` | **Add Doc…** | J6 | | ✓ | ✓ | `nodeKey != '/'` |
| `5_docs@3` | **Attach a Missing Doc Here…** | J8 | | ✓ | ✓ | `vnoc.hasMissing && nodeKey != '/'` |
| `5_docs@4` | **Write ID** | J8 | | ✓ | ✓ | `byName` |
| `9_copy@1` | **Copy Path** | — | `path:line` | ✓ | ✓ | — |

Gone from today's lists: **Show on Canvas** (was "focus and open the canvas": two verbs; now Focus, and Open in Canvas), **Open Canvas** on a
row (now Open in Canvas), **Show Call Tree** (now Show Calls). On a row, Focus Back was missing (the focus bar row had it); it is on the focused
row now.

## 3. The call menu (a call card)

Canvas only: the sidebar has no call rows (07 §2, C1). Every entry also tests `surface == canvas`.

| Group | Entry | Job | What happens | `when` (besides `role == call`) |
|---|---|---|---|---|
| `1_nav@1` | **Explore** | J4 | its EXPLORE context, activated | `calleeKnown` |
| `2_open@1` | **Expand** / **Collapse** | J3 | one level of calls | `hasChildren` |
| `4_code@1` | **Go to Call Site** | J5 | the line where it is called | `sites == 1` |
| `4_code@1` | **Go to Call Site…** | J5 | a QuickPick of every site of the merged call, `line 16` / `base.py:12` with the line's text; moving through it previews each site (04 §2.1) | `sites > 1` |
| `4_code@2` | **Go to Code** | J5 | the callee's definition | `calleeKnown` |
| `4_code@3` | **Show / Hide Code in Card** | J5 | the callee's code | `calleeKnown` |
| `5_docs@1` | **Show Docs** · `5_docs@2` **Add Doc…** | J6 | the callee's | `calleeKnown` |
| `9_copy@1` | **Copy Path** | — | the callee's `path:line` | `calleeKnown` |

The web's call menu has **Remove Call** and **Add Call**: calls there are edited by hand. Here they come from the code; nothing to remove.

## 4. V-NOC in VS Code's menus (R11)

### Code editor (right-click in the text) and editor tab (right-click the tab)

The node is the one at the cursor (the innermost function or class, else the file). From a background tab, that tab's file (10/04 §3, kept).

```
V-NOC ▸  Reveal in Sidebar          select it in the sidebar; the canvas reveals it too if open        J7, J1
         Open in Canvas             the canvas, BESIDE this editor, the node revealed and selected     J7, J1
         Focus                      the sidebar (and the canvas if open) show only it                  J7, J2
         ──────────
         Show Calls                 Open in Canvas, then its calls one level      (function / class)   J7, J3
         ──────────
         Show Docs                  the docs tab beside this editor                                    J7, J6
         Add Doc…                                                                                      J6
         Attach a Missing Doc Here…                                         (only with missing docs)   J8
         Write ID                                                           (only when found by name)  J8
```

Five entries, two conditional, down from nine. Merged or gone:

| Was | Now |
|---|---|
| Focus · Show on Canvas | **Focus** (focus only) and **Open in Canvas** (reveal + open) |
| Reveal in Sidebar | **Reveal in Sidebar** (kept; it now also reveals on an open canvas) |
| Open Canvas · Open Canvas to the Side | **Open in Canvas**, always beside: from an editor you want the code to stay visible |

### Explorer (right-click a file or folder)

```
V-NOC ▸  Reveal in Sidebar · Open in Canvas · Focus
         Show Docs · Add Doc…
```

Which files: `resourceExtname in vnoc.extensions` and folders, as today.

## 5. The other menus

| Where | Contribution | Entries | Job |
|---|---|---|---|
| canvas background | `webview/context`, `webviewSection == 'pane'` | Fit View · Collapse All · ─ · Back · Clear Focus (`vnoc.focused`) · Clear Selection (`vnoc.hasSelection`) · ─ · Close Explore (`activeIsExplore`) | J1, J2 |
| sidebar panel header | `webview/context`, `webviewSection == 'context'` | Close (EXPLORE) · Close Others | J4 |
| sidebar empty space | `webviewSection == 'pane'`, `surface == sidebar` | Collapse All · Clear Focus · Refresh Structure | |
| STRUCTURE view title | `view/title`, `view == vnoc.structure` | `⊞` Open Canvas · `⊟` Collapse All · `⋯` Refresh Structure · Search… · Restore Removed Doc… · Empty Trash | 06 §9 |
| the canvas bar's `⋯` | a menu drawn by the page (not VS Code's): Fit View · Collapse All · Show Minimap ✓ · Refresh Structure | 02 §3 | |
| NEEDS ATTENTION | unchanged (11) | | J8 |

A crumb has no menu (as built in 0.1).

## 6. What a card or a row tells the menu

```ts
// webview/src/node-card/nodeContext.ts — one function for the card and the row
export function nodeContext(f: {
  surface: "canvas" | "sidebar"; contextId: ContextId; id: string;            // id: a structure key or a call id
  role: "node" | "call"; key: string | null; kind: Kind;
  hasChildren: boolean; open: boolean; isFocusRoot: boolean;
  hasCode: boolean; codeOpen: boolean; docs: number; byName: boolean;
  calleeKnown: boolean; sites: number;
}): string {
  return JSON.stringify({ webviewSection: "node", preventDefaultContextMenuItems: true, nodeKey: f.key, ...f });
}
```

Every command still starts with `nodeFor(arg)`: `nodeKey` gives the structure node (the callee for a call), and the new `callFor(arg)` gives the
call (its sites) from `id`.

## 7. If `webview/context` does not reach a webview view

**[check first in step 8.6]**: right-click a row in the new sidebar with one test entry in `webview/context` whose `when` is
`webviewId == 'vnoc.structure'`. VS Code documents `webview/context` for webviews; whether a *view* gets it was not checked here.

If it does not: the sidebar draws its own menu (a small React list in `--vscode-menu-*` colours), built from **the same table**, exported as data
from `extension/package.json` at build time (`scripts/menus.mjs` writes `webview/src/sidebar/menu.json`), and runs the command through a
`command { name, arg }` request. One list stays one list.

## 8. Commands, context keys, keys

| Command | Title | Status |
|---|---|---|
| `vnoc.openInCanvas` | Open in Canvas | **new**: reveal + open (beside an editor when run from one). Replaces `showOnCanvas`, `openCanvas` on a node, `openCanvasBeside` on a node. |
| `vnoc.openCanvas` | Open Canvas | kept, **no node**: just open |
| `vnoc.focus` | Focus (inside the "V-NOC ▸" submenu it needs no prefix) | kept; lineage (R2) |
| `vnoc.revealInSidebar` | Reveal in Sidebar | kept; also reveals on the canvas when it is open |
| `vnoc.showCalls` | Show Calls | **new**, replaces `vnoc.showCallTree` |
| `vnoc.explore` | Explore | **new** |
| `vnoc.goToCallSite` | Go to Call Site… | **new** (one site: goes; several: QuickPick) |
| `vnoc.closeContext` · `vnoc.closeOtherContexts` | Close · Close Others | **new** |
| `vnoc.showOnCanvas` · `vnoc.openCanvasBeside` · `vnoc.showCallTree` · `vnoc.focusDrop` · `vnoc.focusSelection` | | **removed** (`CHANGELOG.md` says so: a user's own key binding on one of them stops working) |
| the rest | | unchanged |

| Context key | Set by | For |
|---|---|---|
| `vnoc.extensions` · `vnoc.callTree` · `vnoc.hasMissing` · `vnoc.canvasOpen` · `vnoc.focused` · `vnoc.hasSelection` | unchanged (`vnoc.focused` and `vnoc.hasSelection` now about the **active** context) | |
| `activeIsExplore` | the session store | Close Explore on the canvas background |

Keys: `Alt+←`, `Enter`, `Ctrl/Cmd+Enter`, `Space`, `Esc` and the arrows are handled **inside** each page (03 §7, 06 §7), so they work the same on
both surfaces and skip text fields and code cards. `contributes.keybindings` keeps only `Ctrl/Cmd+Alt+N` (Search…).

## 9. The test

`extension/src/menus.test.ts` (exists) becomes:

1. every command in any menu is declared and has `category: "V-NOC"` (kept)
2. every node entry in `webview/context` names **both** surfaces in its `when`, except the four that test `surface` (Open in Canvas,
   Reveal in Sidebar, Show / Hide Code in Card); every call entry tests `surface == canvas`
3. every key a `when` tests is a key `nodeContext()` writes (kept)
4. the editor submenu and the Explorer submenu name only commands that accept a `Uri` (a list in the test)

Next: [`09-states.md`](09-states.md)
