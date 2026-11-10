# 10.04 Menus: the node menu on the row and the card, and V-NOC in VS Code's own menus

```
menus
├── 1. two lists, one set of commands
├── 2. the node menu (sidebar row and canvas card)
│   ├── the entries
│   ├── how a row and a card describe themselves
│   └── the hover buttons
├── 3. the V-NOC submenu in VS Code's menus
│   ├── code editor · editor tab
│   ├── Explorer
│   └── which files get it
├── 4. the other menus (focus bar row, FOCUS view, canvas background, crumb, view titles)
├── 5. commands, context keys, key bindings
└── 6. how to check a menu really shows, and a test that keeps the lists equal
```

> **Built differently from the tables below (decided while building, 2026-11-10).** Where this file and the code disagree, the code is right:
>
> | Entry | The plan said | Built |
> |---|---|---|
> | **Focus** | focus; the canvas shows it too | focus, and **the V-NOC side bar comes up**; the canvas is not opened (`vnoc.focus`) |
> | **Show on Canvas** | reveal the node, no focus change | **zoom the canvas into the node** (focus), opening the canvas if it is closed (`vnoc.showOnCanvas`) |
> | **Open Canvas** (on a node) | bring the canvas up as it was | **reveal the node** on the canvas: its ancestors open, the focus steps back if it hides the node, the card is selected and centred. With no node (the palette) it only opens. (`vnoc.openCanvas`) |
> | titles | "V-NOC: Focus" in the submenu | every command has `category: "V-NOC"` and a short title, so the palette reads "V-NOC: Focus" and a menu reads "Focus" |
> | the row's and the card's lists | exceptions: row only `showOnCanvas`; card only `revealInSidebar`, `focusBack`, `expand`, `collapse`, `expandAll`, `showCodeInCard`, `hideCodeInCard` | the same, plus **row only `openCanvas`**: a card is already on the canvas (`menus.test.ts` lists the exceptions) |
> | the crumb's menu (§4) | `webviewSection == 'crumb'` | **not built**; nor a right-click menu on the selection strip |
> | arrow keys in the sidebar (`02-sidebar.md` §4) | move the selection | move VS Code's focus row only; `Enter`, `Space` or a click select (checked in step R6) |

## 1. Two lists, one set of commands

A row of the native tree gets its right-click menu from `view/item/context`; a card in the canvas webview gets it from `webview/context`. Two
contribution points, so two lists in `package.json` (U6). Today they drifted: different order, different entries.

What keeps them the same now:

1. **Every entry runs the same command** on both surfaces, and every command starts with `nodeFor(arg)` (exists: `extension.ts:160`), which turns a
   tree row, a card's context object, a `Uri` or the cursor into a node.
2. **The same groups, in the same order**: `1_show` · `2_view` · `3_code` · `4_docs` · `9_copy`.
3. **A test** reads `package.json` and fails when the two lists differ, except for the few entries that only make sense on one surface (§6).

When the sidebar becomes a webview (a later release), the two lists become one `webview/context` list and the test's exceptions shrink to two.

## 2. The node menu

### The entries

`S` = on a sidebar row (`view/item/context`, `view == vnoc.structure`), `C` = on a canvas card (`webview/context`, `webviewId == 'vnoc.canvas' &&
webviewSection == 'node'`).

