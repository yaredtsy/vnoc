# 07.02: The driver host

> **Status:** 📝 outline: written in full after **S2** · **Program:** Rust core · **Milestones:** M1

**Purpose:** How the core finds, installs and runs drivers: one lookup path from day one (VS-D30).

**Builds on:** [02-07 §3–4](../02-technology/07-repo-and-drivers.md)

## Will define

1. **Lookup:** `<data dir>/vnoc/drivers/<lang>/<ver>/`, picking the newest version in the supported protocol range.
2. **Copy-on-first-run** of the bundled driver: temp → verify → rename, under a lock.
3. **Process management:** spawn, handshake, restart with back-off, shutdown.
4. **Cleanup:** versions unused for 30 days.
5. **(Later) downloads:** manifest, minisign, proxy, `vnoc driver install <file>` (stable for the offline command).

## MVP vs stable

| MVP (0.x) | Stable (1.0) |
|---|---|
| lookup + copy-on-first-run + process management | `vnoc driver install <file>` |

## Questions to settle while writing

- What does the OS do to a copied binary (C11)?
