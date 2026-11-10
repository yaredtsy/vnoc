# 00.03: Functional requirements

Priority: **M** = must for v1 · **S** = should (v1 if cheap, else v1.1) · **L** = later (the design leaves room).
Grouped by area, with gaps left for inserts. ~~Struck~~ rows were dropped in review; the reason is given.

---

## 0x: Project lifecycle and trust

| # | Requirement | P |
|---|---|---|
| FR-01 | A **project** = a git repo with **`.vnoc/` at its git root** (the workspace folder root without git) (VS-D36, VS-D55), or a quick-look session. Right-clicking a sub-folder sets the scope; it never creates a nested `.vnoc/`. | M |
| FR-02 | Multi-root workspace: one project per **git root**; roots inside the same repo share one `.vnoc/` (VS-D55). | S |
| FR-03 | `index.on_open = true` → indexing starts on open without a click (also in Restricted Mode, read-only). | M |
| FR-04 | No process and no files in folders without `.vnoc/` until the user asks. | M |
| FR-05 | Close/reopen restores the graph from the cache without a full re-index. | M |
| FR-06 | One writer per project; a second window is read-only and says so. | S |
| FR-07 | **Workspace Trust** (VS-D18): untrusted → read-only indexing only; no source edits, no playground runs, no user-data writes. Declared in `capabilities.untrustedWorkspaces` (`"limited"`), with `vnoc.server.path` in `restrictedConfigurations`. | M |

## 1x: Setup and configuration

| # | Requirement | P |
|---|---|---|
| FR-10 | **Quick look**: no questions, no IDs, **nothing in the repo**; cache in `context.storageUri` (VS-D17). | M |
| FR-11 | **Guided setup** asks only what the user knows: **scope**, **team or just me**, **may V-NOC edit your code** (VS-D19, VS-P26). | M |
| FR-12 | Hand-written `vnoc.toml` / `local.toml` work the same as setup; JSON-schema autocomplete. | M |
| FR-13 | **Make it a project** (from quick look) creates `.vnoc/` and keeps the cache. | M |
| FR-14 | Scope = include/exclude globs; `.gitignore` respected by default. | M |
| FR-15 | Every setting can be changed later and takes effect without restarting VS Code. | M |
| FR-16 | **`local.toml` overrides `vnoc.toml`** key by key; the settings UI asks "team or just me?" per change. | M |
| FR-17 | **Sync mode auto / manual** (VS-D22). Manual: changes are counted in the status bar (`12 changed · Sync`). | M |
| FR-18 | `.vnoc/.gitignore` is written at setup from the mode (team: `local/` + `local.toml`; solo: `*`) and changes **only on a mode switch**, never when a doc is shared or made private (VS-D63). The repo's root `.gitignore` is never touched (VS-D58). | M |

## 2x: Identity

| # | Requirement | P |
|---|---|---|
| FR-20 | ID modes `on-attach` / `all` / `none`: a project default + `[[path]]` rules (most specific path wins) + per-node `id_in_code`. Default `on-attach` (team), `none` (solo, with a one-line reason) (VS-D42). | M |
| FR-21 | Any ID operation touching **more than one file** shows a preview and needs confirmation. | M |
| FR-22 | Rules only add IDs: switching to `none` never removes existing ones. | M |
| FR-23 | ID syntax and placement are **the driver's job** (VS-D54). Python: an `ID: <id>` line in the def/class docstring (as pylspt does today), and a `# FileID: <id>` comment at the top of a file, after shebang/encoding and any leading comment block (VS-D59). Folders never get in-code IDs; `__init__.py` is never created or edited (VS-D41). Old V-NOC `ID:` lines are *read* as a migration aid only. | M |
| FR-24 | **Per-node Add permanent ID**, any mode, for in-scope nodes. | M |
| FR-25 | **Per-node Remove permanent ID**: removes the driver's recorded span **only if its current text still matches exactly**; otherwise shows the line and changes nothing; never a pattern search (VS-D49); sets `id_in_code = "never"`; docs stay attached (VS-D43). | M |
| FR-26 | "Remove IDs in this folder" (exact span per ID, preview, mismatches listed). | S |
| FR-27 | Without an ID (or a lost comment): the six-step ladder (path + name, content hash, git rename, body hash, similarity, Missing) (VS-D44). A lost comment → one grouped "Restore / Keep removed" notice. | M |
| FR-2D | **Step 0, observed moves:** renames and rewrites made in the editor (tracked ranges) and file renames from VS Code are applied exactly at sync, before the ladder. Anything that isn't clearly an editor edit falls back to the ladder (VS-D76). Ships with the ladder. | M |
| FR-28 | Duplicate in-code IDs: the copy matching `node.toml`'s anchor keeps it; else the copy whose current `path::qualname` hashes to the ID; else, if a node folder exists, **ask**; else the copy first by path, then line. The other gets a new ID (VS-D71, VS-D74). | M |
| FR-29 | **Rust never writes source files.** All source edits are text edits applied by the extension with `WorkspaceEdit`. Files that don't parse are skipped (VS-D16). | M |
| FR-2A | **IDs only inside the scope.** Pulled-in nodes get path identity only (VS-D21). | M |
| FR-2B | New functions under `all` get IDs **as part of the user's save** (`onWillSaveTextDocument` + `waitUntil`) for `Manual` and `FocusOut` saves; `AfterDelay` is skipped; only functions that parse without errors (VS-D31). | M |
| FR-2C | New IDs are **derived by hash** from `path::qualname#occurrence` (a counter if taken), 26 characters, created only on attach or `all` injection and saved permanently at once (never only in SQLite) (VS-D50). | M |

