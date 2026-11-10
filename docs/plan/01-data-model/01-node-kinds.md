# 01.01 — Node kinds

For every kind: what it is, its fields, and **who owns each field**.

- **S** = sync-owned. Computed from disk/analysis, and sync overwrites it freely.
- **U** = user-owned. Set through the API, and sync **never** writes it.
- **B** = both. Sync sets it and the user can change it, with rules (see 03).

That split is the most important thing on this page. In Python every write
replaced the whole document, so user fields only survived if someone remembered to
copy them over (`CODE_SET_FIELDS_TO_PRESERVE` in `child_raw.py`). In v2,
**sync writes are field patches**: sync only ever sends S fields, so it *can't* clobber
U fields, even by accident.

---

## Common fields (every containment node)

| Field | Type | Owner | Meaning |
|---|---|---|---|
| `id` | `NodeId` | S | Stable identity. See [`02-identity.md`](02-identity.md). |
| `name` | string | S (U for Group) | Display name. File/function names come from disk. |
| `parent` | `NodeId` | B | Direct container. May be a `Group`. |
| `owner` | `NodeId` | S | Logical container. Never a `Group`. See 03. |
| `status` | `Status` | S | `Active` or `Tombstoned{…}`. See 04. |
| `id_pending` | bool | S | The ID was minted by us and isn't written into the source yet. See [`04-drivers-and-ids/04-id-strategy.md`](../04-drivers-and-ids/04-id-strategy.md). Only Folder/File/Class/Function. |
| `created_at` / `updated_at` | timestamp | S | `updated_at` changes only when an S field really changes. |
| `theme` | `Option<Theme>` | U | Colours (navbar, sidebar, …), as in Python's `ThemeConfig`. |
| `description` | string | U | Python required one; v2 makes it optional, default empty. |

---

## `Root`

Exactly one per project graph. Fixed ID `Root/root`.

| Field | Owner | Notes |
|---|---|---|
| `id` = `Root/root` | S | Fixed, so every query can name it without a lookup. |
| `theme`, `description` | U | Replaces v1's "global document theme folder" (`FolderSchema/init`). |

`Root` has no `parent`/`owner`/`status`. It is the one exception to "every node has a parent".

> Why a separate kind and not "a Folder with `is_root = true`"? Because then every
> function that takes a folder has to ask "is this the root?". A separate kind
> lets the type system say it once: `enum Container { Root, Folder(..), File(..), … }`.

---

## `Folder`

| Field | Type | Owner | Notes |
|---|---|---|---|
| `path` | `RelPath` | S | e.g. `src/core`. Derived data, can change (rename). |

Children: `Folder`, `File`, `Group[Structure]`.

## `File`

| Field | Type | Owner | Notes |
|---|---|---|---|
| `path` | `RelPath` | S | `src/core/engine.py` |
| `language` | `LanguageId` | S | `python`, `typescript`, … picks the driver. |
| `content_hash` | `Hash128` | S | xxh3-128 of the bytes on disk. "Did the file change?" |
| `outline_hash` | `Hash128` | S | Hash of the list of (id, kind, name, nesting) of its classes/functions. "Did its *structure* change?" A file edit that only changes function bodies keeps this hash. |
| `size_bytes` | u64 | S | For UI and limits. |

Children: `Class`, `Function`, `Group[Code]`.
Also linked from `SourceText.file` (optional, P13).

## `Class`

| Field | Type | Owner | Notes |
|---|---|---|---|
| `qname` | string | S | `pkg.mod.Engine`. Display/debug only, never identity. |
| `file` | `NodeId` | S | The containing **file**, denormalised. "All elements of file X" becomes one lookup instead of a path walk. Updated when the element moves to another file. |
| `span` | `Span` | S | start/end line+column (Python's `CodePosition`). |
| `bases` | `Vec<NodeId or ExternalRef>` | S | Python's `base_classes` (MRO). |
| `body_hash` | `Hash128` | S | Hash of the class's source text. |

Children: `Class`, `Function`, `Group[Code]`.

## `Function`

Same as `Class` (minus `bases`), plus:

| Field | Type | Owner | Notes |
|---|---|---|---|
| `body_hash` | `Hash128` | S | Hash of the function's source text (docstring ID line excluded, so injecting an ID doesn't count as an edit). |
| `is_method` / `kind` | enum | S | `Function`, `Method`, `StaticMethod`, `ClassMethod`, `Property`, `Closure`. pylspt's entry rules need it (`pylspt/plan/03-call-tree/16-entries.md`). |

