# Future 03. Function history: who changed this function, when, and what

**Status: idea only. Not scheduled.** It shares its git layer and its matching with [`../01-graph-diff/`](../01-graph-diff/README.md); build the two
together or this one first as the smaller start.

```
function history
├── README.md                  this file: the idea, how it relates to graph diff, the phases
├── 01-running-git-safely.md   CLI or library · is spawning git from an extension acceptable · Restricted Mode · the hardened runner
└── 02-blame-history-diff.md   last change by blame · the full history walk · the function-only diff · filtering commits · cost
```

## 1. The idea

Git answers history questions per file and per line. V-NOC can answer them per function:

```
parse_order   (src/orders.py · OrderService)
──────────────────────────────────────────────
last changed   3 weeks ago by Sara · a1b2c3d "fix rounding in parse_order"

history        a1b2c3d  Sara    2026-10-21   body       "fix rounding in parse_order"
               9f0e1d2  Yared   2026-09-02   moved      from src/helpers.py, renamed from parse
               77c3b10  Yared   2026-08-30   signature  "accept raw bytes"
               0d4e5f6  Yared   2026-08-12   added

               click a row → the diff of THIS FUNCTION ONLY in that commit
```

Plus a commit list that can be filtered by author, date, message, path, and **by function** ("only commits that changed `parse_order`").

## 2. How it relates to graph diff

```
graph diff        two versions, every node            →  "what changed between A and B?"
function history  one node, every commit that touched it  →  "how did THIS node get here?"
```

History is graph diff restricted to one node, repeated along the commits. Same `git.rs`, same outline-by-text, same matching ladder, same
`header` / `body` fingerprints ([`../01-graph-diff/02-how-to-compute.md`](../01-graph-diff/02-how-to-compute.md) §4–5). Build one engine.

## 3. What it adds to frozen formats

**Nothing.** History is computed from git and the code (index data, place C). An outline cache keyed by blob hash may live under `.vnoc/local/cache/`
later; it is deletable.

## 4. Phases

| Phase | What the user gets | Needs |
|---|---|---|
| **1** | `git.rs` with the hardened runner and the trust gate; "last changed by" from blame in the hover and the doc panel | nothing new from the driver |
| **2** | The history list for one node, and the function-only diff for each row | outline by text (exists); match by ID and location |
| **3** | The commit list with filters (author, date, message, path, node) | phase 2 for the node filter |
| **4** | History that follows a function into another file; unmarked moves by body | the 0.4 matching ladder; the outline cache on disk |

Phase 1 is small and useful on its own. Phase 2 is the real feature.

Next: [`01-running-git-safely.md`](01-running-git-safely.md)