## 3x: Indexing and sync

| # | Requirement | P |
|---|---|---|
| FR-30 | Structure index of the scope: folders, files, classes, functions. | M |
| FR-31 | Incremental re-index on save/create/delete/rename (auto mode). | M |
| FR-32 | **Lazy call trees, the only mode**: computed when opened, to depth N (setting, default 3); expand loads N more under one node. | M |
| ~~FR-33~~ | ~~Eager call trees~~: dropped. Context-sensitive trees multiply at every level (VS-D19). Replaced by FR-3A. | — |
| FR-34 | Cached expansions are invalidated when a file they depend on changes (`reaches` + stored file hashes). | M |
| FR-35 | Progress in the status bar, cancellable. | M |
| FR-36 | Rebuild index (deletes the cache only). | M |
| FR-37 | Per-file index errors never stop the whole index. | M |
| FR-38 | **Calls leaving the scope are resolved**; a first-party target file's structure is indexed when a call tree first reaches it ("pulled in"). **Third-party** = outside the repo root or under `site-packages` / `.venv` / `node_modules`; vendored code in the repo is first-party. Third-party calls are **external leaves**: resolved, never indexed (VS-D32). | M |
| FR-3B | Jump to code on an external leaf opens the library file **read-only**. | M |
| FR-3C | Documented limit: paths through a library back into project code stop at the library leaf. | M (documentation) |
| FR-39 | In manual sync mode, cached call-tree expansions are **checked against current file hashes when read**, so trees are never stale. | M |
| FR-3A | Background call-tree prefetch (low priority). | L |

## 4x: Views and navigation

| # | Requirement | P |
|---|---|---|
| FR-40 | Native sidebar tree, badges (📄 🔒 🆔 ⚠), pulled-in nodes dimmed. Sections: Missing, Not on this branch, Archived. | M |
| FR-41 | Canvas webview on any node: layout, pan/zoom, select, side panel. | M |
| FR-42 | **Focus stack** per canvas tab: focus, Back, Clear, breadcrumb jump. | M |
| FR-43 | Call-tree view with expand-on-demand; markers for recursion / cut / unresolved / external. | M |
| FR-44 | Canvas state survives hidden tabs and VS Code restarts. | M |
| FR-45 | Go to code from any node. | M |
| FR-46 | Reveal in V-NOC from the editor cursor. | M |
| FR-47 | CodeLens `📄 N docs · call tree`. | S |
| FR-48 | Hover preview from the live buffer. | S |
| FR-49 | Search nodes by name. | S |
| FR-4A | Follows the VS Code theme. | M |

## 5x: Documents