Children: `Class`, `Function` (nested defs), `Group[Code]`, and the **call tree root
nodes** (`Call`, `Group[Call]`).

## `Group`

| Field | Type | Owner | Notes |
|---|---|---|---|
| `family` | `GroupFamily` | U (fixed at creation) | `Structure` / `Code` / `Call`. |
| `name`, `description`, `theme` | | U | |
| `parent` | `NodeId` | U | Groups nest. |
| `owner` | `NodeId` | S/derived | The first non-group ancestor. **All children of a group share this owner.** |

What a group may hold:

| family | may contain | may live under |
|---|---|---|
| `Structure` | Folder, File, Group[Structure] | Root, Folder, Group[Structure] |
| `Code` | Class, Function, Group[Code] | File, Class, Function, Group[Code] |
| `Call` | Call, Group[Call] | Function, Class, Call, Group[Call] |

The service layer enforces this when the user moves things (06-api/03).

## `Call` (layer 2: the materialised tree)

| Field | Type | Owner | Notes |
|---|---|---|---|
| `id` | `Call/<hash>` | S | `hash(owner_id, target_id)`. See 02. |
| `target` | `NodeId` | S | The Function or Class being called. |
| `entry` | `NodeId` | S | The function whose tree this node belongs to (denormalised). "Delete/refresh A's tree" is one lookup. |
| `call_count` | u32 | S | Merged siblings with the same target (same as Python). |
| `contexts` | `Vec<ContextKey>` | S | Which layer-1 contexts this node shows. |
| `subtree_hash` | `Hash128` | S | Merkle hash of `(target, children's subtree_hash sorted)`. Equal hash means equal subtree, so don't descend. |
| `depth` | u16 | S | Distance from the entry. For limits/UI. |
| `marks` | flags | S | pylspt marks: cut, recursion, … (`pylspt/src/interp/tree.rs::Mark`). |

`parent` = the entry Function/Class, another `Call`, or a `Group[Call]`.

## Side documents (not containment nodes)

### `Document` (user notes)
`target: NodeId`, `title`, `body`, timestamps. Owner: U, all fields.
Sync never reads or writes documents. When a node is tombstoned its documents
stay. When it's GC'd, documents are **kept and re-targeted to `Root`** with an
"orphaned from <qname>" note (proposed). Losing user writing is the one thing we never do.

### `SourceText` (optional, P13)
`file: NodeId`, `text`, `content_hash`. Stored separately so tree queries never load code.

### `CallContext` (layer 1)
`key: ContextKey` (= function stable def + 128-bit digest, from pylspt's `StableContextId`),
`func: NodeId`, `edges: Vec<CallEdge>`, `edges_hash: Hash128`, `status` flags (cut/gaps).
Deduplicated across entries: the same context is stored once.

### `EntrySummary` (layer 1)
`entry: NodeId` (a Function/Class analysed as an entry), `root_context: ContextKey`,
`reaches: Set<NodeId>` (every function reachable from this entry), `tree_hash: Hash128`
(= `subtree_hash` of the entry's top level), `analysed_at_sync: u64`.
`reaches` is the **reverse index** that answers "B changed: whose trees can be affected?"
→ [`05-orchestrator/07-call-graph-sync.md`](../05-orchestrator/07-call-graph-sync.md)

### `SyncState`
One per project graph: `seq: u64` (incremented each successful sync), `last_full_scan_at`,
stats. Tombstones record the `seq` they were made in.

---

## Exit criteria

- Every field has an owner (S/U/B), and no S-write path can touch a U field.
- No node kind has a field listing its children.

Next: [`02-identity.md`](02-identity.md)
