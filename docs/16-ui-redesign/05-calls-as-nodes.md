# 16.05 Calls as nodes: ids, fetching, the chevron, the marks, stale trees

```
calls as nodes
├── 1. what a call node is, and its id
├── 2. where the trees live and who asks for them
├── 3. the decision: does this function call anything? (R6)
├── 4. opening a call node, one level at a time
├── 5. the code changed (stale)
├── 6. unknown, recursive, cut, incomplete
├── 7. Show Calls (replaces Show Call Tree)
├── 8. messages
└── 9. files and tests
```

Replaces 14 §7's "Calls" section, its progress notification and its "stale never re-runs by itself". `vnoc/callTree` in the core is unchanged:
it returns the whole merged tree of one entry, and the UI shows it one level at a time.

## 1. What a call node is

A call node is one `CallTreeNode` (14 §6) **at one place in the drawing**: "`reserve_stock`, called by `checkout_order`, called by `checkout`".
The same function can be a call node in many places and is also a structure node at home. So a call node needs an id of its own, and a
pointer to the structure node it shows (`key`, the callee; `null` when the index does not know it).

**The id is the path of keys from the function that owns the tree:**

```ts
// shared/calls.ts
export type CallId = string;   // "call:" + JSON of [owner, step, step, …]

/** `call:["app/api.py::checkout","app/order.py::checkout_order","app/inventory.py::reserve_stock"]` */
export function callId(owner: string, steps: readonly string[]): CallId;
export function parseCallId(id: string): { owner: string; steps: string[] } | null;
export const isCallId = (id: string) => id.startsWith("call:");

/** a step: the callee's key; for an unknown callee `?` + its path + `::` + qualname; a second sibling with the same step gets `#2` */
export function stepOf(node: CallTreeNode, siblings: readonly CallTreeNode[]): string;
```

Why keys and not child indexes (today's `0.2.1`): a new call written above an old one moves every index, and with them what is open and what is
selected. Keys survive that. JSON, because a key may contain any character a path can.

Only **functions** own calls (R7). `__init__` owns a class's calls. A call node of kind class (`Store()`) has the children the driver gives it
(what `__init__` calls).

**A class with no `__init__` of its own** (none at all, or an inherited one, 14 §10.1) has no row to put its creation's calls under. Then, and only
then, the **class** owns them: its card's children are its methods, then its creation's calls (call cards, dashed edges), and `wantedCalls`
wants the class. A class that has its own `__init__` is never wanted: its calls are under that `__init__`, and fetching the class would only
fetch the same calls twice (decided 2026-11-13, refines R7).

## 2. Where the trees live, and who asks

The trees stay in the extension (`CallTrees`, `extension/src/callTree.ts`): large, not saved, the latest per owner. What changes is **who asks**.

**The host asks for what is open.** After every session change and every new structure, it computes:

```
wanted = { functions that are open, or the last focus step, in any context }
       ∪ { the callees of open call nodes marked `cut` }
       ∪ { the selected function of the active context }        ← prefetch, so the first click is instant
