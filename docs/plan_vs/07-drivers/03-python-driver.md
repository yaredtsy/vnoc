# 07.03: The Python driver (adapter around pylspt)

> **Status:** 📝 outline: written in full after **S2, S3** · **Program:** driver · **Milestones:** M1, M5, M7

**Purpose:** The thin adapter in `drivers/python` that turns the driver protocol into pylspt calls, without pylspt knowing about V-NOC (VS-D65).

**Builds on:** [02-07 §1 (pylspt conditions)](../02-technology/07-repo-and-drivers.md) · [02-05 §2 (docstring IDs)](../02-technology/05-identity-and-archive.md)

## Will define

1. **Crate layout:** `main.rs` (stdio loop), `adapter.rs` (protocol ⇄ pylspt types), nothing else.
2. **Dependency:** pylspt by `rev`; `[patch]` in a gitignored `.cargo/config.toml` for dev.
3. **The salsa actor:** one long-lived database per workspace, warm across requests (plan/04-02).
4. **IDs (stable):** `edit.rs` `plan_ids` fed with core-derived IDs; recorded spans; the new `# FileID:` writer.
5. **Environment inputs:** the `pyproject.toml` sections and lockfiles from S2 (C22).
6. **What pylspt needs to change** (a list to agree with pylspt's own plan).

## MVP vs stable

| MVP (0.x) | Stable (1.0) |
|---|---|
| outline, call trees, parse errors, env inputs | IDs in docstrings and file headers |

## Questions to settle while writing

- Does pylspt need a span-returning ID reader, or can the adapter compute it?
