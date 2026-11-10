# 04.01: MVP vs stable: what ships when

**Phase 2 starts here.** Phase 1 (folders `00`–`03`) said *what* we build and *why*. Phase 2 says *in what order*, tests the risky parts (`05-spikes/`),
and designs each program in detail (`06-core/` … `10-release/`).

Three releases, each with one job:

| Release | Version | Channel | Its one job | For |
|---|---|---|---|---|
| **MVP** | `0.1` → `0.x` | Marketplace + Open VSX, marked **pre-release** | Prove the core loop: *browse → canvas → attach docs → refactor → docs are still there*. **Never edit the user's code by itself.** | early users, us (dogfooding) |
| **Stable** | `1.0` | normal release channel | Everything in `00-requirements` marked v1: in-code IDs, playground, polish, hardened on big repos and every platform. | everyone |
| **Later** | `1.x` | — | Phase 4: driver downloads, TS/JS, MCP / CLI, prefetch, groups, history… | |

> **🧩 VS Code note: pre-release versions.**
> `vsce publish --pre-release` (and `ovsx publish --pre-release`) puts a version on the **pre-release channel**: users opt in with "Switch to Pre-Release Version" on the
> extension page. The Marketplace wants normal numbers (`major.minor.patch`, no `-beta`), and suggests **odd minors for pre-release, even for release**
> (`0.1.x`, `0.3.x` … pre-release, `1.0.x` stable). The pre-release flag lets the MVP reach real users without everyone getting it by default.

---

## What's in which release

| Area | MVP (0.x) | Stable (1.0) | Later |
|---|---|---|---|
| **Language** | Python, driver bundled in the VSIX + copied to the global drivers folder (VS-D30) | same | downloadable drivers, TS/JS (VS-D29) |
| **Setup** | Quick look; guided setup (scope, team/solo, ID mode); `vnoc.toml` / `local.toml` hand-written + reload | + effective-config view, monorepo package detection (`[[path]]` proposals) | |
| **Explore** | sidebar tree, canvas with the **node card** (expand · code · zoom), **Canvas \| Code** tabs, **focus stack**, go to code, reveal in V-NOC | + CodeLens, hover preview, search | |
| **Call trees** | lazy, depth setting, expand, pulled-in files, external leaves | same, tuned on big repos | background prefetch (VS-O8) |
| **Docs** | **doc panel**: read, edit, add, remove; shared / private; Share / Make private (folder moves) | + doc freshness (VS-D77) | docs on call nodes (VS-O6) |
| **Editing in the canvas** | code + docs editable as views of VS Code's document; ⌘S / Ctrl+S | same | completions in the webview |
| **Identity** | ID mode **`none` only**: permanent IDs live in `.vnoc` folder names; nodes are re-found by **step 0** (observed moves, VS-D76) and the **matching ladder** | + **in-code IDs**: `on-attach` / `all`, `[[path]]` rules, Add / Remove (exact span), Restore / Keep removed, save-time IDs, `# FileID:` | |
| **Lost nodes** | Missing / Not on this branch (VS-D74), Archive / Relink / Remove, V-NOC trash + Restore, **strong** two-ID merge | + **weak**-match merge prompts, archive expiry (90 days), "Empty V-NOC trash" UI | provisional matches (VS-D75: built, off in v1) |
| **Sync** | auto / manual, startup mtime scan, hash-validated cache | same | Rust file watcher for CLI mode |
| **Playground** | — | ✅ (VS-P12) | stored runs |
| **Trust** | Restricted Mode = read-only, no executables from repo files | same | |
| **Drivers offline** | — (bundled anyway) | `vnoc driver install <file>` | download + manifest + minisign |
| **Platforms** | macOS (arm64, x64), Linux x64, Windows x64; local folders | + Linux arm64, **Remote SSH / WSL / dev containers** tested | vscode.dev (VS-O3) |
| **Agents** | — | — | `vnoc mcp`, `vnoc query` (VS-O1) |

### Why this cut 🟡 (confirm in review)

- **MVP never writes to the user's code on its own.** The only code edits are the ones the user types into a card. That's the safest first impression for a tool people
  install on a team repo, and it means the MVP can't produce a surprise diff.
- **`none` mode is the hardest identity case, so it goes first.** If docs survive refactors and branch switches *without* in-code IDs, in-code IDs (stable) only make an already
  working system stronger. Solo users default to `none` anyway (VS-D25).
- **Editing in the canvas is in the MVP** because it's one mechanism ("a view of VS Code's document") that both the doc panel and the code card need. Building it once, early,
  avoids a second editor design later.
- **Playground waits for stable** (checklist A7). It's independent of everything else and easy to add.

## The promise that starts at the MVP: the data format

People will **commit** `.vnoc/nodes/` from the first preview. So these are frozen at `0.1` and only change with a migration:

| Frozen at MVP | Defined in |
|---|---|
| `.vnoc/` layout, sharding by the last 2 characters | 02-04 §2, §4 |
| `node.toml` fields, doc frontmatter (`node:`, `hint:`) | 02-04 §4–5 |
| The ID formula (blake3 → 26 chars, ULID-style encoding) | 02-05 §1 |
| `vnoc.toml` / `local.toml` keys that exist in the MVP (`version = 1`) | 00-05 |

Everything else (the cache schema, the editor ↔ core protocol, the driver protocol) can change freely until 1.0, because nothing outside one machine depends on it.
**At 1.0** the driver protocol is frozen as protocol `1` (VS-D29.1).

## Exit criteria

**MVP (`0.1`) ships when all of these are true:**

1. **Quick look** shows a canvas in **< 30 s** on a ~1,000-file Python repo, writing nothing into the repo (success criterion 1, `00-01`).
2. **The refactor suite** (05-spikes/04, turned into tests): docs survive rename-function, move-file, rename-folder, move-and-edit and cut/paste-between-files, with
   **zero wrong silent matches**. Cases the ladder can't solve end in Missing → Find it…, never in a wrong attachment.
3. **Branch switches don't prompt for docs that live on another branch**; a pulled deletion does (VS-D74). Tested with VS Code open and closed.
4. **No data loss**: a test suite for Remove / Make private / Share / two-ID merge where every lost file is recoverable (git or the V-NOC trash).
5. **Editing in the canvas** never corrupts a file in the "type in the card and the editor at once" test (05-spikes/05).
6. Runs on the four MVP platforms; VSIX < 40 MB per platform (NFR-32).
7. **Two weeks of dogfooding** on this repo and pylspt with no open data-loss bug.

**Stable (`1.0`) ships when:**

1. Everything in the stable column works, and in-code IDs pass a **two-person team test** (two clones, two branches, merges, the same function documented on both sides).
2. The NFR speed targets (NFR-01..05) hold on a **~10,000-file** repo.
3. Remote SSH, WSL and a dev container pass the full suite, including Remove + Restore through `.vnoc/local/trash/` on each OS (spike S3 verified macOS only).
4. The driver protocol is frozen as `1`; the cache schema has a rebuild-on-mismatch path tested from every MVP version.
5. **Four weeks on the pre-release channel** with no P0 bug (data loss, corruption, a hang that blocks VS Code).
6. A short user guide: setup, docs, IDs, team vs solo.

Next: [`02-milestones.md`](02-milestones.md)
