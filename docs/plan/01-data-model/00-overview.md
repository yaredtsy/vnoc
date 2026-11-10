# 01.00 — Data model overview

The data model comes first because everything else (storage, sync, API) is code that
moves *these* things around. If it's wrong, every later layer pays for it.

---

## The graph at a glance

```
Root ─────────────────────────────── (one per project graph, fixed id)
 ├── Folder "src"
 │    ├── Group[Structure] "core stuff"          ◄── user-made, not on disk
 │    │    └── File "engine.py"
 │    │         ├── Class "Engine"
 │    │         │    ├── Function "run"
 │    │         │    │    └── CallNode → Function "step"       (layer 2: tree)
 │    │         │    │         └── CallNode → Function "tick"
 │    │         │    └── Group[Code] "helpers"
 │    │         │         └── Function "_log"
 │    │         └── Function "main"
 │    └── File "util.py"
 └── File "setup.py"

Side documents (not in the containment tree, they point AT nodes):
  Document  ──target──► any node          user notes
  SourceText ──file───► File              the file's text (optional, P13)
  CallContext (layer 1)  func + digest + edges   the call graph facts
  EntrySummary (layer 1) entry function → what it reaches, tree hash
  SyncState               last sync seq, counters
```

## Three ideas carry the whole model

1. **One containment tree, linked child → parent** (`parent` field on each node).
   No child sets on parents. → [`03-edges-parent-and-owner.md`](03-edges-parent-and-owner.md)

2. **Two parents per node: `parent` (may be a group) and `owner` (never a group).**
   Sync compares `owner` with the disk and never needs to know groups exist.
   → [`03-edges-parent-and-owner.md`](03-edges-parent-and-owner.md)

3. **Every node has a lifecycle status** (`Active` / `Tombstoned`). Nothing disappears in one step.
   → [`04-lifecycle-and-tombstones.md`](04-lifecycle-and-tombstones.md)

## What changed compared to v1 (Python)

| v1 | v2 | Why |
|---|---|---|
| top-level nodes have no parent; `FolderSchema/init` is a side thing | `Root` node, every node has a parent | R2: fast top-level query, one code path |
| 5 child-set fields per parent type | `parent` + `owner` on the child | R5: a move is one write; R1: groups transparent |
| 3 group schemas | 1 `Group` with `family` | P8: one set of rules |
| `CallSchema/<uuid4>` | `Call/<hash(parent, target)>` | R4: re-analysis finds the same node again |
| fully expanded call trees, no fingerprints | layer 1 facts + layer 2 tree with `subtree_hash` | R4 |
| `documents` embedded as subdocuments in nodes | `Document` points at its target | editing a doc doesn't rewrite the node; sync never touches docs |
| `is_root: "true"` strings | real booleans / enums | it's 2026 |
| hard delete during sync | tombstone, GC later | R3 |
| absolute & relative paths mixed | relative, `/`-separated | P7 |

## Files in this folder

1. [`01-node-kinds.md`](01-node-kinds.md): every node type and its fields.
2. [`02-identity.md`](02-identity.md): how IDs look, who makes them, why they're stable.
3. [`03-edges-parent-and-owner.md`](03-edges-parent-and-owner.md): containment, groups, invariants.
4. [`04-lifecycle-and-tombstones.md`](04-lifecycle-and-tombstones.md): status, resurrection, GC.
5. [`05-rust-domain-types.md`](05-rust-domain-types.md): the same model as Rust code, explained.

## Exit criteria for this folder

- You can draw the tree above from memory.
- For each node kind you know which fields **sync** owns (it overwrites them) and which fields the **user** owns (sync never touches them). This split is what keeps user data safe.
