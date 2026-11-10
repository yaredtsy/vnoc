# 06.03: The SQLite cache

> **Status:** 📝 outline: written in full after **S2** · **Program:** Rust core · **Milestones:** M1

**Purpose:** The exact cache schema, the `GraphStore` trait, and how entries are validated and evicted.

**Builds on:** [02-04 §8 the cache](../02-technology/04-storage-and-git.md) · [plan/03 storage](../../plan/03-storage/00-overview.md)

## Will define

1. **Tables and indexes** (from 02-04 §8): `files`, `nodes`, `docs`, `doc_links`, `refs`, `import_aliases`, `expansions`, `edges`, `expansion_deps`, `callers`, `key_moves`, `meta`, the parsed-`.vnoc` table.
2. **`GraphStore` trait:** the domain methods only (no raw query), and the in-memory implementation used by tests.
3. **The writer thread:** jobs over a channel, one transaction per sync, readers on their own WAL connections.
4. **Validation by dependency hashes** when an expansion is read; several versions per key; eviction after `cache_days` (30).
5. **Clearing on environment change:** which tables (resolution only, VS-D53), triggered by the driver's environment inputs.
6. **Schema version:** `PRAGMA user_version`; mismatch or corruption → delete and rebuild, never migrate.

## MVP vs stable

| MVP (0.x) | Stable (1.0) |
|---|---|
| everything except ID-specific columns used only in stable | `id_source = code` rows start appearing (M7) |

## Questions to settle while writing

- How big does the cache get on a 10k-file repo (measure in S2)?
