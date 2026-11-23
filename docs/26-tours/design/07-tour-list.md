# 07 The tour list: seeing the tours, the ones on a node, opening one

```
tour list
├── 1. the jobs, and the decisions
├── 2. one TOURS section: the list, or the route of the open tour
├── 3. a tour's row
├── 4. two views: All, and On the selected node
├── 5. opening a tour: it starts at its root
├── 6. from a node to its tours
├── 7. asking for a tour, deleting one
├── 8. empty and other states, keys
└── 9. what it needs: the session, the file, the mockup
```

Added 2026-11-15, after the first mockup. Until here a finished tour could only be found through a palette list (`Open Tour…`). That is
not enough: tours pile up, and the question is usually "is there one about **this**?".

How a tour is made does not matter here. The agent ran `vnoc walk start` from a terminal, or an MCP tool did: either way a file appears in
`.vnoc/cache/walkthroughs/`, and **the list is that folder, live**. A tour is in the list from its first second, while it is being written.

## 1. The jobs, and the decisions

| # | Job | Example | Where |
|---|---|---|---|
| **T10** | **Find a tour I have** | "the one about checkout from this morning" | the list, All |
| **T11** | **Is there a tour about this?** | "I am on `reserve_stock`: has anything explained it?" | the list, On the selected node; `▶ 3` on its card |
| **T12** | **Pick up where I left** | "I was halfway through the tour of `app/`" | the row says `stop 4/9`; a click goes there |
| **T13** | **Ask for one about this** | "nothing explains `gateway.py`" | `＋` copies the sentence for your agent |
| **T14** | **Throw old ones away** | | the row's delete, with Undo |

| # | Decision | Why |
|---|---|---|
| **W14** | **One TOURS section in the sidebar. It shows the list, or the route of the open tour, never both** | a list of tours and a tour's stops are two levels of one thing; two panels for them would be two places to look. Replaces "the tour's panel is stacked like a context panel" (04 §1) only in where the route sits |
| **W15** | **Two views: All, and On *the selected node*.** The second follows the selection and says **why** each tour is in it | "filter by the selected node" without a filter form: you select, the list answers |
| **W16** | **A tour opens at its root**: the cover, the map fitted from its first stop. Opened before: where you left it. Opened from "On a node": at that node's stop | the cover is the tour's own focus on its root; the other two are the reasons you clicked |
| **W17** **ask** | **A node that is on tours shows `▶ N`** in its card's footer and its row, as `📄 N` does for docs | the answer to T11 is on the node before you ask. The cost is one more fact on the card |
| **W18** | **`＋` Ask for a Tour… copies a sentence for the agent.** It does not start anything | in basic a click cannot reach the agent. The button is already where the real one will be |

## 2. One TOURS section

In the sidebar's webview, under the context panels, above NEEDS ATTENTION. It folds like a view.

**Folded** (most of the time, when you are not on a tour):

```
│ ▸ TOURS  5                                        │   the count; a dot pulses beside it while the agent writes one
```

**The list** (no tour is open, or you clicked `‹ Tours`):

```
┌ STRUCTURE ─────────────────────────────── ⊞  ⊟  ⋯ ┐
│ Search nodes…                                     │
│ ⌖  minicart › app › inventory.py            ←  ✕  │
│ ▾ inventory.py                                    │
│     ƒ check_stock                           ▶ 2   │
│     ƒ reserve_stock                         ▶ 3   │   selected in MAIN · it is on 3 tours (W17)
│     ƒ release_stock                         ▶ 1   │
│ ═══════════════════ drag ════════════════════════ │
│ ▾ TOURS                                     ＋  ⋯ │   Ask for a Tour… · more
│ [ All 5 ]  [ On reserve_stock 3 ]                 │   the two views (§4)
│ ƒ checkout_order                     writing 6/10 │   being written: you can open it already
│   10 stops · today 18:40                          │
│ ƒ checkout_order                         finished │
│   6 stops · today 11:05 · "what happens when th…" │   its question, when it was asked with one
│ □ app/                                   stop 4/9 │   you left it on stop 4
│   9 stops · yesterday                             │
│ ƒ reserve_stock                           changed │   its code changed since it was written
│   1 stop · 12 Nov                                 │
│ ƒ checkout                                    new │   never opened
│   2 stops · 10 Nov                                │
└───────────────────────────────────────────────────┘
```

**The route** (a tour is open): the panel of [04](04-route-and-map.md) §1, with one thing added to its header.

```
│ ▾ TOURS                                     ＋  ⋯ │
│ ┃ ‹ Tours  ▶ checkout_order           3 of 10   ✕ │   ‹ Tours: back to the list. ✕: close the tour
│ ┃ ✓  1  ƒ checkout_order                          │
│ ┃ ✓  2    ƒ check_stock                           │
│ ┃ ●  3    ƒ reserve_stock                         │
│ ┃ …                                               │
```

