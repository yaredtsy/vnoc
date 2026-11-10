# 02.03 — Errors, logging, configuration

Python's backend handled errors with `print(e); return None`. That hides
failures, so a sync could "succeed" with half the files missing. This page sets the
rules for v2.

---

## Errors

### Rule: library crates return typed errors; the binary uses `anyhow`

```rust
// crates/vnoc-store/src/error.rs
#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("node {0} not found")]
    NotFound(NodeId),

    #[error("write conflict: {0}")]
    Conflict(String),                       // someone else committed first

    #[error("backend unavailable")]
    Unavailable(#[source] Box<dyn std::error::Error + Send + Sync>),   // retryable

    #[error("backend rejected the request: {message}")]
    Rejected { message: String, detail: serde_json::Value },           // a bug on our side

    #[error(transparent)]
    Domain(#[from] DomainError),
}

impl StoreError {
    pub fn is_retryable(&self) -> bool { matches!(self, Self::Unavailable(_) | Self::Conflict(_)) }
}
```

> **🦀 Rust note: what `thiserror` does.**
> It's a derive macro that writes the `Display` and `std::error::Error` impls for you.
> - `#[error("…")]` is the `Display` text; `{0}` / `{field}` refer to the variant's data.
> - `#[source]` marks the underlying cause, so `error.source()` walks the chain and
>   logs show "backend unavailable: connection refused".
> - `#[from]` also generates `impl From<DomainError> for StoreError`, which makes the
>   `?` operator convert automatically: inside a function returning `Result<_, StoreError>`,
>   `some_domain_call()?` just works.

> **🦀 Rust note: why not `anyhow` everywhere?**
> `anyhow::Error` is "any error, plus context". That's great in `main` and in tests,
> but a caller **can't `match` on it**. The sync needs to know *which* error happened:
> `Unavailable` → retry the sync later; `Rejected` → bug, stop and report; `Conflict`
> → re-snapshot and retry. Typed enums make those decisions explicit and checked by
> the compiler. So library crates use typed enums and the binary uses `anyhow`.

### Each crate has its own error enum

`DomainError`, `StoreError`, `DriverError`, `SyncError`, `AppError`. Each layer wraps the
one below with `#[from]`, adding meaning:

```rust
pub enum SyncError {
    #[error("store: {0}")]      Store(#[from] StoreError),
    #[error("driver {lang}: {source}")]
    Driver { lang: LanguageId, #[source] source: DriverError },
    #[error("sync cancelled")] Cancelled,
    #[error("scan failed at {path}: {source}")]
    Scan { path: RelPath, #[source] source: std::io::Error },
}
```

### Rule: per-file failures are data, not errors

If one file fails to parse, the sync **continues**, and the failure is recorded:

```rust
pub struct SyncReport {
    pub seq: u64,
    pub files_failed: Vec<(RelPath, String)>,   // shown in the UI
    // …
}
```

A failed file is treated as **"unchanged"** for that sync. Its old graph stays as it is, and
nothing under it is tombstoned. That last part is critical: a parse error must never look like
"all functions in this file were deleted". Python had exactly this risk (an exception in
`_build_existing_map` returned `{}`, so everything looked new).

### Rule: no `unwrap()` / `expect()` in library code

…except for real invariants, with a message saying why it can't fail:
`expect("RelPath is validated at construction")`. Clippy's `unwrap_used` lint warns on the rest.

### API mapping

`vnoc-api` maps `AppError` to gRPC status codes in **one** function:
`NotFound → NOT_FOUND`, `Domain(FamilyMismatch|Cycle) → INVALID_ARGUMENT`,
`Conflict → ABORTED`, `Unavailable → UNAVAILABLE`, anything else → `INTERNAL`
(logged with full detail; the client gets a short message and a correlation ID).

---

## Logging and tracing

`tracing` gives structured events **and spans**. A span is a timed scope with fields, and
every event inside it inherits those fields:

```rust
#[tracing::instrument(skip(self, plan), fields(project = %self.project_id, seq))]
async fn run_sync(&self, plan: SyncScope) -> Result<SyncReport, SyncError> {
    tracing::Span::current().record("seq", self.next_seq());
    tracing::info!(files = scan.files.len(), "scan done");
    …
}
```

Every log line from inside a sync then carries `project=… seq=…`. That's the "sync
correlation ID" the Python refactor plan asked for, and it's free.

- Dev: pretty human output. Prod: JSON lines (`tracing-subscriber` `json` feature).
- Level via `RUST_LOG=vnoc_sync=debug,vnoc_store_terminus=info`.
- Timings: the span close time *is* the phase timing. This replaces Python's
  `performance.py` tracker. Add `tracing-chrome` or `tracing-flame` later for profiling.

---

## Configuration

```toml
# vnoc.toml
[server]
listen = "127.0.0.1:8000"
cors_origins = ["http://localhost:5173"]

[store]
kind = "terminus"                  # the only value today; "memory" for dev
url = "http://127.0.0.1:6363"
user = "admin"
password_env = "VNOC_TERMINUS_PASSWORD"   # secrets come from env, never the file
team = "admin"

[sync]
debounce_ms = 500
max_wait_ms = 5000
max_parallel_files = 64
tombstone_gc_days = 7
tombstone_gc_syncs = 20

[drivers.typescript]
command = ["bun", "run", "src/lsp/ts_js/server.ts", "--port", "0"]
```

```rust
#[derive(serde::Deserialize)]
pub struct Config { pub server: ServerConfig, pub store: StoreConfig, pub sync: SyncConfig, … }

let cfg: Config = figment::Figment::new()
    .merge(figment::providers::Toml::file("vnoc.toml"))
    .merge(figment::providers::Env::prefixed("VNOC_").split("__"))   // VNOC_SYNC__DEBOUNCE_MS=300
    .extract()?;
```

**Per-project settings** (ID strategy, ignore patterns, enabled languages, GC thresholds) live in the
project registry (P6), not in this file, because they travel with the project.

---

## Exit criteria

- Each crate has one `error.rs` with a `thiserror` enum. No `Box<dyn Error>` in public signatures except as `#[source]`.
- A test where one fixture file has a syntax error: the sync succeeds, the report lists the file, and nothing under it is tombstoned.
- Running with `RUST_LOG=debug` shows `project` and `seq` on every sync log line.

Next: [`../03-storage/00-overview.md`](../03-storage/00-overview.md)
