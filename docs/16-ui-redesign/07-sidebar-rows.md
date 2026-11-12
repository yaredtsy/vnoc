# 16.07 Sidebar rows: one per kind

```
sidebar rows
├── 1. anatomy, per kind
├── 2. no call rows (C1)
├── 3. hover buttons
├── 4. click, double-click, and the click rule
├── 5. tooltips
├── 6. what a row tells the menu
└── 7. files
```

## 1. Anatomy

22 px high, `--vscode-font-size`, indent 16 px per level (8 px before the first), like the Explorer.

```
 ▾ 📁 app                                              folder
   ▾ 📄 api.py                              📄 1      file with a doc
     ▾ ƒ checkout                           📄 1  ●   function; ● = the cursor is in it
         ƒ _validate                                   a nested def (rare)
     ▸ ◆ Order                                        class
       ƒ health                                       function: no chevron unless it has nested defs
```

| Part | Row |
|---|---|
| chevron | it has children **in the structure** (folders, files, classes, nested defs). Never for calls. |
| icon | codicon by kind, in the kind's symbol colour (04 §5) |
| label | name; search matches bold |
| description (dim) | `📄 N` · `syntax error` |
| end of row | `●` cursor node |

## 2. No call rows (C1)

**The sidebar shows where code lives; the canvas shows what it calls.** Calls are not rows. Changed after your review of the mockup (README §5).

| Why | |
|---|---|
| the two surfaces answer different questions | the sidebar is the project's structure, as the Explorer is the disk's. A call tree inside it mixes "contains" and "calls" in one indent, and the indent is the only thing a row has to say which is which. The canvas has dashed edges for that. |
| the sidebar stays exact | without calls, every chevron in it is known: R6's optimistic chevron (05 §3) only happens on the canvas. In the sidebar, every function would otherwise get a chevron that may disappear. |
| no duplicates | a callee called from three places would be three rows plus its own row at home. On the canvas each copy has an edge that says who calls it; in a list it is just the same name again. |
| the EXPLORE panel says the rest | selecting a call card opens a panel that shows the callee **at home** (03 §4): that is the sidebar's kind of answer to "where is it". |

What this costs: following calls needs the canvas (J3). **Show Calls** on a function row opens it (08 §2), so the way there is one click.

What stays shared: open / closed (R8). A call id in `expanded` is drawn by the canvas only; the sidebar skips ids it has no row for.

## 3. Hover buttons

Shown on the hovered row and on the selected row, at the right end, over the description (as VS Code's inline actions). **The same buttons as the
card's buttons** (04 §3), from the same component (`ActionStrip`), so a user who learnt one knows the other.

| Row | Buttons |
|---|---|
| folder | `⌖` Focus · `＋` Add Doc |
| file | `⌖` Focus · `↗` Go to Code · `＋` Add Doc |
| class, function | `⌖` Focus · `↗` Go to Code · `＋` Add Doc |

`</>` Code in Card is not on a row: it changes a card, and the row has no card. It is in the menu when the canvas is open.

## 4. Click, double-click, and the click rule

| Gesture | Row |
|---|---|
| **click** | Select, then the **click rule** below |
| **double-click** | **Go to Code**, pinned (a folder: open / close) |
| chevron | open / close; no selection change |
| `Ctrl/Cmd+Enter` · `⌖` | Focus |
| right-click | Select, then the menu (08 §2) |
| click in a non-active panel | activates that context first, then as above |

**The click rule** (kept from 10/02 §4; setting `vnoc.sidebar.click`, `auto` by default):

| The canvas is… | A click on a row |
|---|---|
| **open** (in front or behind another tab) | reveal the card on the canvas, in the row's context (which is now active); bring the canvas to the front of its group **without** taking the keyboard from the sidebar. A code editor visible in **another** group follows as a preview tab. **No code tab opens over the canvas.** |
| **closed** | open the code as a **preview** tab (the definition; nothing for a folder), keyboard staying in the sidebar |

Implementation: the row sends `intent select` and then the request **`rowClick { context, id }`**; the host applies the rule (it knows whether
the canvas is open and which editors are visible). Today's `vnoc.open` command keeps doing the same for the palette and tests.

**When the selection is a call** (a call card was clicked), no row in that panel is selected: the call has no row. The EXPLORE panel that opens
under it selects the callee, at home.

## 5. Tooltips

A hover card after VS Code's delay (`workbench.hover.delay`, 500 ms by default), in `--vscode-editorHoverWidget-*` colours:

> **checkout** · function · `app/api.py:11` · 1 doc · *ID in the code* / *found by name only*

## 6. What a row tells the menu

The row's element carries `data-vscode-context`, built by the **same** `nodeContext()` as a card, with `surface: "sidebar"` and `role: "node"`
(08 §6). VS Code collects it on a right-click, shows the entries of `webview/context` whose `when` matches, and passes the object to the command.

## 7. Files

| File | |
|---|---|
| `webview/src/sidebar/Row.tsx` | §1, §4 |
| `webview/src/node-card/ActionStrip.tsx` | §3, shared with the card |
| `webview/src/node-card/nodeContext.ts` | gains `surface`, `role`, `callId`, `calleeKnown`, `sites` (the last three for cards) |
| `extension/src/commands.ts` | `rowClick` (from today's `open`); `vnoc.open` calls it |
| `shared/messages.ts` | request `rowClick { context: ContextId; id: string }` |

Next: [`08-menus.md`](08-menus.md)
