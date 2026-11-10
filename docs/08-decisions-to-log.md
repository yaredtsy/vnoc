# 08. For your decision log: what this plan changes

You keep `plan_vs/01-decisions/01-decision-log.md` yourself, so nothing was edited there. This file lists what would need a row, what each one
supersedes, my suggestions that still need your yes or no, and the open questions.

```
for the log
├── 1. decided on 2026-11-10 (your answers)
├── 2. suggestions waiting for your answer
├── 3. where this plan differs from plan_vs
└── 4. open questions
```

## 1. Decided on 2026-11-10

| Draft row | Decision | Supersedes / changes | Details |
|---|---|---|---|
| **F1** | Function and class IDs stay in the **docstring `ID:` line**. A **comment style** is added later as a project option (`[ids] style`); from then on drivers read both. Names reserved now: `[ids] style`, the comment text `vnoc: <ID>`, capabilities `ids.style.*`. | extends VS-D54 (the driver owns placement) | [`05-identity/02`](05-identity/02-functions-and-classes.md) |
| **F2** | Folders are tracked by a hidden **`.vnoc-id`** file inside the folder, created **only on attach**, written by the core. V-NOC still never touches `__init__.py`. | the rule "folders never get an ID on disk; a folder is matched by its children's hashes" in `02-technology/05` §2 and §4 (step 2, folder part) | [`05-identity/04`](05-identity/04-folders.md) |
| **F3** | The MVP's index is **in memory**, rebuilt at start. SQLite comes later, loaded into the same in-memory index. | moves VS-D46 (SQLite) out of M1; VS-D17's separate Quick-look cache is not needed in 0.1 | [`04-storage/02`](04-storage/02-index.md) |
| **F4** | **0.1 = the docs loop** (tree, canvas, attach, find again, Missing / Relink / Remove). Call trees = 0.2. Canvas editing = 0.3. | the draft VS-D78 cut (M5 and M6 leave the MVP) | [`02-mvp-and-releases.md`](02-mvp-and-releases.md) |

Also from your message, taken as decided:

| Draft row | Decision | Supersedes / changes |
|---|---|---|
| **F5** | **Tracking is optional.** `[ids] default = "on-attach"` (default) or `"none"` in 0.1. A marker that cannot be written does not block the attach (`id_in_code = "no"`). | the draft VS-D78 line "ID mode is the only mode" |
| **F6** | **File IDs** (`# FileID:` comment) are in 0.1 if the driver port is quick, else 0.1.x. No format difference either way. | VS-D59 moves from the post-MVP list (P4) to 0.1 |
| **F7** | **Doc panel in 0.1** (decided 2026-11-10, later the same day): on the right of the canvas, the doc rendered **read-only**, with a doc picker and a close button. **Edit** opens the `.md` in a VS Code tab and the panel is its live preview. Editing inside the panel is 0.3. | the doc panel of M3 is split: reading in 0.1, editing in 0.3 |
| **F8** | **The project root is chosen by the user** when the opened folder sits inside a bigger git repository: repository root (recommended) or the opened folder, asked once at the first attach. An existing `.vnoc/` found by walking up always wins. One project per window in 0.1. | the rule "`.vnoc/` is at the git root"; draft VS-D79's "path as git records it" becomes "path relative to the project root" | 
| **F9** | **UI additions for 0.1:** a "V-NOC" submenu in the code editor's right-click menu (Show on canvas, Focus on canvas, Show docs, Add doc) · the canvas starts with **one level open** · an **ⓘ** button opens the doc panel on the canvas · **docs beside the code**: an ⓘ in the editor title bar opens the doc panel as a tab next to the editor and follows the cursor. A tree click keeps jumping to the code (an editor cannot show a slice). | adds to M2/M3 |

## 2. Suggestions waiting for your answer

Each is independent. A "no" on any of them does not affect the rest of the plan.

