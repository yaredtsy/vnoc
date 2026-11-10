# 09. Which `plan_vs/` file to read (and which are stale)

`plan_vs/` has 43 files. You don't need most of them to catch up. This page says **what to read for each question**, and **which files are out of date**.

## ⚠ First: the plan is behind the decisions

After spike S4 you re-cut the MVP. The new decisions are written up but **not yet in the plan**:

| Draft row | What it decides | Where it is now |
|---|---|---|
| **VS-D78** the MVP cut | the MVP **writes an `ID:` line into the docstring when you attach a doc** (through `WorkspaceEdit`, undoable). Only ID mode, no no-ID mode, no private docs, no Share/Make private, no two-ID merge. Ladder step 1 + VS Code rename events only. Trash commands "Empty trash" / "Restore removed doc". Post-MVP becomes a **P1–P4 list** | `spikes/s4-git/DRAFT-doc-rows.md` §11–12 |
| **VS-D79** the node ID format | `NodeId` (not a ULID): BLAKE3 `derive_key`, Crockford base32, 26 chars; old UUID IDs dropped | §13 (already implemented in `crates/vnoc-domain/src/id.rs`) |
| **VS-D80** driver protocol, 5 decisions | UTF-16 positions everywhere, `planIds` keyed by `(qualname, occurrence)`, core picks IDs / driver picks places… | §15 (already reflected in `07-drivers/01-driver-protocol.md`) |
| Final MVP file formats | `vnoc.toml` (`format = 1`, scope only), `node.toml`, doc frontmatter, `.gitignore`, `trashed.toml` | §14 (`node.toml` is implemented) |
| S4 findings §9–10 | gix lookups, C9, step 0 checks, closing summary | §9–10 (not pasted into `05-spikes/04`) |

**The code already follows these drafts** (`id.rs` says "in code: `ID:` in the docstring"; `node_toml.rs` says "the MVP always writes `id_in_code = yes`"), and 06-01 and 07-01
already cite VS-D78/D79/D80, but the decision log stops at **VS-D77**. Until you paste them, the log, the roadmap and the code disagree.

## Files that are stale because of the drafts

| File | What's stale | Fixed by |
|---|---|---|
| `04-roadmap/01-mvp-vs-stable.md` | says the MVP **never edits code** and uses **ID mode `none` only**; puts in-code IDs, Share/Make private, two-ID merge in the wrong release; "ULID-style" | DRAFT §11 (the edits are written out) |
| `04-roadmap/02-milestones.md` | M3/M4 contents, M7 "in-code IDs", "step 0 ships with the ladder" | DRAFT §11 |
| `00-requirements/03-functional-requirements.md` | the M/S column for private docs, solo, ladder, Remove ID, etc. | DRAFT §11 "Follow-ups" |
| `00-requirements/02-user-journey.md`, `05-user-controls-and-config.md`, `README.md` tree | `none`/solo defaults, team/solo questions in setup (now post-MVP) | DRAFT §11 "Follow-ups" |
| `02-technology/05-identity-and-archive.md`, `04-storage-and-git.md` | old ID formula, "ULID", `PermId`, UUID support | DRAFT §13 sweep |
| `06-core/06-identity.md` | `PermId`, "ULID-style" | DRAFT §13 sweep |
| `05-spikes/01-hello-core.md` | last line "Still to check by hand…" lists things the findings above already checked | small edit |
| `05-spikes/04-git-and-matching.md` | header says gix/C9 "not done yet"; they are done (DRAFT §9) | DRAFT §9–10 |