| Group | Entry | Command | S | C | `when`, besides the surface |
|---|---|---|---|---|---|
| `1_show@1` | Focus | `vnoc.focus` | ✓ | ✓ | S: always · C: `!isFocusRoot` |
| `1_show@2` | Focus Back | `vnoc.focusBack` | | ✓ | `isFocusRoot` (on a row, the focus bar row has Back) |
| `1_show@3` | Show on Canvas | `vnoc.showOnCanvas` | ✓ | | — |
| `1_show@3` | Reveal in Sidebar | `vnoc.revealInSidebar` | | ✓ | — |
| `2_view@1` | Expand | `vnoc.expand` | | ✓ | `hasChildren && !open` (a row has VS Code's own chevron) |
| `2_view@1` | Collapse | `vnoc.collapse` | | ✓ | `hasChildren && open` |
| `2_view@2` | Expand All Below | `vnoc.expandAll` | | ✓ | `hasChildren` · capped (§5) |
| `3_code@1` | Go to Code | `vnoc.goToCode` | ✓ | ✓ | not a folder |
| `3_code@2` | Show Code in Card | `vnoc.showCodeInCard` | | ✓ | `hasCode && !codeOpen` |
| `3_code@2` | Hide Code in Card | `vnoc.hideCodeInCard` | | ✓ | `hasCode && codeOpen` |
| `3_code@3` | Reveal in Explorer | `vnoc.revealInExplorer` | ✓ | ✓ | a folder or a file |
| `4_docs@1` | Show Docs | `vnoc.showDocs` | ✓ | ✓ | — |
| `4_docs@2` | Add Doc… | `vnoc.addDoc` | ✓ | ✓ | any node except the opened folder itself (S: the opened folder is never a row · C: `nodeKey != '/'`). Files and folders take docs since MVP Step 4 (`dcdeed6`). |
| `9_copy@1` | Copy Path | `vnoc.copyPath` | ✓ | ✓ | — |

Two separate commands for Show / Hide Code in Card: a menu entry's label is its command's title, so one command cannot read two ways.

`vnoc.revealInExplorer` is a three-line wrapper: VS Code's own `revealInExplorer` wants a `Uri`, and a menu on a card passes the card's context object.

```ts
register("vnoc.revealInExplorer", async (arg) => vscode.commands.executeCommand("revealInExplorer", absolute((await nodeFor(arg)).path)));
```

How it maps to the web menu in your first screenshot:

| Web | Here |
|---|---|
| Focus | Focus |
| Expand | Expand / Collapse (card); the tree's chevron |
| Copy link | 0.1: Copy Path. A real link (`vscode://vnoc.vnoc/node?…`, opened by `vscode.window.registerUriHandler`) comes when IDs are stable enough to share: later. |
| Build Prompt · New task here | with the features they belong to (agent access, tasks): later |
| Add Call | 0.2, with call trees |
| Create Group | later, with groups |

A right-click on a card **selects it first** (a `contextmenu` listener dispatches `select`), so the ring shows what the menu is about, as in the
Explorer. On a row VS Code does that itself. Selecting never opens anything (`03-focus-and-contexts.md` §1 rule 4), so this cannot open a code tab.

### How a row and a card describe themselves

A `when` clause can only test what the surface tells VS Code.

**A card** sets `data-vscode-context`, built by one function so every card says the same things:

```ts
// webview/src/node-card/nodeContext.ts
export function nodeContext(f: { node: TreeNode; contextId: string; open: boolean; isFocusRoot: boolean; codeOpen: boolean }): string {
  return JSON.stringify({
    webviewSection: "node", preventDefaultContextMenuItems: true,
    nodeKey: f.node.id, contextId: f.contextId, kind: f.node.kind,
    hasChildren: f.node.children.length > 0, open: f.open, isFocusRoot: f.isFocusRoot,
    hasCode: f.node.range !== null, codeOpen: f.codeOpen, docs: f.node.docs,
  });
}
```

> **🧩 VS Code note: a native menu on a webview.** On a right-click, VS Code collects `data-vscode-context` from the clicked element **and all its
> ancestors**, merges them (the inner one wins), and turns every key into a context key for the `when` clauses. The command then receives the merged
> object as its first argument. `preventDefaultContextMenuItems` hides Cut / Copy / Paste.

**A row** sets `contextValue`, which is one string. Put the flags in it as words, and test them with a regex:

```ts
item.contextValue = ["node", node.kind, node.docs > 0 ? "docs" : "", node.range ? "hasCode" : ""].filter(Boolean).join(" ");
// package.json:  "when": "view == vnoc.structure && viewItem =~ /\\b(file|class|function)\\b/"   (e.g. Go to Code: not a folder)
```

The focus bar row is `focusBar`, so no node entry matches it.

### The hover buttons

| Surface | Buttons | How |
|---|---|---|
| a row | `⌖` Focus · `↗` Go to Code (not folders) · `＋` Add Doc (every row) | `view/item/context`, `"group": "inline@1…3"`. VS Code shows them on hover and on the selected row. |
| a card | header: `▸` open · `📄 n` docs · `</>` code · footer: `⌖` · `↗` · `＋` | React buttons calling `useNodeActions` |

```ts
// webview/src/node-card/useNodeActions.ts — replaces today's CanvasActions context
export function useNodeActions(node: TreeNode, contextId: string) {
  return useMemo(() => ({
    select:   () => dispatch({ type: "select", id: node.id }, contextId),
    focus:    () => dispatch({ type: "focus",  id: node.id }, contextId),
    toggle:   () => dispatch({ type: "toggle", id: node.id }, contextId),
    goToCode: () => request("goToCode", { key: node.id }),
    addDoc:   () => request("addDoc",   { key: node.id }),
  }), [node.id, contextId]);
}
```

A card's `↗` and a row's `↗` end in the same command, so "Go to Code" means the same thing everywhere (`02-sidebar.md` §4: beside the canvas when it
is in front).

## 3. The V-NOC submenu in VS Code's menus

This is U4: from the code you are reading, get to the same node in V-NOC.

### Code editor (right-click in the text) and editor tab (right-click the tab)

```
V-NOC ▸  Show on Canvas              reveal + select the node at the cursor; the focus is not pushed     vnoc.showOnCanvas
         Focus on Canvas             push it on the focus stack; canvas and sidebar show only it          vnoc.focus
         Reveal in V-NOC Sidebar     open the V-NOC side bar with the node selected and in view           vnoc.revealInSidebar   NEW
         ─────────
         Open Canvas                 bring the canvas up as it was; nothing else changes                  vnoc.openCanvas
         Open Canvas to the Side     the same, in the column beside this editor                           vnoc.openCanvasBeside  NEW
         ─────────
         Show Docs                   the docs tab beside this editor                                      vnoc.showDocs
         Add Doc…                    the node at the cursor; outside any function or class, the file      vnoc.addDoc
```

"The node at the cursor" is `chainAt` (exists): the innermost function or class around the cursor, else the file. Add Doc… is always shown: every
place in a supported file has a node, so it always has something to attach to.

**The tab's menu passes the tab's `Uri`.** Today `nodeFor` ignores a `Uri` that comes without an Explorer selection and uses the active editor's
cursor, so a right-click on a background tab would act on the wrong file. New rule in `nodeFor`: a `Uri` that is the active editor's document → the
node at the cursor; any other `Uri` → that file's node.

`vnoc.revealInSidebar` is short, because the session and the tree do the work:

```ts
store.dispatch({ type: "reveal", id: node.id, canvas: false });   // steps the focus back if it hides the node, selects it
await treeView.reveal(rowFor(node.id), { select: true, focus: true, expand: false });   // opens the V-NOC side bar too
```

### Explorer (right-click a file or folder)

Its **own** submenu, `vnoc.explorerMenu`, without the cursor and canvas-opening entries that only make sense in an editor:

```
V-NOC ▸  Show on Canvas · Focus on Canvas · Reveal in V-NOC Sidebar
         Show Docs · Add Doc…
```

Add Doc… here works on files and folders (MVP Step 4). Right-clicking the opened folder's own root in the Explorer is answered by the command's
existing message ("Docs go on a folder inside the opened folder"), as today.

### `package.json`

```jsonc
"submenus": [
  { "id": "vnoc.editorMenu",   "label": "V-NOC" },
  { "id": "vnoc.explorerMenu", "label": "V-NOC" }
],
"menus": {
  "editor/context":       [ { "submenu": "vnoc.editorMenu",   "group": "navigation@9",     "when": "resourceExtname in vnoc.extensions" } ],
  "editor/title/context": [ { "submenu": "vnoc.editorMenu",   "group": "3_open@9",         "when": "resourceExtname in vnoc.extensions" } ],
  "explorer/context":     [ { "submenu": "vnoc.explorerMenu", "group": "7_modification@9", "when": "explorerResourceIsFolder || resourceExtname in vnoc.extensions" } ],
  "vnoc.editorMenu": [
    { "command": "vnoc.showOnCanvas",     "group": "1_show@1" },
    { "command": "vnoc.focus",            "group": "1_show@2" },
    { "command": "vnoc.revealInSidebar",  "group": "1_show@3" },
    { "command": "vnoc.openCanvas",       "group": "2_canvas@1" },
    { "command": "vnoc.openCanvasBeside", "group": "2_canvas@2" },
    { "command": "vnoc.showDocs",         "group": "3_docs@1" },
    { "command": "vnoc.addDoc",           "group": "3_docs@2" }
  ],
  "vnoc.explorerMenu": [
    { "command": "vnoc.showOnCanvas",     "group": "1_show@1" },
    { "command": "vnoc.focus",            "group": "1_show@2" },
    { "command": "vnoc.revealInSidebar",  "group": "1_show@3" },
    { "command": "vnoc.showDocs",         "group": "3_docs@1" },
    { "command": "vnoc.addDoc",           "group": "3_docs@2" }
  ]
}
```

### Which files get it

Today every `when` says `resourceLangId == python`. That hides the menu in a Python file VS Code has not recognised, and it has to be edited for
every new driver. Use a context key the extension sets once:

```ts
// extension.ts, at activation (later: from the driver manifests, S14)
void vscode.commands.executeCommand("setContext", "vnoc.extensions", [".py", ".pyi"]);
```

> **🧩 VS Code note: `in`.** A `when` clause can test membership in a list you put in a context key: `resourceExtname in vnoc.extensions`.
> `resourceExtname` is VS Code's own key (the file's extension, with the dot). The list of supported files lives in one line of TypeScript and no
> menu entry names a language.

