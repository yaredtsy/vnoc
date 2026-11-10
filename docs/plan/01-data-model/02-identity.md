# 01.02 — Identity

> **The single most important property of this system:** the same thing on disk gets
> the same ID across syncs, renames and moves. Docs, groups, themes, logs and tests
> all hang off IDs. If an ID changes, the user's work is detached from the code it
> describes.

---

## Shape of an ID

```
<Kind>/<key>

Root/root
Folder/3f0c…-uuid
File/9a1b…-uuid
Class/5dd2…-uuid
Function/449d…-uuid
Group/7c21…-uuid
Call/ab12cd…(32 hex chars = 128-bit hash)
Document/…uuid
Context/<func-key>/<digest-hex>
```

The kind is part of the ID. Two reasons:

1. TerminusDB IDs are already `Type/key` (Python's `FunctionSchema/<uuid>`), so it maps 1:1.
2. Any log line or error message says what kind of thing it's about.

v1 prefixed with the schema class name (`FunctionSchema/…`). v2 uses short kind names.
The pylspt adapter translates (pylspt emits `FunctionSchema/<uuid>` in
`target_id` today: `pylspt/src/interp/wire.rs`). Only the adapter knows the old spelling.

---

## Who makes each ID

| Kind | Source | Stable because… |
|---|---|---|
| `Root` | fixed constant | there's only one |
| `Folder`, `File`, `Class`, `Function` | **`IdStrategy`** (today: UUIDs written into source, D5) | the ID travels *inside* the file, so moving the code moves the ID |
| `Group`, `Document` | random UUID v4, minted by the service that creates it | they're created once by the user and never re-derived |
| `Call` | **deterministic hash**: `xxh3_128("call" ‖ owner_id ‖ target_id)` | re-analysis of unchanged code produces the same (owner, target) pairs, so the same IDs |
| `Context` | pylspt's `StableContextId` (function stable def + 128-bit digest of its key) | pylspt designed it for exactly this (`pylspt/plan/03-call-tree/14-output.md`, "Stable identity") |

### Why `Call` uses `owner`, not `parent`

A user can drag a call node into a `Group[Call]`. That changes its `parent`, but it's
the same call. If the ID used `parent`, grouping a call would change its ID and the
next sync would see a "new" call and tombstone the grouped one. `owner` (the logical
parent: the enclosing call node or the entry function, groups skipped) doesn't change
when grouping. See [`03-edges-parent-and-owner.md`](03-edges-parent-and-owner.md).

### Why the hash doesn't include the call-site line

P10 trade-off. Including the line/column would separate two calls to `f` in the same
function, but then **every edit above the call moves its line and changes its ID**.
Python already merged siblings by target (`call_count`), and pylspt reproduces that
merge (`merge_for_output` in `wire.rs`). We keep the merge, so `(owner, target)` is unique among siblings.

> ⚠ **Hash input must be canonical bytes we write ourselves**, never Rust's
> `#[derive(Hash)]`. std's `Hash` is allowed to change between Rust versions and
> platforms. pylspt already follows this rule (`canon.rs`: "Bytes we wrote, never
> `#[derive(Hash)]`"). Write the input as length-prefixed UTF-8 strings.

---

## Duplicate IDs (copy-paste)

In-source IDs have one classic failure mode: **the user copies code**. Copy a
function and both copies carry `ID: 449d…`. Copy a folder and the `__init__.py`
with its `FolderID` comes along.

Policy (deterministic, so two syncs make the same choice):

1. Collect every occurrence of each ID during the scan/parse of the sync.
2. If an ID occurs more than once:
   - The occurrence whose **owner and path match the snapshot** (where the DB thinks it is) **keeps** the ID.
   - If none match (both copies moved), the one with the **smallest path, then smallest span start** keeps it.
   - Every other occurrence is treated as **new** and gets a fresh ID from `IdStrategy` in step 11.
3. Log it at `info` level: "duplicate ID 449d… in z.py:12, a.py:40; z.py:12 re-assigned".

This logic lives in **one place**, the identify step (05-orchestrator/04 and 05), so it can be unit-tested with a table of cases.

## Kind changes

If a docstring `ID: X` that belonged to a `Function` now sits on a `class`, the IDs
`Function/X` and `Class/X` are different. The function is tombstoned and the class is
created. That's rare and it's the honest answer: it *is* a different kind of thing.

---

> **🦀 Rust note: the newtype pattern.**
> We never pass IDs around as `String`. We wrap them:
>
> ```rust
> #[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
> pub struct NodeId {
>     kind: NodeKind,      // enum: Root, Folder, File, …
>     key: CompactString,  // the part after the slash
> }
> ```
>
> Why bother?
>
> - **The compiler catches mix-ups.** A `fn move_node(node: NodeId, to: NodeId)` can't
>   accidentally receive a file path, a qname or a Terminus IRI, all of which are also strings.
> - **Validation happens once**, when the ID is built (`NodeId::parse("Function/abc")?`).
>   After that, every `NodeId` in the program is known to be valid.
>   This idea is often called *"parse, don't validate"*.
> - **Kind checks are cheap.** `id.kind() == NodeKind::Group` is an enum compare, not a `starts_with`.
>
> Two impls make it pleasant to use:
>
> ```rust
> impl std::fmt::Display for NodeId {          // "Function/abc" for logs and the DB
>     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
>         write!(f, "{}/{}", self.kind.as_str(), self.key)
>     }
> }
> impl std::str::FromStr for NodeId {          // "Function/abc".parse::<NodeId>()
>     type Err = IdError;
>     fn from_str(s: &str) -> Result<Self, IdError> { /* split at '/', check kind */ }
> }
> ```
>
> Then a tiny step further, **typed IDs** where it helps:
> `pub struct FileId(NodeId);`. A function that takes `FileId` can only be called
> with something already checked to be a file. Use these at API boundaries
> (`fn outline(file: FileId)`), and plain `NodeId` inside generic graph code.
> Same pattern for `Hash128([u8; 16])` and `RelPath(String)`.

---

## Exit criteria

- `NodeId` round-trips: `id.to_string().parse::<NodeId>() == Ok(id)` (property test).
- `call_id(owner, target)` is stable across runs and platforms (golden-value test with a hard-coded expected hex).
- The duplicate-ID table test passes: same ID in 2/3 places, with/without a snapshot match.

Next: [`03-edges-parent-and-owner.md`](03-edges-parent-and-owner.md)