| # | Requirement | P |
|---|---|---|
| FR-50 | Attach any number of Markdown docs to a folder / file / class / function (in scope or pulled in). | M |
| FR-51 | Docs are `.md` files in `.vnoc/`, editable **in the canvas doc panel** (Markdown source, as a view of VS Code's document) and in a normal editor tab (VS-D61). | M |
| FR-52 | The canvas has a **doc panel** (toggle in the toolbar and on the card's 📄 badge) that shows the selected node's docs from both sides (private ones marked), rendered, with **Add** (shared / private) and **Remove** (trash rule) (VS-D61). | M |
| FR-53 | Docs on call nodes. | L, confirm |
| FR-54 | Docs are never deleted by sync. | M |
| FR-5A | **Code is editable wherever it's shown** (card, zoom dialog, Code tab), as a view of VS Code's document; **⌘S / Ctrl+S** in the canvas saves the focused editor's document (VS-D62). | M |
| FR-55 | Each doc is **shared** or **private**; Rust decides the file location from that (VS-D24). | M |
| FR-56 | **Share this doc / Make private** move files between sides (VS-D26). | M |
| FR-57 | Sharing / un-sharing **moves** a doc or a node folder between `local/nodes/` and `nodes/` (merging into an existing folder). "Share all docs…" and "Make all private…" preview every folder that moves; the latter warns that teammates lose those docs on commit (VS-D58). | M |
| FR-58 | **Doc freshness:** saving a doc stores the node's normalised body hash in its frontmatter; when the code changes, the doc shows "Code changed since this doc was last edited", with **Still accurate**. Functions and methods only; docstring changes don't count (VS-D77). | M |

## 6x: Playground

| # | Requirement | P |
|---|---|---|
| FR-60 | Playground snippet bound to a function/class (shared or private). | S |
| FR-61 | Run it in a VS Code terminal with the Python extension's interpreter. | S |
| FR-62 | Disabled in Restricted Mode. | M (if FR-60 ships) |
| FR-63 | No stored run history. | M |

## 7x: Missing, not on this branch, archive, relink

| # | Requirement | P |
|---|---|---|
| FR-70 | A documented node absent after the ladder is **Missing** (badge), never silently deleted, when there's **no git repo**, when its `node.toml` is **committed in HEAD's tree**, or when it floats and HEAD is on its last-seen branch or that branch was deleted (VS-D74). | M |
| FR-71 | One grouped, non-blocking notification per sync, **only for Missing**, and **not while HEAD is detached** (the nodes are still listed) (VS-D74). | M |
| FR-72 | Missing and not-on-this-branch nodes return to Active on their own when the code reappears. | M |
| FR-73 | Archive (soft delete), stored on the doc's side. | M |
| FR-74 | Relink to another node from Missing / Not on this branch / Archived. | M |
| FR-75 | Remove with a confirmation listing the files. **Every** Remove (private or shared, committed or not) moves the files to **`.vnoc/local/trash/<timestamp>_<node-id>/`** with a `trashed.toml` (`node`, `side`, `file`, `removed_at`), never the OS trash (VS-D70). That folder is **never auto-cleared** (VS-D52). | M |
| FR-7A | A **"Recently removed"** list offers **Restore**, which moves the files back and never overwrites (a path that exists again is a reported conflict); if the node no longer exists it comes back as **Missing**. The trash's size is shown with an **"Empty V-NOC trash"** button, the only permanent delete, and it needs a click (VS-D70). | M |
| FR-76 | Nodes without user data are never prompted for. | M |
| FR-77 | **HEAD watch**: `vscode.git` notifies; Rust reads only a path lookup in HEAD's tree, the current branch and the local branch list. No ancestry test, no history (VS-D74). | M |
| FR-78 | The cache stores **`last_seen(id, branch)`**, updated on each sync where the node is present and HEAD is on a branch (never while detached). A HEAD change made while VS Code was closed is handled by the same rule. | M |
| FR-79 | A **floating** node (its `node.toml` isn't committed in HEAD's tree) is **"not on this branch"**, with **no notification**, when it has no `last_seen` record or its last-seen branch still exists and isn't the current one. Otherwise it's Missing. A renamed branch looks deleted (accepted) (VS-D74). | M |

## 8x: Sharing / git

| # | Requirement | P |
|---|---|---|
| FR-80 | User data is deterministic text, one item per file, in folders named by the node's ID, sharded by the last 2 characters (VS-D38). | M |
| FR-81 | V-NOC writes `.vnoc/.gitignore` itself (FR-18). | M |
| FR-82 | A fresh clone rebuilds the cache and re-attaches shared docs. It uses `node.toml`'s anchors as written and runs ladder **steps 1 and 6 only** (no git history); stale anchors show **Missing with "Find it…"** (VS-D72). | M |
| FR-83 | Two IDs resolving to one node: **strong match** → folders merged automatically (shared beats local; shared vs shared: smaller ID wins); **weak match** → ask; same-name file collision → `vscode.diff` (VS-D40). | M |
| FR-85 | **One `node.toml` per node**: shared if the node has any shared doc, or `id_in_code = "never"` under a shared ID mode; otherwise local (VS-D39). | M |
| FR-84 | The placement rules (VS-D24) are enforced in the Rust write path; the UI never chooses file paths. | M |

## 9x: Drivers and integrations

| # | Requirement | P |
|---|---|---|
| FR-90 | Works in Cursor / Windsurf / VSCodium (Open VSX). | M |
| FR-91 | `vnoc mcp` for agents. | L |
| FR-92 | More languages as separate driver executables (VS-D28). | L |
| FR-93 | `vnoc query …` CLI (JSON output). | L |
| FR-94 | v1: the bundled Python driver is **copied into the global drivers folder on first run** and run from there (VS-D30). | M |
| FR-95 | Drivers downloaded by the core: protocol-pinned, from a **signed manifest**, **minisign**-verified, atomic install with a lock, versions side by side, proxy-aware (VS-D29.1–6). | M for the install/lookup code path (used by FR-94); download source L |
| FR-96 | `vnoc driver install <file>` for offline machines, same signature check. | S |
| FR-97 | Installing a driver is always a user choice, never triggered by a cloned config alone. | M |

Next: [`04-non-functional-requirements.md`](04-non-functional-requirements.md)
