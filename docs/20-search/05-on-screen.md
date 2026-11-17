# 20.05 On screen

```
on screen
├── 1. the sidebar      two groups, the doc row, what a pick does, the newest answer wins
├── 2. the QuickPick    the core's order, not VS Code's
├── 3. the canvas       dims what is not a hit
└── 4. what stays TypeScript
```

[`../16-ui-redesign/06-sidebar.md`](../16-ui-redesign/06-sidebar.md) §5 stays, with the changes below.

## 1. The sidebar

**Asking.** On each change of the box, after a short debounce (50 ms **[measure]** in S3: drop it if key-to-drawn is under a frame without it),
the page sends `search { query }` to the host, which calls `vnoc/search`. Each request gets the page's next number; **an answer whose number is
not the newest is dropped** (Q8). The `revision` is not compared (Q5: a restarted core counts from the start again). An empty box asks nothing and
shows the panels. When the tree changes while the box has text (`indexChanged`), the page asks again. While an answer is on its way the rows of the
last one stay (no flicker); "No matches." only when the answer for the box's text has no rows.

**For the tests** (S3): the view command `setSearch { text }` puts text in the box as typing would, and the page posts a note
`searchDrawn { query, ids, total, ms }` each time it draws an answer (`ms`: from the change of the box to the draw), as the canvas posts `codeShown`.

**The list.** Two groups, **Nodes** then **Docs**, each with a small header and its count: `Nodes 12`, or `Nodes 200 of 1,340` when the limit cut
it. A group with nothing is not drawn. `↑` `↓` run through both groups as one list.

| Row | Shows |
|---|---|
| a node | as today: the kind's icon (a method's for `in_class`), the name with its `match` ranges bold, the path dim |
| a doc | the doc icon, the title with `title_match` bold, the node's name dim on the right, and the snippet as a second, dim line with `snippet_match` bold. A Missing node's doc has the Missing mark |

A line of `notes` (01 §2), when there are any, sits under the box in the dim colour: `kind:fnn is not a kind (folder, file, class, function,
method)`.

| Pick | Does |
|---|---|
| a node | as today (`pickIntent`: reveal in the active context, then the click rule) |
| a doc of a node in the tree | reveal the node, and open that doc in the doc panel |
| a doc of a Missing node | open the `.md` in a tab (the node has no card) |

## 2. The QuickPick

*V-NOC: Search…* (`vnoc.search`, the id stays): calls `vnoc/search` on each change of its value, shows nodes, then a separator, then docs (the
snippet as the item's `detail`).

An empty value lists nothing (the core finds nothing for no words); the placeholder says what to type. The command takes an optional string,
the value to start with (`executeCommand("vnoc.search", "outer.meth")`): the window tests use it, and any later caller can.

VS Code filters and re-sorts a QuickPick's items by its own fuzzy matcher. The items must be shown **in the core's order, unfiltered**. Giving
every item `alwaysShow: true` and setting `items` again on each `onDidChangeValue` is the known way **[check]** in S0: that the order given is
the order shown while the box has text.

**If the check fails:** *V-NOC: Search…* shows the sidebar and puts the cursor in its box (`focusBox` exists), and `extension/src/search.ts` is
deleted. One search surface is also less to build in the next editor, so this is a fine ending, not a loss.

## 3. The canvas dims

While the sidebar's box has text, the cards the query finds stay as they are; **every other card is dimmed** (opacity 0.35 **[decide by eye]**).
The active result keeps today's dotted highlight. Nothing moves, nothing opens, the focus does not change.

- The session gets one field, `search: string | null` (the query; `null` when the box is empty), set by an intent from the sidebar. It is the
  only thing that crosses from one page to the other.
- The canvas, when `search` is not `null`, asks `searchCards { query, ids }` with the ids of the cards it shows: a structure card's node id, a
  call card's target key (a call to something outside the index has no key: it is dimmed). It asks again when the query, its cards or the
  revision change (debounced 100 ms), and keeps the newest answer only.
- Until the first answer for a query arrives, nothing is dimmed (no flash of everything going dark).
- A query of one letter dims too: the answer is exact (Q12), so there is no reason to wait for a second letter.
- For the tests the canvas posts a note `searchDim { key, dimmed }`, as it posts `codeShown`.

## 4. What stays TypeScript

Drawing and keys, by P1's own line: `searchKey` (what `↑` `↓` `Enter` `Esc` mean), `splitMatch` (a name cut around ranges, now several),
`pickIntent`, the debounce, the request numbering. `rankNodes`, `matchRange`, `indexNodes` and the `Tier` type go; so do their tests, whose rows
are in Rust from S1.
