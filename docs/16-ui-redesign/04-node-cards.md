# 16.04 Node cards: the structure card and the call card

```
node cards
├── 1. the structure card
├── 2. the call card
├── 3. the buttons (always shown) and the footer (the counts)
├── 4. every part, its use case, and what a click does
├── 5. look: type, colour, icons
├── 6. what a card needs from the layout (Placed)
└── 7. files
```

Calls are nodes (R5): a call card is the same component as a structure card, with a different meta line and different actions. One look, one
set of states ([`09-states.md`](09-states.md)).

## 1. The structure card

260 × 72 px (today 240 × 80, and 12 px text): three rows, always all three, so every card has one height.

```
┌────────────────────────────────────────────┐
│ ▸  ƒ  checkout_order         ⌖  ↗  </>  ＋ │  header, 30 px: the buttons are always there
│    app/order.py:24                         │  meta, 22 px
│    +3   📄 2  🔗                           │  footer, 20 px: the counts, each with a tooltip
└────────────────────────────────────────────┘
 ▲ 3 px stripe in the kind's colour
```

| Header, left to right | When |
|---|---|
| chevron `▸` / `▾` | it has children, **or** it is a function whose calls are not known yet (05 §3). The root card of the view has `⌖` here instead (it is always open; `⌖` says "the focus"). |
| kind icon | always (§5) |
| name | always; a function `checkout_order`, a class `Order` (no `()` and no "File " prefix). Cut at the end when long, full name in the tooltip |
| the buttons | always (§3) |

| Meta line | When |
|---|---|
| `path:line` for a def, `path` for a file, `path/` for a folder, mono, dim | always. When it does not fit, it is cut **at the start**, never the end: `…/inventory.py:40`. The file name and the line are what you read; the folders are in the tooltip |
| `syntax error` in `--vscode-errorForeground` | the file did not parse |
| `calls nothing` (right side) | a function whose calls are known and empty |

| Footer, left to right | When | Tooltip |
|---|---|---|
| `+N` | closed and N > 0 (defs + calls, when calls are known) | *"3 hidden: open to see them"* |
| `📄 N` | N > 0 | *"2 docs: click to read"* |
| `🔗` | it has docs: solid = ID in the code, faded = found by name only (kept from today) | *"ID in the code"* / *"found by name only: Write ID to keep it on renames"* |
| `●` / `○` | a closed card hides the selection / the cursor (kept from 10/06) | *"the selection is inside"* / *"the cursor is inside"* |

An empty footer stays (blank). The cards keep one height, so a count that appears (a new doc) moves nothing on the canvas.

Gone: the `⌖ focus` chip (the bar's crumbs and the `⌖` in place of the chevron say it), the cursor `▌` (the dashed inset line says it, 09),
the old footer's path (it is the meta line now).

## 2. The call card

The same size and rows. It stands for **one callee, called from its parent card**; the calls a function makes to one callee are merged into it
(14 T2, §2.1).

```
┌────────────────────────────────────────────┐
│ ▸  ƒ  reserve_stock               ◇  ↗  </> │
│ ↳ line 16 · line 31           inventory.py │
│    ×2   +4   📄 1                          │
└────────────────────────────────────────────┘
       ┄┄┄ dashed edge from its caller
```

| Part | Shows |
|---|---|
| chevron | it has calls of its own (exact: the tree is known), or it is `cut` (opening it asks for more, 05 §4). None when `recursive`. |
| kind icon | the **callee's** kind, as on a structure card: `ƒ` function (`symbol-function`), `◆` class (`symbol-class`, a constructor call). **Never** the call icon: `call-outgoing` is only the `↳` in front of the sites. The dashed edge and the `↳` already say "a call"; the icon says what is called, so the stripe's colour and the icon always agree |
| buttons | §3 |
| meta, left: **the call sites**, `↳ line 16 · line 31` | each a link to **that** site (§2.1). In another file than the caller's: `base.py:12` (14 §10.5). That only happens under a **class** caller (an inherited method's call, 14 §10.1); a function's sites are all in its own file. More than two: `↳ line 16 · line 31 · 3 more ▾`. |
| meta, right: **where the callee lives**, file name only, dim, no `:line` | so you know before exploring. Shown **whole or not at all**: the sites win the room, and when the name does not fit it is hidden, never cut (`payme…` says nothing and looks like a site). The full path is always in the name's tooltip and the docs panel's header |
| a mark instead of the file name | `↻ recursive` · `… calls not loaded` (cut) · `… incomplete`, dim like the file name, each with a tooltip that says what to do (05 §6). None of them is a problem, so none is in a warning colour |
| footer: `×N` | merged calls, N > 1. Tooltip: *"checkout_order calls it 2 times: lines 16, 31"* |
| footer: `+N`, `📄 N` | as on a structure card; docs are the **callee's** |