| Gesture | Result |
|---|---|
| a tour appears or is opened | the section unfolds and shows its route |
| `‹ Tours` | the section shows the list. **The tour stays open**: its chip is on the bar, the canvas does not change, and its row in the list has the bar `┃` of the active panel. A click on that row goes back to its route |
| `✕` on the route's header · **Close Tour** | the tour's context closes ([05](05-states.md) §2); the section shows the list |
| fold the section | the tour stays open; the stage and the map are enough to read it |

The section never unfolds by itself except for a tour you opened or one that just appeared with the canvas visible (W11).

## 3. A tour's row

Two lines, 40 px: a tour is a thing you choose, and one line cannot say which is which.

```
│ ƒ checkout_order                     writing 6/10 │   line 1: the root (kind icon, name) · the state
│   6 stops · today 11:05 · "what happens when th…" │   line 2, dim: size · when · the question
```

| Part | Shows |
|---|---|
| kind icon, **name** | the tour's first stop: what the tour is of. A folder ends with `/` |
| **the state**, at the right | **one** word, the first that applies: `writing 6/10` · `stopped 6/10` · `no news` · `changed` · `stop 4/9` (you are in it) · `finished` (you saw every stop) · `new` (never opened) |
| size | `10 stops`. The minutes are on the cover, not here |
| when | when it was started: `today 18:40` · `yesterday` · `12 Nov` |
| the question | the tour's `query` in quotes, cut at the edge. It is what tells two tours of one node apart. Without one, nothing: the depth is in the tooltip |
| tooltip | everything: *"Tour of checkout_order, and what it calls, and what those call. 10 stops, about 7 min. Started today 18:40, for the code at f4d6e15. You are on stop 3."* |
| on hover | `🗑` Delete; on a tour being written `■` Stop instead (a tour is not deleted under the agent) |

| Rule | Why |
|---|---|
| newest first, by when it was started. The order does not change when you open one | a list that reorders under the pointer is hard to click |
| no step text, no summary of the tour | rule 4 of 01 §2: lists show names. The tour says what it says when you open it |
| the open tour's row has the left bar `┃` and a bold name | as the active panel does |

## 4. Two views

A line of two buttons under the section's header. The choice is kept.

| View | Shows | Named |
|---|---|---|
| **All** | every tour in the cache | `All 5` |
| **On the selected node** | the tours that have something to do with the **active context's selection** (a selected call counts as its callee). It follows the selection: select another node, the list changes | `On reserve_stock 3`. With nothing selected: `On selection`, disabled, tooltip *"Select a node to see its tours"* |

A tour is **on** a node in one of three ways, and in this view line 2 of its row says which:

| The selected node is | Line 2 | A click opens the tour at |
|---|---|---|
| the tour's first stop | `starts here` | the cover |
| a stop of the tour | `stop 3 of 10` (several: `stops 2 and 4 of 10`) | that stop; the first of them |
| a folder, file or class with stops of the tour inside it | `4 stops inside` | the first of those stops |

`reserve_stock` selected:

```
│ [ All 5 ]  [ On reserve_stock 3 ]                 │
│ ƒ checkout_order                     writing 6/10 │
│   stop 3 of 10                                    │
│ ƒ checkout_order                         finished │
│   stop 3 of 6 · "what happens when the payment…"  │
│ ƒ reserve_stock                           changed │
│   starts here                                     │
```

`inventory.py` selected:

```
│ [ All 5 ]  [ On inventory.py 4 ]                  │
│ ƒ checkout_order                     writing 6/10 │
│   4 stops inside                                  │
│ ƒ checkout_order                         finished │
│   2 stops inside · "what happens when the paym…"  │
│ □ app/                                   stop 4/9 │
│   stop 5 of 9                                     │
│ ƒ reserve_stock                           changed │
│   starts inside                                   │
```

- The counts on the two buttons are always shown, so in **All** you already see that three of the five are about what you selected.
- There is no text filter in basic: the cache holds tens of tours, not thousands. The palette's **Open Tour…** filters by typing.
- The view does not change what the canvas draws. It is a list.

## 5. Opening a tour

One click on a row (or `Enter`). The tour's context opens and the canvas draws it (W2).

| Opened from | It opens at |
|---|---|
| **All**, a tour you never opened, or one you finished | **the cover**: the tour's context is focused on the tour's first stop, the bar's crumbs show it, the map is fitted from it to the whole route, the stage shows the size and `Start ›` ([02](02-flow.md) frame 2) |
| **All**, a tour you left halfway (`stop 4/9`) | where you left it, with the seen marks you had. **Start Again** in the row's menu and on the bar's `⋯` goes to the cover and clears the marks |
| **On a node** | that node's stop (§4). The cover is one `Home` away |
| the notification, the status bar item | as from All |

