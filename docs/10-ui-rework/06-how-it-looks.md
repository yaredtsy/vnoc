# 10.06 How each state looks: selected, highlighted, cursor, revealed, focused

```
how it looks
├── 1. the five states and what draws them
├── 2. the canvas card
│   ├── one table
│   ├── when several states meet on one card
│   └── the CSS
├── 3. around the cards: path edges, the hidden-selection dot, the minimap
├── 4. the selection strip
├── 5. the sidebar
├── 6. highlight: pointing without selecting
├── 7. keys on the canvas
└── 8. checks (themes, motion, screen readers)
```

"It is hard to know which node is selected" is the loudest problem in the README. This file is the whole visual language for it, carried over from
`07-ui/03` §5 and extended for the native tree and for `highlight`. Colours come **only** from VS Code theme variables, through `tokens.css`.

## 1. The five states

| State | Meaning | Where it lives | Who sets it |
|---|---|---|---|
| **selected** | the node you are working with; menus, the doc panel and keys act on it | `contexts[c].selected` (session, saved) | a click, a focus, a reveal, a search pick |
| **highlighted** | "look here", without selecting: the menus and the doc panel do **not** follow it | `session.highlight` (session, not saved) and the canvas's own `hover` (local) | moving through the search list · hovering a crumb · hovering a node link in the doc panel |
| **cursor** | the node the text cursor is in | `session.cursor` (not saved) | moving the cursor in a code editor |
| **revealed** | "this was just brought into view" | the canvas, for about one second after it handled `session.reveal` | Show on Canvas, a sidebar click with the canvas open, a search pick |
| **focused** | the root of what is drawn | the top of `contexts[c].focus` | Focus, Back, Clear, a crumb |

A card can be in several at once (§2). Each state has a **shape** as well as a colour, so it still reads in high-contrast themes and for people who
do not see the colour difference.

## 2. The canvas card

### One table

| State | Card | Shape (not colour alone) |
|---|---|---|
| normal | border `--card-border`, kind colour on the left edge (built) | — |
| hover | border `--vscode-list-hoverBackground`; footer buttons appear | — |
| **selected** | 2 px outline in `--vscode-focusBorder`, 1 px outside the card · header background `--vscode-list-activeSelectionBackground`, text `--vscode-list-activeSelectionForeground` · drawn above its neighbours (`z-index`) | the filled header |
| **highlighted** | 2 px outline in `--vscode-editor-findMatchHighlightBorder` (fallback `--vscode-focusBorder` at 50 %) · no header fill | a softer, **dotted** outline |
| **cursor** | a 1 px **dashed** line *inside* the card in `--vscode-editorCursor-foreground` · a `▌` mark before the name · tooltip "the cursor is here" | the dashes and the mark |
| **revealed** | the selected outline pulses twice (about 1 s), then stays as the normal selected outline | the movement |
| **focused** | a `⌖ focus` chip in the header of the root card | the chip |
| syntax error | 1 px line inside the card in `--vscode-errorForeground` (today it is an outline; move it inside so the ring can sit outside) | the "syntax error" text in the body (built) |

### When several states meet on one card

The layers never use the same place, so any combination is readable:

```
 outside the card   1 px gap, then the OUTLINE: selected (solid) · or highlighted (dotted) when not selected
 the card's edge    the BORDER: normal / hover
 inside the card    the INSET LINE: cursor (dashed) · or syntax error (solid red); cursor wins, the error text stays in the body
 header             the FILL: selected · and the marks: ▌ cursor, ⌖ focus
```

| Combination | Result |
|---|---|
| selected + cursor | solid outline outside, dashed line inside, filled header with `▌` |
| selected + highlighted | selected wins (the highlight is already where you are looking) |
| highlighted + cursor | dotted outline outside, dashed line inside |
| selected + revealed | the selected outline pulses |
| focused + selected | chip and filled header |

### The CSS

```css
/* node-card.css — outlines and insets do not change the card's size, so ROW_H / CODE_H in layout.ts stay right */
.node-card { position: relative; }
.node-card.selected { outline: 2px solid var(--card-selected); outline-offset: 1px; z-index: 1; }
.node-card.selected .node-header { background: var(--card-selected-bg); color: var(--card-selected-fg); }
.node-card.highlighted:not(.selected) { outline: 2px dotted var(--card-highlight); outline-offset: 1px; }
.node-card.cursor::after,
.node-card.broken::after { content: ""; position: absolute; inset: 2px; border-radius: 4px; pointer-events: none; }
.node-card.broken::after { border: 1px solid var(--error); }
.node-card.cursor::after { border: 1px dashed var(--card-cursor); }       /* after .broken: the cursor wins */
.node-card.revealed { animation: vnoc-pulse 0.5s ease-out 2; }
@keyframes vnoc-pulse { 50% { outline-offset: 5px; } }
@media (prefers-reduced-motion: reduce) { .node-card.revealed { animation: none; } }
```

```css
/* tokens.css — the only file that names a --vscode-* variable */
--card-selected:    var(--vscode-focusBorder);
--card-selected-bg: var(--vscode-list-activeSelectionBackground);
--card-selected-fg: var(--vscode-list-activeSelectionForeground);
--card-highlight:   var(--vscode-editor-findMatchHighlightBorder, color-mix(in srgb, var(--vscode-focusBorder) 50%, transparent));
--card-cursor:      var(--vscode-editorCursor-foreground);
```

The card computes its classes from four booleans, each its own store selector so only the cards whose state changed re-render (`01-state.md` §5):

