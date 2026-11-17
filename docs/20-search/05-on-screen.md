# 20.05 On screen

```
on screen
├── 1. the sidebar      two groups, the doc row, what a pick does, the newest answer wins
├── 2. V-NOC: Search…   puts the cursor in the box (no QuickPick)
├── 3. the canvas       dims what is not a hit
└── 4. what stays TypeScript
```

[`../16-ui-redesign/06-sidebar.md`](../16-ui-redesign/06-sidebar.md) §5 stays, with the changes below.

## 1. The sidebar

**Asking.** On each change of the box (no debounce: decided after S3, below),
the page sends `search { query }` to the host, which calls `vnoc/search`. Each request gets the page's next number; **an answer whose number is
not the newest is dropped** (Q8). The `revision` is not compared (Q5: a restarted core counts from the start again). An empty box asks nothing and
shows the panels. When the tree changes while the box has text (`indexChanged`), the page asks again. While an answer is on its way the rows of the
last one stay (no flicker); "No matches." only when the answer for the box's text has no rows.

**Measured** (S3, 2026-11-17; the window suite `search`, release core, Apple M1, macOS 14.7.6, in VS Code's extension host; `ms` is from the change of the box to the
draw, `trip` from sending the request to the draw, both read in the page with `performance.now()`). The run that was committed (the whole suite, 11 answers): `ms`
median **60.7 ms**, worst **155 ms**; `trip` median **10.0 ms**, worst **92 ms**. The suite alone (12 and 9 answers in two runs): `ms` median **57.0** and **56.4 ms**,
worst **82** and **87 ms**; `trip` median **5.0** and **5.1 ms**, worst **30** and **31 ms**. So `ms` is the 50 ms debounce plus `trip`: the round trip (page, host, core, and
back) is about **5 ms** when the view is on screen, and the worst ones (30 – 90 ms) are the first answer after a pause. **The debounce is not changed**; the planner decides from
`trip`: without it, key to drawn would be about 5 – 10 ms, and a key typed in the middle of a word would ask the core for a query nobody finished. **A page VS Code calls hidden is
slow**: with the STRUCTURE view hidden (a suite that ran before had left it so) the same answers took 1 – 7 s (`trip` median 965 ms), because the page's timers and drawing are
held back; nothing can be typed in a hidden box, so this is for the window suite (it shows the view first), not for a user.
**Decided** (2026-11-17, after S3): **no debounce.** `trip` is about 5 ms, under a frame, so waiting 50 ms only made every answer 50 ms
later. A query nobody finished costs the core one `run`, about 2 ms on 10,000 nodes (04 §2): cheaper than the wait. The page asks on each change of the box; with no pause, answers to fast keys do cross, and the request numbering (Q8) is what drops the
old ones, so it is a pure function with a unit test (S3b).

**Measured again without the debounce** (S3b, 2026-11-17; the same suite, Apple M1, three runs: 16 answers with the `sidebar` suite before it, and 13 and 13 alone): `ms` (box to drawn)
median **7.2**, **5.6** and **6.5 ms**, worst **30.0**, **32.2** and **21.1 ms**; `trip` median **6.0**, **4.7** and **5.3 ms**, worst **28.1**, **22.2** and **17.8 ms**. Key to drawn is now
about the round trip (the 50 ms was most of it before: 57 – 61 ms median), under a frame, and the worst ones are still the first answer after a pause. Each key asks the core, and the numbering
(`shared/search.ts`, `newest`) drops the answers that are not the newest; the window suite types the keys apart by 120 ms and in one go, and only the newest query's answer is drawn.

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

## 2. *V-NOC: Search…* is the box

*V-NOC: Search…* (`vnoc.search`, the id stays) shows the STRUCTURE view and puts the cursor in its search box (view command `focusSearch`). A
string argument is the text to start with (`executeCommand("vnoc.search", "outer.meth")`); the window tests use it, and any later caller can.
The command waits until the page has drawn once (an event sent before it listens is lost), at most 5 s.

**Why not a QuickPick** (settled 2026-11-17, S3b): VS Code re-sorts a QuickPick's items by its own matcher even when every item has
`alwaysShow: true`. Walked on `e` (18 rows), the core's order began `inner Outer other method…` and VS Code showed `decorated deepest inner…`.
There is no stable API to turn the sort off (`sortByLabel` is not in `@types/vscode`). So the palette's list is gone (`extension/src/search.ts`
deleted in `6f8b188`) and there is one search surface: also one less to build in the next editor.

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
