# 01.05 — The model as Rust types (`vnoc-domain`)

This is the code that goes into the `vnoc-domain` crate. It has **no** DB, async or
gRPC dependencies, only `serde`, a hash crate and a time crate. Everything else
in the workspace depends on it.

---

## 1. One struct for the common part, one enum for the rest

```rust
// crates/vnoc-domain/src/node.rs

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Node {
    pub id: NodeId,
    pub name: String,
    pub parent: NodeId,       // may be a Group
    pub owner: NodeId,        // never a Group (I2)
    pub status: Status,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
    pub user: UserFields,     // U-owned: theme, description. Sync never builds this.
    pub data: NodeData,       // kind-specific, S-owned
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind")]
pub enum NodeData {
    Folder   { path: RelPath },
    File     { path: RelPath, language: LanguageId, content_hash: Hash128,
               outline_hash: Hash128, size_bytes: u64 },
    Class    { qname: String, file: FileId, span: Span, bases: Vec<BaseRef>,
               body_hash: Hash128 },
    Function { qname: String, file: FileId, span: Span, fn_kind: FnKind,
               body_hash: Hash128 },
    Group    { family: GroupFamily },
    Call     { target: NodeId, entry: NodeId, call_count: u32,
               contexts: Vec<ContextKey>, subtree_hash: Hash128, depth: u16,
               marks: Marks },
}
```

> **🦀 Rust note: enum or trait objects?**
> Coming from Python you might reach for a base class `BaseNode` with subclasses,
> which in Rust would be `trait Node` + `Box<dyn Node>`. For a **closed** set of
> kinds that we control, an `enum` is better:
>
> - **Exhaustive `match`:** when you add `NodeData::Module` next year, the compiler
>   lists every `match` that forgot it. With trait objects you'd find out at runtime.
> - **No heap allocation per node, no vtable.** A `Vec<Node>` is one contiguous block.
> - **serde just works.** `#[serde(tag = "kind")]` serialises as `{"kind": "File", "path": …}`.
>   Trait objects need extra crates (`typetag`) to serialise.
>
> Use traits for **open** sets (things other crates implement: `GraphStore`,
> `LanguageDriver`, `IdStrategy`). Use enums for **closed** sets (data). That one rule
> covers most "enum or trait?" questions.

> **🦀 Rust note: why `user: UserFields` is a separate struct.**
> It makes the S/U split (01-node-kinds) **visible in the types**. The sync code
> builds `NodePatch` values (below), which simply have no field for theme or
> description, so sync *can't* write them, even by mistake. The compiler is enforcing a
> business rule here.

## 2. Patches, not whole documents

Sync never sends a whole `Node`. It sends what changed:

```rust
/// A change sync wants to make to an existing node. Only S-owned fields exist here.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct NodePatch {
    pub name: Option<String>,
    pub parent: Option<NodeId>,
    pub owner: Option<NodeId>,
    pub status: Option<Status>,
    pub data: Option<DataPatch>,     // e.g. new content_hash + outline_hash for a File
}
```

`Option<T>` means "unchanged" when `None` and "set to this" when `Some`. The Terminus
adapter turns a patch into WOQL that touches only those triples (03-storage/04), which
fixes the read-modify-write problem (L5).

> **🦀 Rust note: `#[derive(Default)]`.**
> With `Default`, you write only the fields you set:
> `NodePatch { owner: Some(new_owner), ..Default::default() }`. The `..` is
> "struct update syntax": take every other field from `Default::default()` (all `None`).

## 3. Small value types (newtypes)

```rust
pub struct Hash128(pub [u8; 16]);          // Copy, Eq, Ord, Hash; Display as 32 hex chars
pub struct RelPath(String);                // always '/'-separated, no leading '/', no '..'
pub struct Span { pub start: LineCol, pub end: LineCol }
pub struct LineCol { pub line: u32, pub col: u32 }   // 1-based line, 0-based col (Python's convention)
pub struct ContextKey { pub func: NodeId, pub digest: Hash128 }
pub enum  GroupFamily { Structure, Code, Call }
pub enum  FnKind { Function, Method, StaticMethod, ClassMethod, Property, Closure }
pub enum  BaseRef { Node(NodeId), External(String) }
pub struct LanguageId(CompactString);      // "python", "typescript"
```

`RelPath::new(s)` normalises and rejects bad input. After that, the rest of the code trusts it.

> **🦀 Rust note: when to derive `Copy`.**
> `Hash128` and `LineCol` are small, plain data, so derive `Copy` and pass them by value.
> `NodeId` holds a string, so it's `Clone` but not `Copy`. Pass `&NodeId` when you
> only read it, and clone only when you need to store it. If clones show up in profiles,
> switch `key` to `Arc<str>` (cloning is then a counter bump). That change doesn't affect
> callers, which is another benefit of the newtype.

## 4. Errors in the domain crate

```rust
#[derive(Debug, thiserror::Error)]
pub enum DomainError {
    #[error("invalid node id {0:?}")]
    InvalidId(String),
    #[error("{child} cannot be placed in a {family:?} group")]
    FamilyMismatch { child: NodeId, family: GroupFamily },
    #[error("moving {node} under {target} would create a cycle")]
    Cycle { node: NodeId, target: NodeId },
    #[error("invalid relative path {0:?}")]
    InvalidPath(String),
}
```

More on `thiserror` in [`../02-workspace-and-crates/03-errors-logging-config.md`](../02-workspace-and-crates/03-errors-logging-config.md).

## 5. The hashing helpers

```rust
pub fn call_id(owner: &NodeId, target: &NodeId) -> NodeId { … }   // 01-data-model/02
pub fn subtree_hash(target: &NodeId, children: &mut [Hash128]) -> Hash128 { /* sort children, then hash */ }
pub fn outline_hash(items: &[(NodeId, NodeKind, &str, u16 /*depth*/)]) -> Hash128 { … }
```

All of them use one `Canon` byte writer (length-prefixed fields) and xxh3-128, never `derive(Hash)`.

---

## Exit criteria

- `vnoc-domain` compiles with **no** async, DB or network dependency (`cargo tree -p vnoc-domain` is short).
- serde round-trip tests for every `NodeData` variant.
- Golden tests for `call_id`, `subtree_hash` and `outline_hash`.

Next: [`../02-workspace-and-crates/01-cargo-workspace.md`](../02-workspace-and-crates/01-cargo-workspace.md)
