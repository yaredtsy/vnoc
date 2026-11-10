# 05.02 — Step 1: scan

**Input:** project root + scope. **Output:** `ScanResult`. Reads the disk, never the DB.

```rust
pub struct ScanItem {
    pub path: RelPath,
    pub kind: ScanKind,              // Folder | File
    pub content_hash: Option<Hash128>,   // files only
    pub size: u64,
    pub mtime: SystemTime,
}
pub struct ScanResult { pub items: Vec<ScanItem>, pub skipped: Vec<(RelPath, String)> }
```

## Walking

Use the `ignore` crate's **parallel walker** (`WalkBuilder::build_parallel`):

- honours `.gitignore`, `.ignore`, `.git/info/exclude`, global gitignore
- plus our own always-ignored list: `.git/`, `.vnoc/`, `__pycache__/`, `node_modules/`, `.venv/`, `target/`
- plus `ProjectSettings.extra_ignores`
- doesn't follow symlinks (🟡 proposed: avoids cycles and files outside the project)

Which files are included? **All non-ignored files** become `File` nodes (the tree shows
everything, like an IDE). Only files a driver `handles()` get parsed. Empty folders are included.

## Hashing: skip work with an mtime cache

Hashing every file on every sync is wasteful. Keep an in-memory cache in the `ProjectHandle`:

```
path → (size, mtime, content_hash)
```

If size and mtime match the cache, reuse the hash without reading the file. The cache is
rebuilt on project open (first full sync hashes everything, in parallel). A scoped sync
hashes only its paths.

> mtime alone isn't trustworthy (some editors/tools preserve it). That's why the full safety-net
> sync (05-orchestrator/09) **ignores the cache** every Nth run (🟡 every 10th) and re-hashes.

> **🦀 Rust note: parallel walk + collect without locks.**
> `build_parallel().run(|| { … })` calls your closure on many threads. The classic way to collect
> results is to give each thread a channel `Sender` clone and collect from the `Receiver` at the end:
>
> ```rust
> let (tx, rx) = std::sync::mpsc::channel();   // std's channel is enough here
> walker.run(|| {
>     let tx = tx.clone();                       // each thread gets its own Sender
>     Box::new(move |entry| {
>         if let Ok(e) = entry { let _ = tx.send(to_item(e)); }
>         ignore::WalkState::Continue
>     })
> });
> drop(tx);                                      // close the channel so the iterator ends
> let items: Vec<ScanItem> = rx.into_iter().flatten().collect();
> ```
>
> Run the whole scan inside `tokio::task::spawn_blocking`, because it's blocking file I/O and
> CPU hashing, which must not run on the async workers (00-foundations/03).

## Errors

Unreadable file/folder → into `skipped` with the reason, and it's **treated as unchanged** (not
missing). Permission errors must never cause tombstones.

## Scoped scan

For `Paths(ps)`: stat each path. If it exists, scan it (recursively if it's a folder). If it
doesn't exist, it's "gone" (it may be missing *or* moved). Also include each path's parent folder
chain, so the owner checks in step 4 have what they need.

## Exit criteria

- 10k-file fixture: cold full scan < 1 s on a laptop SSD, warm (cache hit) < 200 ms.
- `.gitignore`d files never appear. A permission-denied folder appears in `skipped` and causes no tombstones.

Next: [`03-snapshot.md`](03-snapshot.md)