| Case | Rule |
|---|---|
| the canvas is hidden | it opens, beside the editor. Opening a tour is your request; a tour *appearing* opens nothing (W11) |
| another tour is open | it is replaced. Its place is kept: its row now says `stop 3/10`, and a click brings it back there |
| the tour is being written | it opens like any other. You read behind or ahead of the agent ([02](02-flow.md) frame 6) |
| the tour is *changed* | it opens, and says so on its cover ([05](05-states.md) §4) |
| MAIN | untouched. To put MAIN on the tour's root instead: the row's menu, **Reveal Root in Main** |

"It starts with the focus on the root" is therefore the tour's own context doing what Focus does in MAIN, without changing MAIN: when you
close the tour you are where you were.

## 6. From a node to its tours

| Where | What | Click |
|---|---|---|
| a card's footer, a row's right side | **`▶ 3`** when the node is a stop of three tours (W17). Counted from the cache's tours; a folder, file or class counts the tours with stops inside it | select the node, unfold TOURS, switch to **On the selected node** |
| the node menu (card and row) | **Show Tours** (when it has some) · **Ask for a Tour…** | the same · §7 |
| a tour card on the map | nothing new: you are already on a tour | |

```
┌──────────────────────────────────────────────┐
│ ▸  ƒ  reserve_stock            ⌖  ↗  </>  ＋ │
│       line 19                                │
│ ──────────────────────────────────────────── │
│       +1   1 doc   ▶ 3                       │   ▶ 3: tooltip "on 3 tours: click to see them"
└──────────────────────────────────────────────┘
```

So T11 takes no step when the list is in **On the selected node**: select a node anywhere (the canvas, the structure, the editor's
**Reveal in Sidebar**) and the list is already its tours.

## 7. Asking for a tour, deleting one

| Control | Does |
|---|---|
| **`＋` Ask for a Tour…** (the section's header, the node menu, the empty states) | copies a sentence for the agent, with the selected node's address: `Walk me through app/inventory.py::reserve_stock`. A message says *"Copied. Paste it to your agent."* With nothing selected it copies `Walk me through ` and says *"Add what you want explained."* |
| a row's menu → **Ask for a New Tour** | the same, with that tour's depth and question: for a *changed* tour, "make this again" |
| **`🗑` Delete** (hover, the row's menu, `Delete`) | the row becomes *"Deleted the tour of checkout_order. Undo"* for six seconds, then goes. No dialog |
| the section's `⋯` | **Delete Finished Tours** · **Delete All Tours…** (this one asks) |
| a row's menu | **Open** · **Start Again** · **Reveal Root in Main** · ─ · **Ask for a New Tour** · **Delete** |

When a click can reach the agent (phase 1's requests, or the extension starting the harness), `＋` sends the same sentence instead of copying
it. The button, its place and its wording stay.

## 8. Empty and other states, keys

| State | The section shows |
|---|---|
| no tours at all | *"No tours yet. A tour is your agent explaining a piece of the code, step by step."* · **Copy a Request for `reserve_stock`** (the selection; without one: **Copy a Request**) |
| **On the selected node**, none | *"No tour goes through `gateway.py`."* · **Ask for One** · **Show All 5** |
| a tour's file cannot be read (a newer schema, a broken file) | its row, dim: `cannot be read`, with only Delete |
| the cache is being read (first start) | VS Code's progress line on the view; no skeleton rows |

| Key, in the list | Does |
|---|---|
| `↑` `↓` | move the row focus |
| `Enter` · `Space` | open |
| `Delete` / `⌘⌫` | delete, with Undo |
| `←` `→` on the two view buttons | switch the view |

**V-NOC: Open Tour…** in the palette is the same list as a QuickPick, for the keyboard: typing filters by name and question; with a node
selected, its tours come first under a line *"on reserve_stock"*.

## 9. What it needs

**The session.** A tour's place can no longer live in the tour's context (04 §8): a tour that is closed keeps its place.

```ts
// shared/session.ts
export interface Session {
  // …
  /** where you are in each tour you have opened, by tour id. The 50 opened last are kept */
  tourPlaces: Readonly<Record<TourId, TourPlace>>;          // NEW; TourPlace is 04 §8's, without `id`
  /** the TOURS section */
  tourList: { view: "all" | "onSelection" };                // NEW
}
// ContextState.tour is now the tour's id only: TourId | null
```

**The file.** One more field beside the five of [05](05-states.md) §7: `created_at`, for "today 18:40" and the order. Everything else in a
row is already there: the first stop's name and kind, `status`, `writing`, `query`, `depth`, the stops' nodes (for "on this node") and their
`code_hash` (for `changed`, computed for the rows on screen only).

**The core.** `[check]` who keeps the cache small. Proposal: `vnoc walk start` removes all but the 50 newest tours.

**The mockup.** [06](06-mockup-brief.md) §4.1 has the five tours of the pictures above.

Back: [`README.md`](README.md)
