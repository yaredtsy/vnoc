# 10. What's left, in order

Three groups: **housekeeping** (small, unblocks everything), **finish the MVP** (M0 → M6), and **after the MVP**.
Milestone contents follow the draft MVP cut (VS-D78).

## A. Housekeeping (do first, each is small)

| # | Task | Why |
|---|---|---|
| A1 | Delete the empty `crates/vnoc-domain/tests/fixtures/te.py` | it breaks `lifecycle.rs` and the extension test (they expect 1 file) |
| A2 | Paste **VS-D78, D79, D80** into the decision log (text ready in `spikes/s4-git/DRAFT-doc-rows.md` §12, §13, §15) | the log stops at D77 but code and 06-01/07-01 already depend on them |
| A3 | Paste the S4 findings (§4, §9, §10) into `05-spikes/04-git-and-matching.md` | the spike file still says gix/C9 "not done" |
| A4 | Apply the roadmap edits (§11) to `04-roadmap/01` and `02`, then the follow-ups in `00-requirements/` | the roadmap still says "MVP never edits code" |
| A5 | Run the D79 sweep (§13): `PermId` → `NodeId`, "ULID-style" → Crockford base32, drop UUID support | 02-04, 02-05, 06-06 describe the old formula |
| A6 | Delete or keep the root `src/main.rs` (`Hello, world!`) and the root `[package]` | leftover from `cargo new` |

## B. Finish the MVP

### M0 — skeleton + CI (🟡 almost done)
- [ ] CI workflow: build + test on every PR (cache `drivers/python/target`, the driver takes minutes)
- [ ] package **one VSIX** (macOS arm64) with `vsce package --target`; binaries inside the VSIX, ad-hoc signed
- [ ] extension finds the core in the VSIX's `bin/` (today: the repo's `target/debug`)
- [ ] write `08-extension/01-package-and-activation.md` and the CI part of `10-release/01`
- [ ] decide: native runners per OS vs cross-compiling (signing + build time lean native)

### M1 — index, cache, sidebar tree, Quick look (🟡 started)
- [ ] write `06-core/03-cache-schema.md`, `07-drivers/02-driver-host.md`, `08-extension/02`
- [ ] crates `vnoc-store` (trait) + `vnoc-store-sqlite` + `vnoc-store-memory`; one writer thread
- [ ] crate `vnoc-driver-host`: lookup in `<data dir>/vnoc/drivers/python/<ver>/`, **copy-on-first-run** under a lock, restart with back-off (driver uses `panic = "abort"`)
- [ ] real scope from `vnoc.toml` (`include` / `exclude`); `.vnoc/` found at the git root
- [ ] lazy tree loading (`tree.children`) instead of one big answer
- [ ] **sidebar `TreeView`**, **Quick look** (cache in `storageUri`, nothing in the repo), **Go to code**
- [ ] Restricted Mode = read-only; pylspt by git `rev`
- [ ] open spike items: C11 (quarantine/Defender), C12 (lock crate)

### M2 — the canvas (🟡 a basic one exists)
- [ ] write `09-webview/01` and `02`
- [ ] dagre layout; the **node card** ported from old V-NOC (expand · code · zoom dialog); **Canvas | Code** tabs
- [ ] **focus stack** (focus, Back, Clear, breadcrumbs); Reveal in V-NOC; auto / manual sync

### M3 — docs + the ID on attach (❌; domain types done)
- [ ] write `06-core/04-userdata-writer.md`, `06-core/06-identity.md`
- [ ] crate `vnoc-userdata`: `nodes/<shard>/<id>/`, `node.toml` (already in `vnoc-domain`), doc frontmatter, `.vnoc/.gitignore`
- [ ] guided setup (**scope only**), minimal `vnoc.toml` (`format = 1`)
- [ ] driver **`vnoc/planIds`** (port from spike S3) → `persist()` with a real `IdWriter` → `WorkspaceEdit` in the extension
- [ ] **doc panel**: read, add, remove

### M4 — docs survive (❌; logic proven in S4)
- [ ] write `06-core/05-sync-and-matching.md`
- [ ] crate `vnoc-sync`: scan / diff / reconcile; gix lookups; the `.git` watcher + rebase/merge pause; **VS-D74** (Missing vs not on this branch)
- [ ] ladder **step 1** only (restore a lost `ID:` line when `path::qualname` still exists)
- [ ] `onWillRenameFiles` / `onDidRenameFiles` → exact anchor updates
- [ ] relink by hand, archive (no expiry), remove to the V-NOC trash, **"Empty trash"** and **"Restore removed doc"** commands
- [ ] grouped Missing notification

### M5 — call trees (❌)
- [ ] write `06-core/07-call-tree.md`
- [ ] driver `vnoc/callTree` + `vnoc/expand` (port from S2), environment inputs
- [ ] lazy expand, depth as a **VS Code setting**, pulled-in files (dimmed), third-party = external leaves, markers

### M6 — editing in the canvas (❌; blocked)
- [ ] **run spike S5 first** (Monaco in a webview, the typing race, ⌘S keybinding)
- [ ] write `08-extension/03`, `09-webview/03`
- [ ] Monaco slice editor as a view of VS Code's document; doc panel edit mode; ⌘S / Ctrl+S

### ⭐ MVP 0.1 pre-release — exit criteria (from 04-01, reworded by D78)
Quick look < 30 s on ~1,000 files · refactor suite (ID moves with the function; terminal/git renames → Missing, never a wrong link) · branch switches don't prompt ·
no data loss (trash + restore + relink) · canvas editing never corrupts a file · 4 platforms, VSIX < 40 MB · two weeks of dogfooding.
Also write `10-release/02-testing.md`.

## C. After the MVP (P-list, order set after tester feedback)

| | Items |
|---|---|
| **P1** | no-ID mode + ladder steps 2–6 (twin and short-body rules) + step 0 (observed moves), together; Keep removed; weak-match prompts |
| **P2** | private docs, `local.toml`, solo mode, Share / Make private |
| **P3** | settings UI, `[[path]]` rules, monorepo overrides, `all` mode, save-time IDs |
| **P4** | archive expiry, doc freshness (VS-D77), provisional tier (VS-D75), Remove ID, `# FileID:`, "Recently removed" panel |

Then **M8** playground, **M9** polish (CodeLens, hover, search, `vnoc driver install <file>`), **M10** hardening (10k files, Linux arm64, Remote SSH / WSL / dev containers,
driver protocol frozen at 1, user guide) → **1.0**. Phase 4: driver downloads, TS/JS driver, `vnoc mcp` / `vnoc query`.

## Open questions still on the table

- `ts-rs` or `specta` to generate the webview's TypeScript types (today written by hand)
- one `graphChanged` with a diff, or invalidate-and-refetch?
- CI: native runners or cross-compile?
- Does `#:schema https://…` (a URL) work in TOML extensions? (only local paths tested)
- pylspt: a facade crate, an external cancel flag, lazy pagination (VS-D69)

Next: [`11-glossary.md`](11-glossary.md)
