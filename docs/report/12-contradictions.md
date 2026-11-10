# 12. Contradictions: where the plan, the drafts and the code disagree

Checked on 2026-11-08. **Three sources** can say different things:

| Source | Where | How current |
|---|---|---|
| **The plan** | `plan_vs/` (decision log stops at **VS-D77**) | written before the MVP cut |
| **The drafts** | `spikes/s4-git/DRAFT-doc-rows.md` (VS-D78 MVP cut, VS-D79 ID format, VS-D80 driver protocol, §14 file formats) | the newest decisions, **not pasted into the plan yet** |
| **The code** | `crates/`, `drivers/python/`, `extension/`, `webview/` | follows the drafts |

**Rule while this is unresolved:** the drafts + the code win. Every row below says which file needs to change and where the fix text already exists.

Severity: 🔴 changes what the MVP is · 🟠 a rule or format differs · 🟡 small or wording only

---

## 1. What the MVP does (🔴)

| # | Topic | The plan says | The drafts / code say | Plan files to fix | Fix text |
|---|---|---|---|---|---|
| 1.1 | **Does the MVP edit code?** | "**Never edit the user's code by itself.**" ID mode **`none` only**; in-code IDs are stable (1.0) | **Yes, one line:** an `ID:` line in the docstring **when you attach a doc**, through `WorkspaceEdit`, undoable. ID mode is the only mode. Code: `persist()` writes into code before returning an ID; `node_toml.rs` says "the MVP always writes `id_in_code = yes`" | `04-roadmap/01-mvp-vs-stable.md` (release table, area table, "Why this cut") | DRAFT §11 |
| 1.2 | **"Why this cut"** | "`none` mode is the hardest case, so it goes first" | the opposite reason: the ID travels with the function, so the ladder isn't needed to follow renames | `04-roadmap/01` | DRAFT §11 |
| 1.3 | **How lost nodes are found in the MVP** | M4 = **step 0** (observed moves) + the **full ladder** (steps 1–6); "step 0 ships with the ladder, never after it" (VS-D76) | M4 = ladder **step 1 only** + VS Code's `onWillRenameFiles` / `onDidRenameFiles`. Steps 2–6 + step 0 → **P1**. Spike code: `Steps::mvp()` = step 1 only | `04-roadmap/01`, `02-milestones.md`, `06-core/05` | DRAFT §11, §12 |
| 1.4 | **Private docs, solo mode, `local.toml`, Share / Make private** | in the MVP (M3) | **P2** (post-MVP). The MVP has shared docs only | `04-roadmap/01` + `02`, `README.md` tree (B2 Q2, D1, F5, G), `00-requirements/02-user-journey.md`, `05-user-controls-and-config.md`, `03-functional-requirements.md` (M/S column) | DRAFT §11 "Follow-ups" |
| 1.5 | **Setup questions** | 3 questions: scope, team/solo, ID mode (`on-attach` / `all` / `none`) | **scope only** | `README.md` tree B2, `00-requirements/02` | DRAFT §11 |
| 1.6 | **Two-ID merge** | the strong merge is in the MVP | **out** of the MVP | `04-roadmap/01` (Lost nodes row, exit criterion 4) | DRAFT §11 |
| 1.7 | **The V-NOC trash UI** | restore from a **"Recently removed"** panel | two commands: **"V-NOC: Empty trash"** and **"V-NOC: Restore removed doc"**; the panel is **P4** | `README.md` tree E5, `04-roadmap/01`, FR-75 | DRAFT §11 |
| 1.8 | **Post-MVP shape** | milestones **M7** (in-code IDs) and parts of **M9** | a **P1–P4 list**, order set after tester feedback; M7 disappears | `04-roadmap/02-milestones.md` | DRAFT §11 |
| 1.9 | **MVP exit criteria 2 and 4** | the S4 refactor table must pass with no in-code IDs; no-data-loss covers Share / Make private / merge | docs survive *because the ID line moves with the function*; terminal/git renames → Missing → relink by hand. S4's table becomes the P1 suite | `04-roadmap/01` | DRAFT §11 |
| 1.10 | **When IDs reach code (milestones)** | 06-06: "M4 (derivation), M7 (in code)"; 07-03: docstring IDs are **stable** | **M3**, with the doc panel | `06-core/06-identity.md`, `07-drivers/03-python-driver.md` (MVP vs stable table) | DRAFT §11–12 |

## 2. The ID format (🟠, all already decided by the code)

| # | Topic | The plan says | The drafts / code say | Plan files to fix |
|---|---|---|---|---|
| 2.1 | **Formula** | VS-D50: `base32(blake3(path + "::" + qualname + "#" + occurrence)[..16])` | VS-D79: BLAKE3 **`derive_key`** with context `"v-noc node id v1"`, input `path \0 qualname \0 occurrence` (`crates/vnoc-domain/src/id.rs`) | `02-technology/05-identity-and-archive.md` §1 |
| 2.2 | **Collision counter** | `path::qualname#1#2`, `#3` | append `\0` + a counter **starting at 2** | same |
| 2.3 | **Name and encoding** | "ULID-style", `PermId`, "reuse the `ulid` crate's encoder" | **`NodeId`, not a ULID**; a hand-written Crockford encoder | 02-05 §1, `02-technology/04-storage-and-git.md` §4, `06-core/06-identity.md`, `04-roadmap/01` line 58 |
| 2.4 | **Old V-NOC UUID IDs** | "still read and kept" | **dropped**; `NodeId` refuses a UUID (a test pins it). A UUID `ID:` line is replaced in place | 02-05 §1 (delete the bullet), §2 (add the guard rule) |
| 2.5 | **Decision log rows** | VS-D50 and VS-D60 are not marked superseded | superseded by VS-D79 | `01-decisions/01-decision-log.md` (yours) |