| # | Suggestion | Why now | Where |
|---|---|---|---|
| **S1** | Add **`kind`** to `node.toml` (`function` / `class` / `file` / `folder`; unknown values kept). | It is additive, and it is the only way to say what a *Missing* node was. | [`04-storage/01`](04-storage/01-vnoc-folder.md) §3 |
| **S2** | **Normalise paths to NFC** inside `NodeId::derive` (hash input only). | Free before the first real ID exists, impossible after. Replaces "read paths from git". | [`05-identity/01`](05-identity/01-id-generation.md) §3 |
| **S3** | Replace `persist()` + `IdWriter` with a pure **`first_free_id()`** and an async attach flow that uses LSP's standard **`workspace/applyEdit`**. | `persist()` cannot await the real write, and F5 means an ID no longer always implies a write. | [`05-identity/01`](05-identity/01-id-generation.md) §4 |
| **S4** | **Never rewrite an anchor automatically** in 0.1; offer "Update stored locations" as a command. | Avoids the four VS-D74 rewrite rules and any surprise in `git status`. | [`05-identity/05`](05-identity/05-finding-nodes-again.md) §4 |
| **S5** | **No notifications in 0.1**, only a Missing view. That removes the need for Missing vs "not on this branch", `gix` and the `.git` watcher in 0.1. | The branch rule exists only to avoid a pop-up. | [`05-identity/05`](05-identity/05-finding-nodes-again.md) §2 |
| **S6** | `.vnoc/` is created **on the first attach**, with defaults. No guided setup and no separate Quick-look mode in 0.1. | With an in-memory index, browsing writes nothing anyway. | [`04-storage/01`](04-storage/01-vnoc-folder.md) §4 |
| ~~S7~~ | Replaced by **F7** below. | | |
| **S8** | New modules stay **inside `crates/vnoc`** (index, userdata, identity). No new crates until a second program needs one. | Six planned crates, one consumer. | [`03-repo-structure.md`](03-repo-structure.md) §3 |
| **S9** | The driver announces **`files.extensions`**, and uses the **protocol crate's types**. | Needed before a second driver; catches wire drift at compile time. | [`06-drivers.md`](06-drivers.md) §3 |
| **S10** | Drivers run from the VSIX's **`bin/`** only. Copy-on-first-run, the shared drivers folder and downloads wait until after 1.0. | changes VS-D30's "copied to the global drivers folder" for 0.x | [`06-drivers.md`](06-drivers.md) §5 |
| **S11** | Read-only code in cards with **Shiki HTML**; Monaco only for the card being edited (0.3). Measure first. | One Monaco instance per open card. | [`07-ui/02`](07-ui/02-design.md) §5 |
| **S12** | Duplicate IDs are **reported, not repaired** in 0.1. | VS-D71's automatic repair is a source edit nobody asked for. | [`05-identity/05`](05-identity/05-finding-nodes-again.md) §3 |
| **S13** | pylspt keeps its own repo (already decided); the **outline walk, per-def `plan_ids`, edit text + span and header-comment placement move into pylspt**, and it re-exports the ruff types the driver needs. | The re-export blocks the git pin (R4); the rest avoids porting spike workarounds into the product. | [`06-drivers.md`](06-drivers.md) §7 |

## 3. Where this plan differs from `plan_vs`

For when you (or I, if you ask) sync the plan docs. Nothing here is a new contradiction inside this folder; it is the list of places where `plan_vs`
would say something else.

| Topic | `plan_vs` / drafts say | This plan says |
|---|---|---|
| MVP contents | M0–M6 (draft VS-D78) | 0.1 docs loop; 0.2 call trees; 0.3 editing (F4) |
| Cache | SQLite in M1 (VS-D46) | memory in 0.1; SQLite when measured (F3) |
| Folder identity | children's content hashes; no ID on disk | `.vnoc-id` (F2) |
| File IDs | post-MVP (P4) | 0.1 or 0.1.x (F6) |
| ID modes in the MVP | on-attach only (draft VS-D78) | `on-attach` and `none` (F5) |
| Setup | Quick look (cache in `storageUri`, VS-D17) + guided setup | nothing written until the first attach (S6) |
| Lost nodes in the MVP | ladder step 1 + rename listeners + VS-D74 + archive + grouped notice | marker → anchor → Missing view; Relink; Remove (S4, S5) |
| Anchor rewrites | automatic under VS-D74's rules | by Relink or by command (S4) |
| Doc panel | in the MVP (M3), with editing | 0.1: read-only panel, editing in a VS Code tab; 0.3: editing in the panel (F7) |
| Crates | `vnoc-store`, `-sqlite`, `-memory`, `-driver-host`, `-userdata`, `-sync` | modules of `crates/vnoc` (S8) |
| Driver location | bundled **and** copied to a global folder (VS-D30) | bundled only, for 0.x (S10) |
| `node.toml` | no `kind` | `kind` (S1) |
| ID hash input | the path exactly as git records it (draft VS-D79) | the same path, NFC-normalised (S2) |
| `persist()` | writes the ID before returning it | `first_free_id()` + async flow (S3) |

`report/12-contradictions.md` is still accurate for everything else. Its "how to clear all of this" list is cheaper if you first decide which parts of
`plan_vs` become history: most of §1 there (what the MVP does) is replaced by F4 anyway.

## 4. Open questions

| # | Question | My lean |
|---|---|---|
| **Q1** | Docs on the **project root** (the root folder has no path, so it cannot be a node today). | Later: a fixed node, or simply `.vnoc/README.md`. Not in 0.1. |
| **Q2** | Should the tree show **non-code files** (README, YAML) so they can be documented? | A "show all files" toggle, after 0.1. The identity model already handles them (attached by path). |
| **Q3** | Does ruff / pydocstyle complain about an `ID:` line in a docstring with default rules? | Check before 0.1; it affects how F1's comment style is prioritised. |
| **Q4** | Does `vscode-languageclient` refuse a versioned `workspace/applyEdit` when the document has moved on? | Test it in Step 2. If not, compare versions in the extension before applying. |
| **Q5** | The pylspt repo is named `rust_python_parser` on GitHub, and `vendor/ruff` must be reachable from a git dependency. | Checked: `vendor/ruff` is committed (no submodule), so a git dependency carries it. Use the real URL, pinned by `rev`; the driver's own three ruff path dependencies must go first. Steps in [`06-drivers.md`](06-drivers.md) §7. Still blocks CI (R4) until done. |
| **Q6** | macOS Gatekeeper and Windows Defender on binaries inside a VSIX (spike item C11, still open). | Test in Step 0 on a second machine. May require signing before a public release. |
| **Q7** | The real start-up time of the in-memory index on a 1,000-file and a 10,000-file repo. | Measure at the end of Step 1. This number decides whether SQLite is 0.2 or later. |