## 4. The other menus

| Where | Contribution · `when` | Entries |
|---|---|---|
| focus bar row | `view/item/context` · `viewItem == focusBar` | inline `←` Focus Back · `✕` Clear Focus |
| FOCUS view title | `view/title` · `view == vnoc.focusStack` | `←` Focus Back · `✕` Clear Focus |
| FOCUS view row | `view/item/context` · `view == vnoc.focusStack` | click: jump back (the row's command) · inline `✕` drop this step and those after it · menu: Show on Canvas · Go to Code |
| STRUCTURE title | `view/title` · `view == vnoc.structure` | `🔍` Search · `⟳` Refresh · Collapse All (`"showCollapseAll": true` on the view, VS Code's own) · overflow: Open Canvas, Open Canvas to the Side |
| canvas background | `webview/context` · `webviewSection == 'pane'` | Fit View · Collapse All · Clear Selection (`vnoc.hasSelection`) · Focus Back / Clear Focus (`vnoc.focused`) · Refresh |
| a crumb | `webview/context` · `webviewSection == 'crumb'` | Jump Back to Here · Show on Canvas · Go to Code |

Back and Clear leave STRUCTURE's title: they are on the focus bar row and in the FOCUS view now.

## 5. Commands, context keys, key bindings

Every command gets `"category": "V-NOC"` and a short title. The palette then shows *V-NOC: Focus on Canvas*, and inside the V-NOC submenu it reads
*Focus on Canvas* (today the submenu repeats "V-NOC:" on every line).

| Command | Changes since today |
|---|---|
| `vnoc.focus` · `focusBack` · `focusClear` · `showOnCanvas` · `goToCode` · `showDocs` · `addDoc` · `openCanvas` · `refreshStructure` | body becomes `store.dispatch(...)` plus what it opens; arguments via `nodeFor` |
| `vnoc.open` (the row's click) · `vnoc.focusSelection` · `vnoc.focusJump` · `vnoc.search` · `vnoc.revealInSidebar` · `vnoc.revealInExplorer` · `vnoc.openCanvasBeside` · `vnoc.expand` · `vnoc.collapse` · `vnoc.expandAll` · `vnoc.collapseAll` · `vnoc.showCodeInCard` · `vnoc.hideCodeInCard` · `vnoc.copyPath` · `vnoc.clearSelection` · `vnoc.fitView` | **new** |

Commands that only make sense from a menu or a row (`open`, `expand`, `showCodeInCard`, `focusJump`, …) are hidden from the palette with
`"commandPalette": [{ "command": "…", "when": "false" }]`, as `vnoc.goToCode` is today.

`vnoc.expandAll` opens at most 500 nodes below the one clicked and says so when it stops **[pick the number after measuring]**: the open set is in
the saved session and travels with every broadcast.

| Context key | Set by | Used for |
|---|---|---|
| `vnoc.extensions` | activation | which files get the V-NOC submenus |
| `vnoc.focused` | the session store, when it changes | Back, Clear, the FOCUS view |
| `vnoc.hasSelection` | the session store | Clear Selection |
| `vnoc.canvasOpen` | `Panels` (created, disposed) | the click rule (`02-sidebar.md` §4) |
| `vnoc.hasMissing` | on `indexChanged` | the Missing view |

Key bindings (`contributes.keybindings`):

| Key | Command | `when` |
|---|---|---|
| `Alt+←` | `vnoc.focusBack` | `vnoc.focused && (focusedView == vnoc.structure \|\| focusedView == vnoc.focusStack)` |
| `Ctrl/Cmd+Enter` | `vnoc.focusSelection` | `focusedView == vnoc.structure` |

On the canvas, `Alt+←` stays the webview's own `keydown` listener (today `App.tsx:116`), now posting the `focusBack` intent, and it **skips the key
when the target is a text field or a code card**: `Option+←` is "word left" on macOS, and a VS Code key binding cannot see which element inside a
webview has focus.

## 6. How to check a menu really shows

You reported that the V-NOC entry is not in VS Code's menus. The declaration is in `package.json` at `11b99ce`, and the `.vsix` in the repo is older
than it. Rule out the stale build first:

```
1. node scripts/package.mjs                                   (builds core, driver, webview, extension, and the .vsix)
2. code --install-extension vnoc-darwin-arm64.vsix --force
3. Developer: Reload Window
4. right-click inside a .py file  →  is there a "V-NOC" line near the bottom of the first group?
```

Or press F5 in the repo, which always runs the current source. If the entry is still missing after that, run *Developer: Inspect Context Keys*,
click in the editor, and look at `resourceLangId` / `resourceExtname` in the console: the `when` clause is the only thing that can hide a declared
submenu.

Then stop this from happening silently again. A menu that names a command that does not exist, or a `when` with a typo, fails without any error.
One small test reads `package.json`:

```ts
// extension/src/menus.test.ts — node --test, no VS Code window needed
// 1. every command used in any menu is declared in contributes.commands, and has category "V-NOC"
// 2. every submenu used is declared in contributes.submenus
// 3. the node commands on a row (view/item/context, view == vnoc.structure, viewItem =~ node) and on a card
//    (webview/context, webviewSection == 'node') are the same set, in the same groups, except:
//      row only:  showOnCanvas
//      card only: revealInSidebar, focusBack, expand, collapse, expandAll, showCodeInCard, hideCodeInCard
// 4. every key a card's `when` clause tests is a key nodeContext() writes
```

Check 3 is "the node shares its menu with the sidebar", written as a test.

Next: [`05-build-order.md`](05-build-order.md)
