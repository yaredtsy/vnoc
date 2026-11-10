# Future 01.03 Showing it

```
showing a diff
├── 1. choosing what to compare
├── 2. the sidebar: a Changes tree
├── 3. the canvas: the graph, coloured
├── 4. the function diff card
├── 5. large diffs
└── 6. what VS Code gives for free
```

One rule for all of it: **a diff is a mode of the views the user already has**, not a new screen. The same tree, the same canvas, the same card, with
change marks on top. Design details (colours, sizes) belong to [`../../07-ui/02-design.md`](../../07-ui/02-design.md) when this is built.

## 1. Choosing what to compare

A small control at the top of the sidebar and of the canvas:

```
┌──────────────────────────────────────────────┐
│ Compare:  Uncommitted changes            ▾   │
├──────────────────────────────────────────────┤
│   Uncommitted changes          (default)     │
│   Staged changes                             │
│   This branch vs main                        │
│   Last commit                                │
│   Since checkpoint  "agent session 14:02"    │
│   Pick two commits…                          │
│   ──────────────                             │
│   Off                                        │
└──────────────────────────────────────────────┘
```

"Off" is the normal V-NOC. Everything below describes what is added when a comparison is on.

## 2. The sidebar: a Changes tree

Two ways to show it. Offer both with a toggle, as VS Code's own Source Control view does (tree / list).

**a) In place: the normal tree with marks**

```
▾ src                                  ● 4
  ▾ orders.py                          ● 3
    ▾ OrderService                     ● 2
        parse_order            U       body
        validate               U       signature
      old_total                D
  ▾ parsing.py                         ● 1
      parse_payload            M       ← helpers.py
    helpers.py
      fmt_money                A
```

Good for "where in the project did things change". Unchanged nodes stay visible but dim.

**b) Changes only: a flat list grouped by kind**

```
UPDATED (2)
  OrderService.parse_order        src/orders.py        body         📄 2 docs · doc may need a look
  OrderService.validate           src/orders.py        signature
MOVED (1)
  parse_payload                   src/helpers.py → src/parsing.py
ADDED (1)
  fmt_money                       src/helpers.py
REMOVED (1)
  old_total                       src/orders.py        📄 1 doc will become Missing
───
Quiet: 12 reformatted · 3 docs only                     [show]
```

Good for review: read it top to bottom, tick things off.

Letters follow git's habit so nobody has to learn them: **A** added, **D** removed, **U** updated (git uses M for "modified"; **M** is free for
*moved* here, **R** renamed). Pick the final letters with the design; the point is to be consistent with the file decorations VS Code already shows.

## 3. The canvas: the graph, coloured

The canvas is where "graph-level" becomes visible.

```
        ┌───────────────┐
        │ orders.py  ●3 │
        └───────┬───────┘
     ┌──────────┼─────────────┐
┌────▼─────┐ ┌──▼────────┐ ┌──▼─────────┐
│ Order-   │ │ old_total │ │ (rest of   │
│ Service  │ │  REMOVED  │ │  file)     │
│   ●2     │ │  ░░░░░░░  │ │  UPDATED   │
└────┬─────┘ └───────────┘ └────────────┘
  ┌──┴───────────┐
┌─▼──────────┐ ┌─▼─────────┐
│ parse_order│ │ validate  │
│  UPDATED   │ │  UPDATED  │
└────────────┘ └───────────┘
```

| Element | In diff mode |
|---|---|
| Node card | a coloured edge and a small label: added (green), removed (red, drawn as a ghost: it does not exist in the new version), updated (amber), moved (blue) |
| Parent card | the roll-up dot and count |
| Unchanged nodes | dimmed. A toggle hides them completely: **"only what changed"**, which lays out a much smaller graph |
| Moved node | drawn at its **new** place, with a faint ghost at the old place and a dashed line between them |
| Call edges (when 0.2 exists) | a new call is a green edge, a removed call a red dashed edge |
| Focus | "Focus on changes" opens the smallest subtree that contains every change |

Colour is never the only signal (labels and icons carry the same meaning), and the colours come from VS Code's theme variables for git decorations
(`gitDecoration.addedResourceForeground` and friends **[check exact names]**), so the canvas matches the user's theme.

**The call-tree view, later.** With call trees for both versions, one more question becomes answerable: *who is affected?* Select a changed function and
highlight its callers, one or two levels up. This is the "blast radius" of a change, and it is the strongest reason to do the call-graph half at all.

## 4. The function diff card

Open a changed node and its card shows **only that function**, old against new.

```
┌─ OrderService.parse_order ───────────── UPDATED body ───────────────┐
│  src/orders.py                              📄 2 docs   ⓘ           │
├──────────────────────────────────┬──────────────────────────────────┤
│ HEAD                             │ Working tree                     │
│  def parse_order(self, raw):     │  def parse_order(self, raw):     │
│ -    data = json.loads(raw)      │ +    data = self._decode(raw)    │
│      return Order(**data)        │      return Order(**data)        │
├──────────────────────────────────┴──────────────────────────────────┤
│  [Open in editor]   [Revert this function]*  [Stage this function]* │
└─────────────────────────────────────────────────────────────────────┘
                                         * later phase, see 04-resolving.md
```

- Both sides start at the function's first line and end at its last. Lines are numbered as in their own file.
- For a **moved** function the two sides come from two different files; the header shows both paths.
- For **added** / **removed** there is one side.
- Inline (one column) and side-by-side are both useful; cards are narrow, so inline is the default in a card and side-by-side in a full tab.

## 5. Large diffs

| Problem | Answer |
|---|---|
| 500 changed files | The tree first shows **files** (from git alone, instant). A file's functions are outlined when the file row is opened, or in the background. |
| 300 changed functions on the canvas | Start in "only what changed", collapsed to files. Never lay out more cards than the canvas handles today. |
| One huge function | The card shows the changed hunks with a few lines of context, with "show all". |
| Mass renames / reformatting | The **Quiet** line (§2b) keeps them out of the way with a count. |

## 6. What VS Code gives for free

> **🧩 VS Code note: three built-in pieces.**
> - **`vscode.diff` command.** `vscode.commands.executeCommand("vscode.diff", leftUri, rightUri, title)` opens VS Code's real diff editor. The two
>   sides are URIs, and a `TextDocumentContentProvider` can serve any text for a made-up scheme, for example
>   `vnoc-diff:/src/orders.py?rev=HEAD&node=<id>` returning just one function. That gives a full function diff in a tab with **no webview work**.
> - **`FileDecorationProvider`.** Puts a letter and a colour on entries of a tree view. It works on V-NOC's own sidebar tree if the tree items carry a
>   `resourceUri` **[check]**.
> - **The built-in git extension's API.** `vscode.extensions.getExtension("vscode.git")` exposes repositories, HEAD, and change events. Useful to know
>   *when* to recompute (a commit, a checkout), so V-NOC does not need its own `.git` watcher **[check what the API exposes today]**.
>
> Monaco also ships a diff editor (`monaco.editor.createDiffEditor`), which is the piece for a diff **inside a canvas card** once 0.3 has brought
> Monaco into the canvas properly.

The cheapest useful version is therefore: Changes list in the sidebar (§2b) + `vscode.diff` on click. No canvas work at all. That is phase 1 in
[`05-options-and-path.md`](05-options-and-path.md).

Next: [`04-resolving.md`](04-resolving.md)
