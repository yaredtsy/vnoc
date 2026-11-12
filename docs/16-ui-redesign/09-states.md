# 16.09 States: how each one looks, on every surface

```
states
├── 1. the states
├── 2. the canvas card
├── 3. the sidebar row
├── 4. the bar, the panels, the editor
├── 5. when several meet
└── 6. checks
```

Extends 10-ui-rework/06 (its card looks are kept; its sidebar section is replaced, the sidebar being a webview now).

## 1. The states

| State | Meaning | Kept in | Per context? |
|---|---|---|---|
| **selected** | the node the menus, keys and docs act on | `contexts[c].selected` | yes |
| **highlighted** | pointed at without selecting: hover on a row, a crumb, a search result | `session.highlight` (+ the page's hover) | no |
| **cursor** | the text cursor is inside it | `session.cursor` | no |
| **focus root** | the last step of the focus: the root of what is drawn | `contexts[c].focus` | yes |
| **open** | its children are drawn | `contexts[c].expanded` | yes, shared by both surfaces |
| **revealed** | just brought into view (one time) | `session.reveal` → the page's `pulse` | |
| **active context** | the context the canvas draws | `session.active` | |
| **loading calls** · **calls failed** | 05 §4 | `callTreeState` events | |
| **unknown callee** · **recursive** · **cut** · **incomplete** | 05 §6 | the call tree | |
| **syntax error** | the file did not parse | the structure | |
| **docs** · **link** | it has docs; found by ID / by name | the structure | |

## 2. The canvas card

Kept from 10/06 §2: selected = 2 px solid outline `--card-selected` + header fill; highlighted = 2 px dotted outline; cursor = 1 px dashed inset;
revealed = two pulses (none with reduced motion); syntax error = red inset.

| State | Look | Changed? |
|---|---|---|
| focus root | `⌖` in place of the chevron | **new** (was a `⌖ focus` chip) |
| hidden selection / cursor inside a closed card | `●` / `○` on the meta line | moved from the header |
| hover | the button under the pointer gets its hover background; nothing else on the card changes (the buttons are always shown, 04 §3) | **new** |
| call card | dashed edge in; the meta line `↳ line 16 · …` | **new** |
| unknown callee | 60% opacity | **new** |
| loading calls | spinner on the chevron; one skeleton card in the next column | **new** |
| calls failed | the meta line: *couldn't read calls · Retry* in `--vscode-errorForeground` | **new** |
| on the path to the selection | the edges 2 px `--card-selected` | kept |

The colours are `tokens.css`'s, mapped to VS Code variables: `--card-selected` = `--vscode-focusBorder`, `--card-highlight` =
`--vscode-list-focusOutline`, `--card-cursor` = `--vscode-editorCursor-foreground`.

## 3. The sidebar row

| State | Look |
|---|---|
| selected, view has keyboard focus | `--vscode-list-activeSelectionBackground` / `-Foreground`, and `--vscode-list-focusOutline` around the focused row |
| selected, view not focused | `--vscode-list-inactiveSelectionBackground` |
| keyboard focus on a row that is not selected (only after `Tab` into the tree) | `--vscode-list-focusOutline` |
| hover | `--vscode-list-hoverBackground`, the hover buttons, and the card gets the dotted outline |
| highlighted from elsewhere (a hovered crumb, a search result) | `--vscode-list-hoverBackground` |
| cursor | `●` at the end, `--vscode-editorCursor-foreground` |
| focus root | the first row of the panel, always open, no chevron (`⌖` icon in the focus bar above says it) |
| syntax error | the name in `--vscode-errorForeground`, `syntax error` as description |
| search match | the matching letters `--vscode-list-highlightForeground`, bold |

## 4. The bar, the panels, the editor

| Place | State | Look |
|---|---|---|
| canvas bar | active context chip | filled `--vscode-button-secondaryBackground`; the others outlined |
| canvas bar | loading anything | 2 px indeterminate line under the bar, `--vscode-progressBar-background` |
| sidebar panel | active | 2 px bar on the left `--vscode-focusBorder`, header bold |
| sidebar panel | EXPLORE opened by selecting a call, not activated yet | the header in italics, like a preview tab, until it is activated once |
| sidebar view | loading | VS Code's progress line on the view (`withProgress`) |
| code editor | — | V-NOC draws nothing in the editor in this redesign |

## 5. When several meet

One card or row can be selected, the cursor, the focus root and loading at once. Each state has its own channel, so none hides another:

| Channel | Card | Row |
|---|---|---|
| outside the edge | selected (solid) · highlighted (dotted; not drawn when selected) | background: selected > highlighted > hover |
| inside the edge | cursor (dashed) · syntax error (red; the cursor wins) | the name's colour: syntax error |
| the chevron's place | focus root `⌖` · spinner | spinner |
| the meta line / description | marks, hidden dots, *calls nothing*, failure | description |
| opacity | unknown callee | unknown callee |

## 6. Checks

- [ ] Dark+, Light+, High Contrast dark and light: every row of §2 and §3 can be told apart. In High Contrast `--vscode-contrastActiveBorder`
      replaces the selected outline (10/06 §8).
- [ ] *Reduce motion*: nothing pulses, spinners are static dots.
- [ ] VoiceOver reads a row as *"function checkout, selected, 1 doc, cursor here, level 3, 2 of 4"* and a call card as *"call to function
      reserve_stock, called twice, lines 16 and 31, collapsed"*.
- [ ] 100% and 200% window zoom: no text is cut in a 72 px card or a 22 px row.

Next: [`10-build-order.md`](10-build-order.md)