A call whose callee the project does not know (`key: null`, 05 §6): 60% opacity, no docs, no buttons, meta right says *"not in the index"*.

### 2.1 Merged calls and their sites (C2)

The core merges **sibling** calls to one callee into one node and keeps **every** site, each with its file, sorted and without repeats
(`crates/vnoc/src/calls.rs` `merge_group`; 14 T3, §10.4). So `checkout_order` calling `reserve_stock` on lines 16 and 31 is **one** card, `×2`,
with both sites. Nothing is lost by merging; what has to be right is how a site is reached.

| Rule | Why |
|---|---|
| **one card per callee per caller** (the merge, unchanged) | two cards with the same name under one caller say nothing a `×2` does not |
| the same callee under **two different callers** stays two cards | each is a different edge: "A calls it" and "B calls it". The tree repeats a callee once per caller, never per call. |
| **every site is its own link** on the card's meta line | a click goes to *that* line, never "the first one" |
| more than two sites: `3 more ▾` opens a list **inside the card**: one line per site, `line 40  stock = reserve_stock(item, qty)`, the line's text read by the host (the `code` request, one line each) | you see which call is which before you jump |
| **no single "Go to Call Site" button** among the card's buttons | on a merged node a button cannot know which site you mean; on a one-site node the meta link is already one click |
| the menu's **Go to Call Site** and `Ctrl/Cmd+Enter`: one site → go there; several → a QuickPick of the sites with their line text, and **moving through the list previews each site** in the editor beside the canvas (`preserveFocus`), as *Go to References* does | the keyboard way, and the way from the menu |
| a site with no file (an older driver, 14 §10.3) | listed as `line 16 ?`, not a link, tooltip *"the driver did not say which file"* |

## 3. The buttons

At the right end of the header, **always shown** (not only on hover). Icon buttons, 22 px, in `--vscode-descriptionForeground` at rest and
`--vscode-foreground` with `--vscode-toolbar-hoverBackground` under the pointer, each with a tooltip that names its key. Nothing on the card
appears, moves or hides on hover: the buttons and the counts are always in the same place.

| Card | Buttons |
|---|---|
| folder | `⌖` Focus · `＋` Add Doc |
| file | `⌖` Focus · `↗` Go to Code · `＋` Add Doc |
| class, function | `⌖` Focus · `↗` Go to Code · `</>` Code in Card · `＋` Add Doc |
| call (known callee) | `◇` Explore · `↗` Go to Code · `</>` Code in Card |
| call (unknown callee) | none: its sites are on the meta line |

| Cost | How it is kept small |
|---|---|
| four buttons take ~92 px of the header, so a name gets ~120 px (about 14 characters at 13 px) | the name is cut at the end with its tooltip; the dim colour keeps the buttons quieter than the name |
| many cards, many icons | dim at rest; the selected card's are full colour |
| the card is 16 px taller than with a hover strip | it keeps every count on screen without a hover; the canvas zooms |

The structure cards' buttons are also a sidebar row's hover buttons (07 §3), from the same `ActionStrip` component. A **row** keeps VS Code's
list behaviour (inline actions on the hovered and the selected row, over the description), because a 22 px row has no room for both. Four at
most. There is no Go to Call Site button (§2.1): the sites on the meta line are the buttons.

## 4. Every part: use case and click

