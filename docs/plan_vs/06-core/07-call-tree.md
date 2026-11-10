# 06.07: Lazy call trees

> **Status:** 📝 outline: written in full after **S2** · **Program:** Rust core (+ driver) · **Milestones:** M5

**Purpose:** How call trees are built level by level, cached, and kept correct without re-solving everything.

**Builds on:** [02-05 §8 scope](../02-technology/05-identity-and-archive.md) · [plan/05-07 call graph sync](../../plan/05-orchestrator/07-call-graph-sync.md)

## Will define

1. **Layers:** layer 1 (call sites per function, from the index), layer 2 (resolved trees, on demand).
2. **`callTree` / `expand`:** depth, budget, the frontier flag, cancellation.
3. **Scope:** pulled-in files indexed on first reach (dimmed), third-party → external leaves, the through-a-library limit.
4. **Caching:** expansions keyed by (function, context), validated by dependency hashes, several versions kept.
5. **Markers:** recursion, cut, unresolved, external.
6. **Call-node IDs:** `hash(owner, target)` (VS-P14), stable across machines.

## MVP vs stable

| MVP (0.x) | Stable (1.0) |
|---|---|
| all of it | tuning on 10k-file repos |

## Questions to settle while writing

- Does C1's answer force work inside pylspt before M5?
