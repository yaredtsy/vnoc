# 06.03 — Application services (`vnoc-app`)

The layer between the API and everything else: **use cases**. The API layer decodes requests
and calls these. Sync and storage do the work.

Why a separate layer? Because the same use case may be called from more than one place
(gRPC today, a CLI command or the agent's tool calls tomorrow), and business rules
(family checks, cycle checks, P17) must live in one place, not inside a gRPC handler.

---

## Shape

```rust
#[derive(Clone)]
pub struct App {                         // cheap to clone: everything inside is Arc
    pub projects: ProjectService,
    pub graph:    GraphQueryService,
    pub sync:     SyncService,
    pub groups:   GroupService,
    pub docs:     DocumentService,
    pub nodes:    NodeService,
}
```

Each service holds what it needs: `Arc<dyn GraphStore>`, the open-projects map, config.

## The `ProjectWriter`: every write goes through one queue per project

```rust
pub struct ProjectWriter { tx: mpsc::Sender<WriteJob> }

struct WriteJob { plan: WritePlan, meta: CommitMeta, reply: oneshot::Sender<Result<ApplyOutcome, StoreError>> }

impl ProjectWriter {
    pub async fn apply(&self, plan: WritePlan, meta: CommitMeta) -> Result<ApplyOutcome, StoreError> {
        let (reply, rx) = oneshot::channel();
        self.tx.send(WriteJob { plan, meta, reply }).await.map_err(|_| StoreError::closed())?;
        rx.await.map_err(|_| StoreError::closed())?
    }
}
// the task: `while let Some(job) = rx.recv().await { let r = graph.apply(job.plan, job.meta).await; let _ = job.reply.send(r); }`
```

Both the sync (step 10) and every user action use it. That's what makes the "nothing else writes to
`main` between steps" guarantee in 03-storage/04 true.

> **🦀 Rust note: the request/response actor pattern.**
> `mpsc` carries the job in, and a `oneshot` inside the job carries the answer back. The caller
> `await`s the oneshot, so to the caller it looks like a normal async function call, but every
> write is executed one at a time, in order, by one task. You'll see this pattern three times
> in this project (scheduler, pylspt actor, writer). It's the idiomatic Rust replacement for
> "a shared object with a lock".

## Example use case: move a node into a group

```rust
impl GroupService {
    pub async fn move_into(&self, project: &ProjectId, node: NodeId, group: NodeId)
        -> Result<(), AppError>
    {
        let p = self.open.get(project)?;                                     // open project handle
        let nodes = p.graph.get_nodes(&[node.clone(), group.clone()]).await?;
        let (n, g) = (find(&nodes, &node)?, find(&nodes, &group)?);

        let NodeData::Group { family } = &g.data else { return Err(AppError::NotAGroup(group)) };
        rules::check_family(*family, n.kind())?;                             // I5
        rules::check_no_cycle(&p.graph, &n.id, &g.id).await?;                // I6 (ancestors of g)
        rules::check_group_owner(*family, n, g)?;                            // 05-orchestrator/08 table

        let plan = WritePlan {
            patches: vec![Patch { id: node.clone(),
                                  set: NodePatch { parent: Some(group.clone()), ..Default::default() },
                                  expect: Some(Expect::Parent(n.parent.clone())) }],
            ..Default::default()
        };
        p.writer.apply(plan, CommitMeta::user(format!("move {node} into {group}"))).await?;
        p.events.send_graph_changed([n.parent.clone(), group]);              // UI refreshes both containers
        Ok(())
    }
}
```

> **🦀 Rust note: `let … else`.**
> `let NodeData::Group { family } = &g.data else { return Err(…) };` means "destructure this
> pattern, or else run the block (which must leave the function)". It reads like a guard clause
> and avoids a nested `match`. Stable since Rust 1.65.

Note that `rules::*` are **pure functions in `vnoc-domain`** (they only need nodes), so they're unit-tested
there and shared with sync (the P17 rule uses the same helpers).

## Services list

| Service | Responsibilities |
|---|---|
| `ProjectService` | create/open/close/delete (05-orchestrator/01), settings, registry |
| `GraphQueryService` | read-only: children, ancestors, nodes, call trees, callers. Filters out tombstoned nodes unless asked. |
| `SyncService` | `request(project, scope)` → scheduler; `subscribe` → events; reports |
| `GroupService` | group operations (05-orchestrator/08 table) |
| `DocumentService` | documents CRUD; orphaned documents (re-targeted to Root by GC) |
| `NodeService` | user fields (theme, description); trash: list tombstoned, empty trash (= GC now) |
| `GcService` (internal) | tombstone collection, context sweep, optimize trigger |

## Exit criteria

- Each use case has unit tests against the memory store, with no gRPC involved.
- No `vnoc-app` function takes or returns a `pb::` type.

Next: [`../07-roadmap/01-milestones.md`](../07-roadmap/01-milestones.md)
