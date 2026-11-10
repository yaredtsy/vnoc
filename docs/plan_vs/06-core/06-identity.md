# 06.06: Identity: deriving IDs, and IDs in code

> **Status:** 📝 outline: written in full after **S3, S4** · **Program:** Rust core (+ driver) · **Milestones:** M4 (derivation), M7 (in code)

**Purpose:** How permanent IDs are created, and (for stable) how they get into code and back out.

**Builds on:** [02-05 §1–3, §7](../02-technology/05-identity-and-archive.md) · [plan/04-04 id strategy](../../plan/04-drivers-and-ids/04-id-strategy.md)

## Will define

1. **Deriving an ID:** the blake3 formula, `occurrence`, the counter when taken (checked against code, both sides, archived), ULID-style encoding (VS-D60).
2. **When an ID is created:** on the first attachment (MVP), or on injection under `all` (stable). Saved at once, never only in SQLite.
3. **(Stable) modes and rules:** `on-attach` / `all` / `none`, `[[path]]` resolution (longest path), `id_in_code`, the scope rule.
4. **(Stable) Add / Remove:** the driver's edit and recorded span, exact-text removal, the mismatch message.
5. **(Stable) lost IDs:** the grouped Restore / Keep removed notice.
6. **(Stable) save-time IDs:** precomputed per document version (VS-D66), the reasons that inject, the parse guard.
7. **Duplicates** (copy-paste, cherry-pick).

## MVP vs stable

| MVP (0.x) | Stable (1.0) |
|---|---|
| derivation + folder IDs only (`none` mode) | everything about IDs in code |

## Questions to settle while writing

- Is "already taken" checked against the cache (fast) or a scan (exact)?
