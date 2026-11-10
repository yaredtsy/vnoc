# 03.02 — Repositories: reads shaped by use case

The classic **repository pattern** says "one repository per aggregate, hiding the DB".
Python had that in name (`structure_repo`, `code_element_repo`, `call_repo`, …), but each
repo also committed its own writes, built WOQL inline, and inherited a 450-line `BaseRepo`.

v2 keeps the good part (hide the DB behind typed methods) and changes two things:

1. **Writes are not in repositories.** All writes go through `WritePlan` → `apply` (03). Repositories only read.
2. **Read methods are named after the question they answer**, and return only the fields that question needs.

---

## Read models (rows)

These are small structs defined in `vnoc-store`. They are *not* `Node`: they're
projections, the minimum each use case needs.

```rust
/// For the structure diff (05-orchestrator/04). One row per Folder/File, Active or Tombstoned.
pub struct StructureRow {
    pub id: NodeId,
    pub kind: NodeKind,               // Folder | File
    pub path: RelPath,
    pub owner: NodeId,                // logical container — the ONLY parent the diff looks at
    pub parent_is_group: bool,        // needed for the move rule (01-data-model/03)
    pub content_hash: Option<Hash128>,// files only
    pub outline_hash: Option<Hash128>,
    pub status: StatusLite,           // Active | Tombstoned(since_seq)
}

/// For the code-element diff (05-orchestrator/05). Rows for classes/functions of the given files.
pub struct CodeRow {
    pub id: NodeId, pub kind: NodeKind, pub name: String,
    pub owner: NodeId, pub file: FileId, pub parent_is_group: bool,
    pub span: Span, pub body_hash: Hash128, pub status: StatusLite,
}

/// For the call-tree diff (05-orchestrator/07). One level of the tree.
pub struct CallRow {
    pub id: NodeId, pub owner: NodeId, pub target: NodeId,
    pub subtree_hash: Hash128, pub call_count: u32, pub has_user_data: bool,
}
```

> Why `has_user_data`? Rule 6 in 01-data-model/04: call nodes without user data are
> hard-deleted, the rest are tombstoned. The adapter computes it (in the parent group, has
> documents, has a theme) so sync doesn't need three more queries.

## The read methods by use case

| Use case | Method | Returns | Notes for adapters |
|---|---|---|---|
| **Sync: structure** | `snapshot_structure()` | all `StructureRow` | the one "read everything" call; must be lean (7 fields). 10k files ≈ 1–2 MB. |
| **Sync: code** | `snapshot_code(files)` | `CodeRow` for those files | uses the denormalised `file` field: one lookup per file, batched in one query |
| | `code_rows_by_ids(ids)` | `CodeRow` for those IDs (any file, Active or Tombstoned) | finds defs that moved in from unchanged files, or were tombstoned in an earlier sync (05-orchestrator/05) |
| **Sync: calls** | `entries_reaching(funcs)` | entry IDs whose `reaches` ∋ any func | reverse lookup on `EntrySummary.reaches` |
| | `call_level(owners)` | `CallRow` whose `owner` ∈ owners | the tree diff loads **one level at a time**, only where hashes differ |
| | `contexts(keys)` | `ContextRow { key, edges_hash }` | layer-1 diff |
| **UI: tree** | `children(parent, ChildQuery { kinds, include_tombstoned, page })` | `Page<Node>` | `parent == X`; paged (Python's `project-children-pagination` plan) |
| | `ancestors(id)` | path to Root | breadcrumbs |
| | `get_nodes(ids)` | full nodes | |
| **UI: docs** | `documents_for(target)` | `Document`s | |
| **GC** | `tombstones(TombstoneQuery { older_than, max_seq })` | `TombstoneRow` | |
| **Bookkeeping** | `sync_state()` | `SyncState` | |

When a new feature needs a new question (e.g. "functions with most callers"), you add a
method here and implement it in both adapters. That's deliberate friction: it keeps
every query visible, named and testable.

---

## Should this be one trait or several?

The list above sits on one trait (`ProjectGraph`) for now. If it grows past ~20 methods,
split it by use case into `SyncReads`, `TreeReads`, `DocReads`, and have `ProjectGraph:
SyncReads + TreeReads + DocReads + Write`.

> **🦀 Rust note: supertraits.**
> `trait ProjectGraph: SyncReads + TreeReads {}` means "anything that is a
> `ProjectGraph` is also a `SyncReads` and a `TreeReads`". Functions can then ask for
> *only what they use*: `fn diff(store: &dyn SyncReads)`. That documents the function's
> needs and makes test doubles smaller. Start with one trait; split when it hurts.

## Exit criteria

- Every read used by sync returns a row type, not a `Node`. (Check: `snapshot_*` never deserialises `user` fields.)
- `children()` is paged. Tested with 10k children under one folder.

Next: [`03-write-plan-and-unit-of-work.md`](03-write-plan-and-unit-of-work.md)