```

and fetches each wanted key whose tree is neither known nor on its way: **at most two requests at once**, the prefetch last. A key that stops
being wanted while its request runs is **cancelled** (`sendRequest`'s token, as 7.3 built it).

Why the host and not the page: the open set is in the session (R8), a reload restores open call nodes without the page doing anything, and the rule lives in one tested function (`wantedCalls(session, tree, trees)` in `shared/calls.ts`).

Each fetch is announced to the canvas (§8): `loading`, then the tree, or `failed` with the message. The canvas bar shows its progress line.
**No notification**: a click on a chevron is not worth a toast. (The sidebar has no call rows, C1, so it draws nothing for calls.)

`KEEP = 50` stays, counted over trees nothing wants: a wanted tree is never dropped.

## 3. The decision: does this function call anything? (R6)

A function's chevron must show before its calls are fetched, or the user cannot ask for them. Three ways:

| | How | Cost | Feel |
|---|---|---|---|
| **A. optimistic** (recommended) | every function whose tree is not known has a chevron. Opening it fetches the tree; if it is empty, the chevron goes and the meta line says *calls nothing*. Below the first level everything is exact, because the whole tree came at once. | one request per function opened (20–500 ms, 14 §11); the prefetch of the selected function hides most of it | VS Code's Call Hierarchy and the Explorer on a slow drive behave this way: a chevron that disappears on opening is familiar |
| B. a 1-level check | a new core method `vnoc/hasCalls { keys }`, sent for the functions **drawn** (a file's functions when the file opens), answered by the driver from one open project | about 9 ms per function on httpx (14 §11 sweep: 1,134 functions in 9.7 s), plus opening the project per request (T6): a file of 30 functions ≈ 0.3–0.5 s, on every file opened | exact chevrons; a new method in the core and the driver |
| C. full prefetch | fetch the tree of every drawn function | the cost of B, but the trees are large | wasteful |

**B, decided 2026-11-13**, as soon as the driver keeps the project open (steps 8.2a and 8.2b). Until then A stays, with the same UI:
`calls: "unknown"` just stops happening for drawn functions. A fix from the analyzer's side made B cheap and exact; §3.1 is the contract.

### 3.1 The probe: exact chevrons from one small answer

| Rule | Why |
|---|---|
| **What is probed:** every function and class **card drawn** on the canvas whose calls are not known yet. The selected card first, then in reading order; cancelled when the card is no longer drawn. **Never ahead of a fetch the user asked for** (open, cut, prefetch): one probe in flight at a time, and only while no such fetch is waiting | the canvas asks only for what it shows. The driver solves on **one** worker (8.2a), so a probe queued before a click delays the click: on httpx a warm solve is 5 ms at the median but 60 ms at p90 (14 §11.1) |
| **The request:** the host sends `vnoc/hasCalls { key } → { revision, count }`, **one key per request**, so each probe is scheduled and cancelled on its own (a batch of 30 would hold the one worker for up to a second, uncancellable). For each key the core asks the driver for `vnoc/callTree` with **`depth: 1`**, and runs the answer through the **same merge and filter** as `vnoc/callTree` (code outside the project and unresolved calls dropped, 14 §4) | the chevron must mean exactly "opening shows at least one card", whatever the analyzer's own options count |
| **The answer:** `{ revision, calls: { [key]: n } }`, `n` = how many call cards opening would show (merged, filtered children) | `n = 0`: no chevron, the meta line says *calls nothing*. `n > 0`: a chevron, and `+n` in the closed card's footer (04 §1) |
| **The probe's tree never reaches the page.** Its children are not drawn, and its marks are not shown | the analyzer marks a node `Cut` where `depth` stopped and it has children (`analyzer-core/src/render.rs`, the `max_depth` check). That `Cut` only means "the answer was shortened": it is not the *calls not loaded* of §6 |
| **The children's `Cut` is not used for their chevrons** | a child is marked `Cut` when it calls anything, **including** calls the core then drops (a library, an unresolved call), so it could promise a chevron that opens nothing. The children's chevrons come from the full tree |
| **Opening fetches the full tree** (no `depth`), once per opened function, as in §4. The canvas still shows **one level per click**; the levels below come from that same tree, with no new request | nothing opens by itself; a `Cut` in the **full** tree (the budget ran out) is a real one and shows *calls not loaded* (§6) |
| **While a probe is pending** | the chevron's slot is empty and keeps its width (nothing shifts when it appears). A chevron that appears is better than one that disappears |
| **When it is dropped** | on a new index revision; the drawn cards are probed again |
| **Full trees have a depth cap too** | the core asks for every full tree with `depth: 32` (one constant). A node at the cap comes back `Cut` with its children; opening it fetches the callee's own tree (§4, §6), as for any cut. Without a cap a deep chain breaks the core: it reads a driver's answer with `serde_json`'s default nesting limit (128; a call-tree level costs 2), so a chain past ~60 functions fails to parse, and the answer's size has no bound |

What it costs (debug builds, on a kept project, from the analyzer's own measurements):

| | the probe (`depth: 1`) | opening (the full tree) |
|---|---|---|
| **Python** | the same work as the full tree: `depth` only shortens the answer, the whole tree is still walked. ~6 ms warm. It saves size, not time | one more full solve, ~6 ms: each request builds a new solver. So about **12 ms per card you open** |
| **TypeScript** | the depth limit really skips work (budget and frontier): cheaper | a new request for the opened node, until the analyzer's `more` tokens exist |

**No paging for Python.** Python's walk cannot stop early: values flow back from callees, so a body left unwalked would change answers above it.
If the analyzer ever pages its output, Python gets only that: a shorter answer, not less work.

## 4. Opening a call node, one level at a time

```ts
// shared/calls.ts
export type Calls =
  | { state: "unknown" }                      // the owner's tree is not here: draw a chevron, no +N
  | { state: "loading" }
  | { state: "failed"; message: string }
  | { state: "known"; children: CallTreeNode[] };