Fix text for all of §2: DRAFT §13 (the sweep list).

## 3. File formats and settings (🟠)

| # | Topic | The plan says | The drafts / code say | Plan files to fix |
|---|---|---|---|---|
| 3.1 | **Version key in `vnoc.toml`** | `version = 1` | **`format = 1`** (the only required key; newer = refused, not rewritten) | `00-requirements/05-user-controls-and-config.md`, `04-roadmap/01` (frozen-format table) |
| 3.2 | **What `vnoc.toml` holds** | scope, ID default + `[[path]]` rules, sync mode, `[call_tree] depth` | **scope only** (`[index] include / exclude`); other keys are read and kept but not written | 00-05 |
| 3.3 | **Call-tree depth** | `[call_tree] depth = 3` in `vnoc.toml` | a **VS Code setting** | 00-05 (lines 45–46, 78–79, 100) |
| 3.4 | **`node.toml` fields** | no `occurrence`; `archived_at` only in stable | `occurrence` (written only when ≠ 1) and `archived_at` are **in the MVP format** (`node_toml.rs`) | `02-technology/04-storage-and-git.md` §4–5 |
| 3.5 | **`.vnoc/.gitignore`** | per mode: team `local/` + `local.toml`, solo `*` | the MVP writes one line: **`local/`**; never removes a user's line | 02-04, 00-05 |

Fix text: DRAFT §14.

## 4. How things are wired (🟠)

| # | Topic | The plan says | What was decided / built | Plan files to fix |
|---|---|---|---|---|
| 4.1 | **HEAD-change nudge** | the extension listens to **`vscode.git`** `repository.state.onDidChange` and sends `vnoc/headChanged` | S4 (C9): `vscode.git` is **unreliable** → **the core watches `.git` itself**; `vscode.git` is not used | `02-technology/01-architecture-at-a-glance.md` (flow 3), `02-running-rust-sidecar.md` line 97, `05-identity-and-archive.md` line 255, `08-extension/02-commands-and-views.md` item 6 |
| 4.2 | **Cargo workspace** | 02-07's Rust note: `members = ["crates/*", "drivers/*"]`, one shared `Cargo.lock`, `default-members` | `drivers/python` is **its own workspace** with its own toolchain and lockfile (pylspt's salsa pin). The root `Cargo.toml` **excludes** `drivers` | `02-technology/07-repo-and-drivers.md` (tree line 12, the note at lines 66–73). 06-01 and 10-01 are already right |
| 4.3 | **pylspt dependency** | git dep on `github.com/yaredtsy/pylspt` pinned by `rev` | a **path** dependency (`../../../pylspt`, TODO M1); pylspt's remote is actually **`yaredtsy/rust_python_parser`** | 02-07 §1 (the URL), or rename the repo |
| 4.4 | **Webview kept alive when hidden** | 02-03: hidden = destroyed by default; restore state + `keys.translate` instead of `retainContextWhenHidden` | the skeleton sets **`retainContextWhenHidden: true`** | code shortcut, fix in M2 (no plan change) |
| 4.5 | **Driver size** | NFR-32: "~38 MB uncompressed today (no strip/LTO)" | S2 measured **16.2 MB** release, 8.4 MB zipped | `00-requirements/04-non-functional-requirements.md` 🟡 |

## 5. The plan disagrees with itself (🟡)

| # | Where | What |
|---|---|---|
| 5.1 | `06-core/01-crates-and-domain.md`, `07-drivers/01-driver-protocol.md` | they cite **VS-D78, D79, D80**, which **don't exist in the decision log** yet |
| 5.2 | `05-spikes/04-git-and-matching.md` header | "Not done yet: the three gix lookups (C14), C9 and the pause rule": all three are done (DRAFT §9) |
| 5.3 | `05-spikes/01-hello-core.md` last line | "Still to check by hand: … panel restore, Restricted Mode, C18, C6, C3": the findings above it already answer these |
| 5.4 | `plan_vs/README.md` | the user tree shows the **full v1** (team/solo, ID modes, save-time IDs, `# FileID:`, Recently removed) without saying which parts are MVP |
| 5.5 | `03-next/02-phase-plan.md` | "M7–M10 → 1.0": M7 no longer exists after the cut |

## 6. Code vs plan: not contradictions, just not built yet

These look like gaps but they're only **unbuilt**: activation on `workspaceContains:.vnoc/vnoc.toml`, the real scope rule, SQLite, the TreeView, generated TS types (`webview/src/types.ts` is written by hand and says so).
The root `src/main.rs` (`Hello, world!`) isn't in any plan; it's a leftover.

---

## How to clear all of this (in order)

1. Paste **VS-D78, D79, D80** into `01-decisions/01-decision-log.md` (DRAFT §12, §13, §15); mark VS-D50 / VS-D60 superseded. → clears 5.1, 2.5
2. Paste DRAFT §4, §9, §10 into `05-spikes/04`; fix the last line of `05-spikes/01`. → clears 5.2, 5.3
3. Apply DRAFT §11 to `04-roadmap/01` and `02`, then its "Follow-ups" to `00-requirements/` and `README.md`. → clears §1, 5.4, 5.5
4. Run the DRAFT §13 sweep (ID format) and apply §14 (file formats). → clears §2, §3
5. Add a decision row for C9 ("the core watches `.git`, no `vscode.git`") and sweep the four files in 4.1; fix 02-07's workspace note and the pylspt URL. → clears §4

Steps 1–2 touch files you maintain yourself (log, checklist, spikes), so I left them alone. Steps 3–5 are normal doc edits and I can do them if you want.

Next: [`13-pylspt-whats-left.md`](13-pylspt-whats-left.md)