| Part | Job | Click | Double-click |
|---|---|---|---|
| card body | select it | **Select** (in the card's context) | structure: **Focus** · call: **Explore** |
| chevron | J1, J3: one level more | `toggle` (calls not known: fetch, then open, 05 §4) | — |
| footer `📄 N` | J6 | Select + open the docs panel (`ⓘ`) on it | — |
| footer `+N` | J1 | the same as the chevron: open it | — |
| footer `×N`, `🔗`, `●`, `○` | read only | nothing (the tooltip explains it) | — |
| `↳ line 16` | J5: "where is it called?" | that site, in the editor (beside the canvas when it is in front) | — |
| `3 more ▾` | J5 | the list of every site in the card (§2.1) | — |
| `⌖` | J2 | Focus | — |
| `◇` | J4 | Explore (activated) | — |
| `↗` | J5 | Go to Code (the definition; for a call, the callee's) | — |
| `</>` | J5 without leaving the canvas | show / hide the code in the card (+220 px, between the meta line and the footer). For a call: the callee's code. | — |
| `＋` | J6 | Add Doc… | — |
| right-click | everything else | Select, then the node menu ([`08-menus.md`](08-menus.md) §2) | — |

Every button has `nodrag nopan` (React Flow must not start a drag), and a click on a button does not also select twice.

## 5. Look

| Thing | Value | Why |
|---|---|---|
| name | `--vscode-font-family`, `--vscode-font-size` (13 px), weight 600 | the size of the rest of VS Code; today's 12 px / 11 px is what makes the screenshot hard to read |
| meta | `--vscode-editor-font-family`, 11 px, `--vscode-descriptionForeground` | |
| footer | `--vscode-font-family`, 11 px, `--vscode-descriptionForeground`; `📄 N` and `+N` are buttons (hover background) | the counts read as quiet facts, the buttons above as actions |
| background · border | `--vscode-editorWidget-background` · `--vscode-editorWidget-border` | a card looks like a VS Code widget in every theme |
| kind stripe and icon colour | `--vscode-symbolIcon-folderForeground`, `-fileForeground`, `-classForeground`, `-functionForeground` | the colours the Outline view uses for the same kinds. The stripe says **what the node is**, never a state: an orange stripe is a class, not a warning |
| warning / error colours | `--vscode-errorForeground` for `syntax error` only. No mark on a card uses the warning colour | yellow and red mean "something is wrong" everywhere in VS Code; a cut tree or a recursion is not |
| icons | **codicons** (`@vscode/codicons`): `folder`, `file-code`, `symbol-class`, `symbol-function` (`symbol-method` inside a class), `call-outgoing` for `↳` only (never as a card's kind icon), `target` `⌖`, `link-external` `↗`, `code` `</>`, `add` `＋`, `compass` `◇` | the sidebar and the card use the same font of icons, the same as VS Code's own views. Replaces `@tabler/icons-react` and the emoji in `kinds.ts`. |
| radius | 4 px | VS Code's widgets |

## 6. What a card needs from the layout

```ts
// webview/src/canvas/layout.ts
export type Placed = {
  id: string;                        // a structure key or a call id
  role: "node" | "call";             // NEW
  key: string | null;                // NEW: the structure node it shows (the callee for a call); null for an unknown callee
  kind: Kind; name: string; path: string; line: number | null; range: Range | null;
  depth: number; x: number; y: number;
  codeOpen: boolean; collapsed: boolean; syntaxError: boolean; docs: number; link: TreeNode["link"];
  /** structure children + calls; null while the calls of a function are not known (the chevron shows, `+N` does not) */
  childCount: number | null;         // CHANGED: was number
  /** "unknown" | "loading" | "known" for a function's or a call's calls; null for folders, files, classes */
  calls: "unknown" | "loading" | "known" | null;   // NEW
  /** a call card only */
  call: { count: number; sites: OpenableSite[]; callerPath: string | null; marks: CallMark[] } | null;  // NEW
};
// gone: callRows
```

`nodeContext()` (the card's `data-vscode-context`) gains `role`, `calleeKnown`, `sites` (a count) and `callId`; [`08-menus.md`](08-menus.md) §6.

## 7. Files

| File | Change |
|---|---|
| `webview/src/node-card/NodeCard.tsx` | header, meta by `role`, the footer (§1, §2), no `NodeCalls` |
| `webview/src/node-card/NodeHeader.tsx` | §1 and §2 headers |
| `webview/src/node-card/NodeMeta.tsx` (new, replaces `NodeBody.tsx`) | the meta line |
| `webview/src/node-card/NodeFooter.tsx` | rewritten: the counts of §1 and §2 with their tooltips (the old footer's path and `+` go) |
| `webview/src/node-card/ActionStrip.tsx` (new) | §3: the buttons; shared with the sidebar row (it takes the node and the context). The card shows it always; the row on hover and selection |
| `webview/src/node-card/NodeCalls.tsx`, `canvas/calls.ts`'s `callRows` | **deleted** (`siteText`, `listedSites`, `markBadge`, `isStale` move to `shared/calls.ts`) |
| `webview/src/node-card/kinds.ts` | codicon names instead of emoji |
| `webview/src/node-card/node-card.css` | §5; the `.node-calls` rules go |
| `webview/package.json` | `+ @vscode/codicons`; `- @tabler/icons-react` once nothing uses it |

Next: [`05-calls-as-nodes.md`](05-calls-as-nodes.md)