/** The calls under a structure function (its key) or a call node (its id). */
export function callsOf(trees: TreeStore, id: string): Calls;
```

| Under | Children |
|---|---|
| a structure function `F` | the root children of `trees[F]` |
| a call node | its node's `children` in its owner's tree. If the node is **`cut`** (the walk stopped there) and has no children: the root children of `trees[callee]`, fetched when the call node is opened (§2's second line). |

The flow of one click on `checkout_order`'s chevron (closed, tree not known):

| Step | Canvas card |
|---|---|
| click the chevron | `toggle` → its id joins `expanded` |
| the host sees a wanted key, fetches | spinner on the chevron; one skeleton card in the next column |
| the tree arrives | the call cards, dashed edges |
| it calls nothing | no chevron any more; the meta line says *calls nothing* |
| it failed | the meta line says *couldn't read calls · Retry* (Retry re-fetches) |
| the user closes it before the answer | the request is cancelled |

**Never in bulk.** Expand All Below opens folders, files and classes; it never opens calls. A merged tree can have a thousand nodes (httpx's
`request`: 995); opening them all is the "dump" you asked to avoid. A call node's own `+N` says how many are behind it.

## 5. The code changed (stale)

A tree is stale when the structure's `revision` is newer than the tree's (`isStale`, kept). Then:

1. A stale tree that is still wanted is fetched again, **800 ms after the last index change** (typing produces many), while the old one stays on
   screen. The progress lines show it.
2. A stale tree nothing wants is dropped.
2b. A wanted tree whose last fetch **failed** is fetched again, once, at the same moment: the code changed, so the failure may be gone (a syntax
   error fixed, a file saved). A second failure stays failed until the next change or **Retry** (§4). Without this, a failure shown before the
   Retry button exists (8.3 part 2) would stay for ever.
3. When the new tree arrives, every call id in any context that no longer resolves is removed from `expanded`, and from `selected` (which falls
   back to the owner function). This is `reduce`'s `callsChanged` step, like `treeChanged`. An EXPLORE context whose call is gone closes.

So the canvas follows the code without a Refresh button. Today's "the code changed · Refresh" goes away.

## 6. Unknown, recursive, cut, incomplete

| Case | Where it comes from | Card / row | Can |
|---|---|---|---|
| **unknown** (`key: null`) | the callee is in a file the index does not have (ignored, outside the folder) | 60% opacity, *not in the index* | select, its site links. No Explore, no docs, no Go to Code. |
| **recursive** | the callee is already above it on this path | no chevron, `↻ recursive` | everything but open. Explore goes to its home, as for any call. |
| **cut** | the driver's depth or node budget ran out | chevron, `… calls not loaded`, dim (not a warning colour). Tooltip: *"The analysis stopped here. Open to load what `<name>` calls."* | opening fetches the callee's own tree (§4) |
| **incomplete** | the analysis gave up on part of it | `… incomplete` with the tooltip | everything; the children shown may be partial |

## 7. Show Calls (replaces Show Call Tree)

`vnoc.showCalls` (title **Show Calls**, was `vnoc.showCallTree` "Show Call Tree"): on a function, **Open in Canvas** (beside the editor when run
from one), then `expand` the function. The host fetches, the cards appear one level deep. On a class: the same; its tree is what creating
it calls, through its own `__init__` **or an inherited one** (14 §10.1: `Child(Base)` with no `__init__` of its own still calls what
`Base.__init__` calls). The message is decided by the **answer**, never by looking for an `__init__` among the class's children in the
structure (that misses the inherited one):

| The class's call tree | Shows |
|---|---|
| has children | the calls, as for a function |
| comes back empty | *"Order calls nothing when it is created; open a method to see its calls."* (true with no `__init__` at all, and with one that calls nothing) |

Same `vnoc.callTree` context key as today.

## 8. Messages (`shared/messages.ts`)

| | Change |
|---|---|
| request `callTrees` | kept: a page that opens later gets every tree the host holds |
| request `showCallTree` | **removed** (stale trees are fetched again by themselves, §5) |
| request `openAt` | kept (a site link, Go to Call Site) |
| request `code` | also reads one line per site for the card's site list (04 §2.1) |
| event `callTree { key, result }` | kept |
| event **`callTreeState { key, state: "loading" \| "failed" \| "idle", message? }`** | **new**: the canvas draws spinners and failures from it |
| note `callsDrawn` | **replaced** by the counts in `rendered` (`{ visible, hidden, calls }`) |

## 9. Files and tests

| File | Change | Tests |
|---|---|---|
| `shared/calls.ts` (new) | ids, `stepOf`, `callsOf`, `wantedCalls`, `resolveCall` (for `reduce`'s `CallLookup`), and `siteText`, `listedSites`, `markBadge`, `isStale` moved from `webview/src/canvas/calls.ts` | `shared/calls.test.ts`: ids survive a call inserted above; the `#2` rule; `callsOf` through a `cut` node; `wantedCalls` with two contexts, a focus step, a prefetch; a stale tree that is not wanted is not asked for |
| `shared/session.ts` | `callsChanged`; `select` of a call opens / closes EXPLORE children (03 §4) | `shared/session.test.ts` |
| `extension/src/callTree.ts` | `CallTrees` asks for `wantedCalls`, two at a time, cancels what is no longer wanted, emits `callTreeState`, debounces stale refetches | the window suite's `calls.test.ts`: open a function on the fixture → `rendered.calls` grows; close it before the answer → no tree kept for it |
| `extension/src/commands.ts` | `showCalls` replaces `showCallTree`; `refreshCallTree` goes | |
| `webview/src/state/calls.ts` | holds trees and states from the two events | |

Next: [`06-sidebar.md`](06-sidebar.md)
