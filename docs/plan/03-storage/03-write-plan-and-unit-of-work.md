# 03.03 — `WritePlan`: one atomic change

The **Unit of Work** pattern: collect every change of one logical operation, then
commit them together or not at all. In v2 that object is the `WritePlan`, plain data
with no DB handle inside.

---

## The type

```rust
// crates/vnoc-store/src/plan.rs

#[derive(Debug, Default, Clone, PartialEq)]
pub struct WritePlan {
    pub creates:  Vec<Node>,                       // new containment nodes (full)
    pub patches:  Vec<Patch>,                      // field changes to existing nodes
    pub deletes:  Vec<NodeId>,                     // HARD deletes (groups, user-data-free calls, GC)
    pub side:     Vec<SideOp>,                     // non-containment docs
    pub state:    Option<StateCas>,                // SyncState compare-and-set (idempotency)
}

#[derive(Debug, Clone, PartialEq)]
pub struct Patch {
    pub id: NodeId,
    pub set: NodePatch,                            // from vnoc-domain (S-owned fields only)
    pub expect: Option<Expect>,                    // precondition, see below
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expect {
    Parent(NodeId),                                // "only if parent is still X"
    Status(StatusLite),
}

#[derive(Debug, Clone, PartialEq)]
pub enum SideOp {
    UpsertContext(CallContext),
    UpsertEntrySummary(EntrySummary),
    UpsertSourceText { file: FileId, text: String, hash: Hash128 },
    DeleteSourceText(FileId),
    CreateDocument(Document),
    PatchDocument { id: DocId, patch: DocumentPatch },
    DeleteDocument(DocId),
    RetargetDocuments { from: NodeId, to: NodeId, orphaned_from: OrphanInfo },  // GC
}

pub struct StateCas { pub expect_seq: u64, pub new_seq: u64, pub stats: SyncStats }

pub struct CommitMeta {
    pub author: String,                            // "vnoc-sync", or the user
    pub message: String,                           // see 05-terminus-best-practices
    pub kind: CommitKind,                          // Sync | User | Gc | Bootstrap
}

pub enum ApplyOutcome {
    Applied { commit: Option<CommitRef>, counts: PlanCounts },
    AlreadyApplied,                                // StateCas showed seq already moved (retry case)
    PreconditionFailed(Vec<NodeId>),               // some Expect didn't hold → caller re-plans
}
```

## Why plain data is so useful

- **Testable without a DB.** The whole sync pipeline is a function
  `(scan, snapshot, parse results) → WritePlan`. Tests compare plans with `insta`
  snapshots: "for this fixture change, the plan is: 2 creates, 1 patch (owner), 1 tombstone".
- **Dry runs for free.** `vnoc sync --dry-run` prints the plan and doesn't apply it.
- **One place to make atomic.** Each adapter implements `apply` once, and does it well.
- **Logging and metrics.** `plan.counts()` gives "+12 ~40 →3 ✝5" for the commit message.

> **🦀 Rust note: building plans with `&mut`.**
> Pipeline steps take `&mut WritePlan` and push into it:
>
> ```rust
> fn plan_structure(diff: &StructureDiff, plan: &mut WritePlan) { plan.creates.extend(…); }
> fn plan_code(diff: &CodeDiff, plan: &mut WritePlan) { … }
> ```
>
> The borrow checker guarantees only one step mutates the plan at a time. When steps run
> in parallel (per-file parsing), each produces **its own** partial result (`Vec<…>`) and the
> orchestrator merges them afterwards. Shared `Mutex<WritePlan>` is never needed. Python's
> body parser used `asyncio.Lock` around shared buffers; in Rust, "each task returns its
> part, then merge" is simpler and faster.

## Preconditions (`Expect`): user edits vs a running sync

A sync takes seconds. Meanwhile the user may drag a file into a group. The sync's plan was
built from a snapshot where the file's parent was the folder, so it must not overwrite the
user's change.

Two layers of protection:

1. **One writer per project.** All `apply` calls for a project go through a single
   `ProjectWriter` actor (a task with an `mpsc` queue). User edits and sync applies never run
   at the same moment, so within our process there's no race at all.
2. **Preconditions for B-owned fields.** When sync patches `parent` (a disk move), it says
   `expect: Some(Expect::Parent(old))`. If the user changed `parent` after the snapshot, the
   precondition fails, and the adapter returns `PreconditionFailed([ids])`. The sync then
   re-snapshots *just those nodes* and re-plans them, without restarting everything.

S-only fields (`owner`, hashes, spans) need no precondition: only sync writes them, and
syncs of one project are serialised (05-orchestrator/09).

## Idempotency (`StateCas`)

What if `apply` times out? Did the commit land or not? We can't know from a timeout.

Every sync plan carries `StateCas { expect_seq: N, new_seq: N+1 }`, and the adapter
applies it **in the same transaction**. On retry:

- `SyncState.seq == N` → the first attempt didn't land, so apply normally.
- `SyncState.seq == N+1` → it already landed, so return `AlreadyApplied`.

So retrying an apply is always safe. User edits (no `StateCas`) are made idempotent by
construction: "set parent to G" twice gives the same result.

## Size

A first import of a big project can produce 100k+ creates. The plan is still **one**
`WritePlan` (memory is fine: ~100 bytes × 100k = ~10 MB). *How* it reaches the DB in
chunks while staying atomic is the adapter's problem (05-terminus-best-practices,
"Big plans").

## Exit criteria

- `WritePlan` is `Clone + PartialEq + Debug`, so tests can compare it.
- Contract test: apply a plan twice with the same `StateCas` → second returns `AlreadyApplied`, graph unchanged.
- Contract test: a patch with a stale `Expect::Parent` → `PreconditionFailed`, nothing in the plan applied.

Next: [`04-terminus-adapter.md`](04-terminus-adapter.md)
