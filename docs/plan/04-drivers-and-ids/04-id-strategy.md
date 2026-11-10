# 04.04 — `IdStrategy`: in-source IDs today, open for others

Decision D5: keep the Python approach (IDs written into the source) but **behind
a trait**, so a project can switch strategy later without changes to the sync pipeline.

---

## The Python approach, as it was

| Thing | Where the ID lives | Python code |
|---|---|---|
| function / class | `ID: <uuid>` line in its docstring | `vnoc_lsp_python/id_injector.py` |
| file | `FileID: <uuid>` in the module docstring | `file_folder_ids.py::read_or_inject_file_id` |
| folder | `FolderID: <uuid>` in that folder's `__init__.py` (created if missing) | `file_folder_ids.py::read_or_inject_folder_id` |

What we keep: the **formats** (so a project's existing IDs keep working, even though the DB
is new) and the idea that IDs travel with the code.

What we change (R7):

1. **Reading never writes.** "Read or inject" becomes two separate operations.
2. **Writes happen once, after the DB apply**, only for things that need an ID.
3. **Our own writes don't trigger a new sync** (watcher expectation, below).

## The trait

```rust
// crates/vnoc-drivers/src/ids.rs

#[async_trait::async_trait]
pub trait IdStrategy: Send + Sync {
    fn kind(&self) -> IdStrategyKind;              // InSource | (later) Manifest | Fingerprint

    /// Read the IDs of scanned folders/files. Pure read.
    async fn read_structure_ids(&self, items: &[ScanItem]) -> Result<Vec<IdRead>, IdError>;

    /// The ID of a class/function, given what the driver outlined. Pure.
    /// (In-source: `def.source_id`. Other strategies could compute one.)
    fn code_id(&self, def: &DefOutline, file: &FileId) -> Option<IdKey>;

    /// After the DB apply: make the IDs we minted durable (in-source: write them into files).
    async fn persist(&self, req: PersistRequest) -> Result<PersistReport, IdError>;
}

pub enum IdRead {
    Found(IdKey),             // e.g. FileID present
    Missing,                  // carrier exists, no ID yet → needs one
    Unsupported,              // this item can't carry an ID (e.g. a .md file) → use fallback matching
}
```

## Where it sits in the pipeline

```
SCAN ─► read_structure_ids ─► DIFF (structure) ─► PARSE ─► code_id ─► DIFF (code) ─► RECONCILE
                                                                                         │
     every item without an ID gets a freshly MINTED id (uuid v4, in memory), with        │
     `id_pending = true` on the node                                                     ▼
                                                                                  PLAN ─► APPLY
                                                                                         │
                       persist(minted ids) ◄─────────────────────────────────────────────┘
                       (in-source: write FileID/FolderID/ID lines, watcher told to expect them)
                       success → next sync's plan clears `id_pending`
```

### Why mint first, write later (and the `id_pending` flag)

The DB needs an ID to create the node, and the file needs the same ID written into it. Two
orders are possible:

| Order | If step 2 fails… |
|---|---|
| write file, then DB (Python) | file has an ID the DB never saw. Harmless, but it means writing user files *before* we know the sync worked, and those writes trigger the watcher mid-sync. |
| **DB, then write file (v2)** | DB has a node whose ID isn't in the file yet. |

To handle the v2 failure case: the node is created with `id_pending = true`. On the next sync, a
def **without** an ID that sits in the same `owner` with the same kind and name **matches the pending node**,
reuses its ID and retries `persist`. So nothing is duplicated and no user data is lost. Once
persisted, the flag is cleared.

### Watcher expectation (no self-triggered syncs)

Before writing, `persist` registers `(path, expected_content_hash_after_write)` with the
project's event filter (05-orchestrator/09). When the watcher sees an event for that path and
the file's hash equals the expected hash, the event is dropped. If the user edited the file in
the meantime, the hash differs and the event goes through as normal. This is safer than
Python's "pause the watcher" approach, which lost events (L8).

### Safety guards when writing (from pylspt's `inject_ids`)

- Write only first-party project files (`InjectError::OutsideProject`).
- Never write a file that doesn't parse (`InjectError::DoesNotParse`). The user is probably mid-keystroke.
- Re-check right before writing that the file's content hash still equals the one we parsed. If not, skip it; the next sync retries.
- Only `.py` for now (pylspt's guard against notebooks).

## Things that can't carry an ID (`IdRead::Unsupported`)

Non-code files (`README.md`, `pyproject.toml`, images), and folders where creating an
`__init__.py` would be wrong (a TS project, `node_modules`-like dirs, data folders).

🟡 **Proposed — confirm (P18):**
- **Folders:** the `__init__.py` carrier is used **only in Python package folders** (folders that already
  contain `.py` files), as before. Other folders use **fallback matching**.
- **Files:** fallback matching for files without a carrier.
- **Fallback matching** = identity by path; a "missing" item and a "new" item with the **same
  content hash** (files) or the **same set of child IDs** (folders) are treated as a move. The ID for
  these is minted and stored only in the DB.

Python created an `__init__.py` in every folder of every project, including non-Python
ones. P18 stops that while keeping the Python behaviour where it makes sense.

## Future strategies (O1), and what they'd implement

| Strategy | `read_structure_ids` | `code_id` | `persist` |
|---|---|---|---|
| **InSource** (today) | parse docstrings / `__init__.py` | `def.source_id` | write ID lines |
| **Manifest** | read `.vnoc/ids.json` (path → id) | from a manifest keyed by (file, qname, nth) | rewrite the manifest |
| **Fingerprint / no-ID** | always `Unsupported` | none: match by (owner, kind, name), then by `body_hash` for moves | nothing |

The pipeline doesn't change for any of them: it only sees "found / missing / unsupported" and
"persist these". The per-project setting `id_strategy` picks one at project open.

> **🦀 Rust note: trait objects for strategies.**
> This is the textbook "strategy pattern". In Rust it's `Arc<dyn IdStrategy>` stored in
> the project's context, chosen at runtime from settings:
>
> ```rust
> let ids: Arc<dyn IdStrategy> = match settings.id_strategy {
>     IdStrategyKind::InSource => Arc::new(InSourceIds::new(python_driver.clone())),
>     // IdStrategyKind::Manifest => Arc::new(ManifestIds::open(&root)?),
> };
> ```
>
> Commented-out arms like that are fine in a plan, but in code, use `#[non_exhaustive]` on the
> enum, or keep only the variants that exist, so the compiler forces a decision when one is added.

## Exit criteria

- A sync over a fixture with no IDs: the DB gets nodes with `id_pending`, files get IDs written, the watcher fires **no** extra sync, and the next sync clears `id_pending` with zero other changes.
- A fixture where persist fails for one file (made read-only): no duplicates appear on the next sync.
- Copy-pasted function (duplicate ID): handled per 01-data-model/02.

Next: [`../05-orchestrator/00-overview.md`](../05-orchestrator/00-overview.md)