(The decision log, the checklist and `05-spikes/` are yours to edit; this report didn't touch them.)

---

## What to read, by question

| I want to know… | Read | Status |
|---|---|---|
| **what the product is and does** | `plan_vs/README.md` (the user tree), `00-requirements/01-vision-and-scope.md` | ✅ (tree shows the full v1, not the MVP) |
| **what's in the MVP** | `spikes/s4-git/DRAFT-doc-rows.md` §11–12 (**not** `04-roadmap/01` until it's updated) | ⚠ draft |
| **why something was decided** | `01-decisions/01-decision-log.md` (search for the `VS-D` number) | ✅ up to D77 |
| **how the four programs fit** | `02-technology/01-architecture-at-a-glance.md` | ✅ |
| how VS Code starts and talks to the Rust core | `02-technology/02-running-rust-sidecar.md`, then `06-core/02-editor-protocol.md` | ✅ |
| the canvas, node card, focus stack, doc panel | `02-technology/03-frontend-webview.md` | ✅ |
| `.vnoc/` layout, `node.toml`, git, the cache | `02-technology/04-storage-and-git.md` + DRAFT §14 | ⚠ needs D79 sweep |
| IDs, matching, Missing, archive | `02-technology/05-identity-and-archive.md` + `crates/vnoc-domain/src/id.rs` | ⚠ needs D79 sweep |
| the monorepo and drivers | `02-technology/07-repo-and-drivers.md` | ✅ |
| **the order of work** | `04-roadmap/02-milestones.md` + DRAFT §11 | ⚠ |
| what each spike found | `05-spikes/01…04` (Findings sections) | ✅ (S5 empty) |
| the crate list and rules | `06-core/01-crates-and-domain.md` | ✅ written |
| the editor ↔ core messages | `06-core/02-editor-protocol.md` | ✅ written |
| the core ↔ driver wire | `07-drivers/01-driver-protocol.md` | ✅ written |
| how to build and run it today | `10-release/01-ci-and-packaging.md` ("Run it locally") | ✅ written |

## Design docs (phase 2b): which are written

The rule: a design doc is written **after** the spike that answers its questions. 📝 = outline only (says what it *will* define).

| Program | File | Status | Can be written now? |
|---|---|---|---|
| core | `06-core/01-crates-and-domain.md` | ✅ | — |
| core | `06-core/02-editor-protocol.md` | ✅ (only what exists) | grows with each milestone |
| core | `06-core/03-cache-schema.md` | 📝 | ✅ S2 done → **needed for M1** |
| core | `06-core/04-userdata-writer.md` | 📝 | ✅ S3 done → M3 (apply D78 cut: no private side) |
| core | `06-core/05-sync-and-matching.md` | 📝 | ✅ S4 done → M4 (MVP = step 1 + rename events + VS-D74) |
| core | `06-core/06-identity.md` | 📝 | ✅ → M3/M4 |
| core | `06-core/07-call-tree.md` | 📝 | ✅ S2 done → M5 |
| drivers | `07-drivers/01-driver-protocol.md` | ✅ | — |
| drivers | `07-drivers/02-driver-host.md` | 📝 | ✅ → **needed for M1** |
| drivers | `07-drivers/03-python-driver.md` | 📝 | ✅ S2, S3 done |
| extension | `08-extension/01-package-and-activation.md` | 📝 | ✅ S1 done → **M0/M1** |
| extension | `08-extension/02-commands-and-views.md` | 📝 | ✅ → M1 |
| extension | `08-extension/03-documents-and-saving.md` | 📝 | ⏳ waits for **S5** (for M6); the M3 part can go first |
| webview | `09-webview/01-app-and-state.md` | 📝 | ✅ → M2 |
| webview | `09-webview/02-canvas-and-node-card.md` | 📝 | ✅ → M2 |
| webview | `09-webview/03-editors-and-doc-panel.md` | 📝 | ⏳ waits for **S5** |
| release | `10-release/01-ci-and-packaging.md` | ✅ local build; CI part 📝 | ✅ → M0 |
| release | `10-release/02-testing.md` | 📝 | ⏳ "after all spikes" |

**Suggested reading for a fast catch-up (about 45 min):** this report → `plan_vs/README.md` → `02-technology/01-architecture-at-a-glance.md` → `spikes/s4-git/DRAFT-doc-rows.md` §11–14 →
`06-core/01` → `06-core/02` → `07-drivers/01`.

Next: [`10-whats-left.md`](10-whats-left.md)