```tsx
const selected    = useSession((s) => s.contexts[s.active].selected === data.id);
const highlighted = useSession((s) => s.highlight === data.id) || useView((v) => v.hover === data.id);
const cursor      = useSession((s) => s.cursor === data.id);
const revealed    = useView((v) => v.revealed === data.id);
```

## 3. Around the cards

| Thing | Look | Why |
|---|---|---|
| **path to the selection** | the edges from the root of the view down to the selected card are 2 px in `--card-selected`; the rest stay thin | you see where the selection sits in the tree at a glance |
| **selection hidden in a closed card** | the nearest drawn ancestor shows a small filled dot (`●`, `--card-selected`) after its `+n` badge, tooltip "the selected node is inside" | after a collapse, the selection is not lost; the strip's Show opens the way to it |
| **cursor hidden in a closed card** | the same, as a hollow dot (`○`, `--card-cursor`) | the same, for the cursor |
| **minimap** | the selected card's rectangle in `--card-selected`, the cursor's in `--card-cursor` (`<MiniMap nodeColor>`) | find it in a large drawing |

The path edges come from `pathTo(root, selected)` (exists in `tree.ts`, moving to `shared/tree.ts`), memoised on the root and the selection.

## 4. The selection strip

The answer to "what is selected?" is always on screen, also when the card is scrolled away or hidden in a closed card:

```
┌──────────────────────────────────────────────────────────────────────────────────┐
│ V-NOC  my-project · 87 files · 430 functions                              ⟳   ⓘ  │
│ ←  All code › billing › Invoice                                                  │  crumbs = the focus
│ ƒ Invoice.total · src/billing/invoice.py:42          Show   ⌖ Focus   ↗ Code   ✕ │  strip = the selection
├──────────────────────────────────────────────────────────────────────────────────┤
```

| Part | Does |
|---|---|
| kind icon, name, path:line | the selected node; hidden when nothing is selected |
| Show | a reveal: open what hides it, centre it, pulse |
| ⌖ Focus · ↗ Code | the same commands as on the card |
| ✕ | clear the selection (`select null`) |
| right-click | the same menu as the card (`data-vscode-context` built by `nodeContext`) |

## 5. The sidebar

The native tree draws what VS Code lets it draw (`02-sidebar.md` §11):

| State | Row |
|---|---|
| selected | VS Code's own selected row, kept in step by `treeView.reveal(row, { select: true, focus: false })` (`02-sidebar.md` §5) |
| highlighted | **not shown**: a tree cannot mark a row without selecting it, and selecting would move the doc panel and the menus |
| cursor | a `●` badge in `--vscode-editorCursor-foreground` from a `FileDecorationProvider`, tooltip "the cursor is here"; fallback: `●` in the description (`02-sidebar.md` §7) |
| focused | the focus bar row and the FOCUS view (`02-sidebar.md` §3) |
| syntax error | `syntax error` in the description (built) |

The FOCUS view's current step has a `●` in its description.

## 6. Highlight: pointing without selecting

The web version calls this `highlightedNode`: "point at this node on the canvas", which never changes what the doc panel or the logs show. Here it
is used in three places:

| Source | When | Lives in |
|---|---|---|
| **search list** (`vnoc.search`) | the active item moves (`quickPick.onDidChangeActive`) | `session.highlight`, set by the extension. Cleared when the list closes; a pick then selects and reveals as usual. |
| **a crumb** | the pointer is over it | the canvas's local `hover` |
| **a node link in the doc panel** (later, when docs link to nodes) | the pointer is over it | the canvas's local `hover` |

Rules:

1. A highlight never changes `selected`, `focus` or `expanded`, and never opens a card.
2. If the highlighted card is drawn but off-screen, the canvas pans to it **without zooming** (`setCenter` at the current zoom, 200 ms). If it is
   hidden in a closed card, that card's dot (§3) is drawn as highlighted instead; nothing opens.
3. `select` and `focus` clear `session.highlight`. Closing the search list clears it.
4. The tree does not show it (§5).

Why the search one goes through the session and not straight to the canvas: the extension owns the QuickPick, and the session is the one way the
extension tells the canvas anything (`01-state.md` §7 rule 3). The hover ones stay local: they start and end inside the canvas.

## 7. Keys on the canvas

While the canvas has keyboard focus:

| Key | Does |
|---|---|
| `↑` `↓` | select the previous / next card in row order |
| `←` | select the parent |
| `→` | open the card if it is closed, else select its first child |
| `Enter` | focus the selection |
| `Space` | open / close the selection's children |
| `Alt+←` | focus back (skipped inside a text field or a code card: `04-menus.md` §5) |
| `Esc` | clear the selection |

A selection moved by a key is panned into view only when it leaves the screen: no re-centring on every arrow. The row order is `layout().nodes`,
which is already sorted top to bottom.

## 8. Checks

Before calling the look done:

- [ ] Dark+, Light+, and **High Contrast** (dark and light): every state in §2 can be told apart. In High Contrast, `--vscode-contrastActiveBorder`
      replaces the selected outline colour (it is defined there, empty in other themes).
- [ ] With *Reduce motion* on in the OS, nothing pulses.
- [ ] A screen reader (VoiceOver: `Cmd+F5`) reads a card as "function parse_order, selected, cursor here, 2 docs" (`aria-label` built from the same
      booleans; `aria-selected` on the card).
- [ ] 30 cards open with code, pan and zoom: still smooth with the outlines (outlines do not trigger layout; measure in *Developer: Open Webview
      Developer Tools* → Performance).

Next: back to [`README.md`](README.md)
