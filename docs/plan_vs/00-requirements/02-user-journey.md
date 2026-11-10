# 00.02: The user journey, branch by branch

This expands the tree in the [README](../README.md). For each branch: **what the user does → what they see → what happens underneath**.
The *how* is in `02-technology/`. FR-xx numbers point to [`03-functional-requirements.md`](03-functional-requirements.md).

> **🧩 VS Code note: the places an extension can put UI.**
>
> | Name | What it is | We use it for |
> |---|---|---|
> | **Activity Bar icon** | the vertical icon strip on the far left | the V-NOC icon, which opens our sidebar |
> | **TreeView** | a native, themed tree in the sidebar | C1 tree, the Missing / Not on this branch / Archived lists |
> | **WebviewPanel** | an editor tab showing *your own* HTML/JS (an iframe) | C2 canvas, C3 call tree |
> | **Command** | a named action (`vnoc.setUp`), shown in the Command Palette (⇧⌘P) | every action |
> | **Context menu** (`menus` in `package.json`) | right-click entries | "Set up here", "Attach doc", "Add permanent ID" |
> | **Status bar item** | text in the bottom bar | "V-NOC: indexing 40%", "V-NOC: 12 changed · Sync" |
> | **Notification** | the toast at the bottom right, with buttons | the Missing prompt (E3) |
> | **QuickPick** | the dropdown at the top (like ⌘P) | setup questions, "Relink to…" |
> | **CodeLens** | small clickable text above a line of code | `📄 2 docs · call tree` above a `def` |
>
> **Rule we follow:** native where it's enough, a webview only for the canvas.

> **🧩 VS Code note: Workspace Trust.** When someone opens a folder they just cloned, VS Code asks "Do you trust the authors?". If they say no,
> the workspace is in **Restricted Mode**. Extensions declare what they do there in `package.json`:
> ```json
> "capabilities": { "untrustedWorkspaces": {
>     "supported": "limited",
>     "description": "In Restricted Mode, V-NOC only indexes and shows your code. It doesn't edit source files or run playgrounds.",
>     "restrictedConfigurations": ["vnoc.server.path"] } }
> ```
> `restrictedConfigurations` matters: a malicious repo could set `vnoc.server.path` in its `.vscode/settings.json` to run its own binary. Listed settings are
> ignored from the workspace until it's trusted. Code checks `vscode.workspace.isTrusted` and listens to `onDidGrantWorkspaceTrust` (VS-D18).

---

## A. Opens a folder

| Step | User sees | Underneath |
|---|---|---|
| A1 | `.vnoc/vnoc.toml` exists and `index.on_open = true`: status bar "V-NOC: indexing…", then the sidebar fills. | Activation via `workspaceContains:.vnoc/vnoc.toml` → spawn the core → `initialize` (root, storage, trust, **current git HEAD**, proxy) → driver found in the global drivers folder → cache reused or rebuilt. |
| A1-untrusted | Same, plus a line in the sidebar: "Restricted Mode: read-only". | Indexing is read-only, so it's allowed. No source edits, no playground runs, no `.vnoc/` writes except the cache (VS-D18). |
| A1-closed-head | HEAD moved while VS Code was closed. | Nothing special: the next sync applies the same rule as any other (VS-D74). It never depends on how HEAD moved. |
| A1-no-driver | The project lists a language whose driver isn't installed. | "This project uses TypeScript. Install the TypeScript driver (12 MB)? [Install] [Not now]". Never automatic (02-07). v1 only has Python, which is bundled. |
| A2 | No `.vnoc/`: nothing pops up. The V-NOC Activity Bar view says "Set up a project or take a quick look". | Nothing runs until the user touches it (FR-04). |

## B. Sets up a project

### B1. Quick look
1. Right-click a folder or file ▸ **V-NOC: Quick look**.
2. The canvas opens within seconds. Banner: "Quick look: nothing is saved in your repo. [Make it a project]".
3. Underneath: scope = what was clicked, ID mode = never. **The cache lives in VS Code's per-workspace storage (`context.storageUri`)**, and no `.vnoc/` is created (VS-D17).
4. **Make it a project** → B2 with the scope pre-filled. That's the moment `.vnoc/` is created (the cache moves into `.vnoc/local/cache/`).

### B2. Guided setup
Right-click ▸ **V-NOC: Set up here** (or the Command Palette). Setup asks only **what the user knows** (VS-D19, VS-P26):

```
Q1  What should V-NOC index?
    ● src/                          ← what you right-clicked (the scope; .vnoc/ still goes at the git root)
    ○ the whole workspace
    ○ choose folders…

Q2  Who will see V-NOC's notes?
    ● My team (commit .vnoc/ with the code)
    ○ Just me (only the local side is created; V-NOC stays out of git entirely)

Q3  May V-NOC add ID comments to your code?   (change any time, and per node)
    ● On attach: only where I attach docs   ← default for "My team"
    ○ All: every file/class/function in scope
    ○ None                          ← default for "Just me", with the reason:
                                       "ID comments are committed with your code and teammates would see them."
```

How call trees load is **not** a question. Lazy is the only mode, and the depth is a setting (F3).
Then `.vnoc/vnoc.toml` (solo: `local.toml`) and `.vnoc/.gitignore` (team: `local/` + `local.toml`; solo: `*`) are written and indexing starts. In a monorepo, setup also proposes one `[[path]]` section per detected package (`pyproject.toml`, `package.json`). Q3 = All → a preview ("312 files will get an ID line") must be confirmed first (FR-21).

### B3. Hand-written config
The user writes `.vnoc/vnoc.toml`. Saving it → "Config changed: re-index? [Yes]". A JSON schema gives autocomplete (FR-12). `local.toml` works the same way for personal overrides.

## C. Explores

| | Does | Sees |
|---|---|---|
| C1 | Opens the sidebar | `Root ▸ folders ▸ files ▸ classes ▸ functions` for the scope. Badges: 📄 docs, 🔒 private docs, 🆔 ID, ⚠ missing. **Pulled-in** files (outside scope, reached by a call tree) appear dimmed at their real path. |
| C2 | Opens the canvas on a node | Each node is the old v-noc **card**: **[▸]** expand/collapse children, **[</>]** show its code inside the card, **[⤢]** zoom into a big code dialog, a link icon (solid = in-code ID, outlined = `node.toml`) and a doc count. Panel tabs **Canvas \| Code** (Code = the selected node's code, full size). Code is **editable** in the card: it's a view of VS Code's own document, so undo and unsaved changes are shared with a regular editor; no completions in v1, so "Open in editor" is there for bigger edits (VS-D48, VS-D57). Click = select. **Double-click = focus**: only that node's children are shown. |
| C2 ★ | **Focus stack** | Breadcrumb `Root / src / orders.py / OrderService`. **Back**, **Clear**, click a crumb to jump. One stack per canvas tab; survives hidden tabs and restarts (FR-44). |
| C3 | Opens a function's call tree | 3 levels (the depth setting). Frontier nodes show "+". Expanding loads more under *that* node. Calls into repo files outside the scope (including vendored code) are followed and those files pulled in. Calls into **third-party** code (outside the repo, `site-packages`, `.venv`, `node_modules`) are **external leaves**: resolved, not indexed (VS-D32). Known limit: a path through a library back into your code (a framework calling your view, a callback passed to `map`) stops at the library leaf. |
| C4 | Go to code | The editor opens at the exact lines. The code is never copied; a hover preview reads the live buffer. On an external leaf, the **library file opens read-only**. |
| C5 | Reveal in V-NOC (from the editor) | The tree and canvas select the cursor's function. |

## D. Adds knowledge

### D1. Attach a doc
Right-click a node ▸ **Attach doc** ▸ **Shared** or **Private 🔒** (default: shared in team mode, private in solo; the user picks once per doc).
Or from the canvas **doc panel** ([📄 Docs] toggle): **[+ Add doc]**. A Markdown file is created on that side (`nodes/…` or `local/nodes/…`) and opened **in the doc panel**, next to the graph. It's rendered to read and switches to Markdown source with **[✎]**; **[🗑]** removes a doc (trash rule). "Open beside" gives a regular VS Code editor. Private ones are marked 🔒 (VS-D61).
**Share this doc** / **Make private** move it later (VS-D26).

Under **`on-attach`**, the first doc on an in-scope node also adds its `ID:` line (placed by the driver; Python: in the docstring) (a `WorkspaceEdit`; if the file has unsaved changes, the line joins the buffer and goes out with the user's next save).

### D2. Playground
Right-click a function/class ▸ **New playground** → a `.py` snippet bound to the node (shared or private, like docs) → **Run** in a VS Code terminal with the Python extension's interpreter.
Output isn't stored. **Disabled in Restricted Mode.**

### D3. Per-node ID
Right-click any file / class / function ▸

- **Add permanent ID**: writes one `ID:` line for that node (Python: into its docstring, created if missing; for a **file**, a `# FileID:` comment at the top), in any mode, even `none`. On a pulled-in node it first offers "add this file to the scope?" (VS-D21).
- **Remove permanent ID**: removes the exact span the driver recorded, **only if its text still matches** (VS-D49). If it doesn't, nothing changes and V-NOC shows you the line. No pattern search, so nothing else in the docstring can be touched. The node keeps its docs (found through `node.toml` now) and gets `id_in_code = "never"`, stored where a teammate's sync will see it (VS-D39).
- If someone (a docstring rewrite, an AI agent, a teammate) deletes ID lines by hand, the next sync finds the nodes anyway and asks once: **Restore / Keep removed**.

Right-click a **folder** ▸ **IDs for this folder and below** ▸ All / On attach / None / Inherit (a `[[path]]` rule, VS-D42). Folders themselves never get an ID in code.

Both are `WorkspaceEdit`s (undoable), never direct disk writes (VS-D16). Disabled in Restricted Mode.

## E. Keeps coding

| | Situation | User sees |
|---|---|---|
| E1 | Edit / save / rename / move / delete, **auto sync** | Tree/canvas update within ~1 s. |
| E1-manual | Same, **manual sync** | Status bar `V-NOC: 12 changed · Sync`. Call trees stay correct (checked against file hashes when read). Structure, new nodes, Missing and IDs wait for Sync (VS-D22). |
| E1-save | Saves a file under an **`all`** rule with a new function | The ID line appears **as part of that save** (one save, one undo step), on manual saves and on auto-save when leaving the editor (`FocusOut`). Auto-save during a typing pause (`AfterDelay`) skips it. Functions with syntax errors are skipped (VS-D31). |
| E2 | A documented node moved or was renamed and was found again | Nothing. The docs are still there. |
| E2-editor | Renames a function **and** rewrites its body in the editor, then saves | Nothing. V-NOC watched the edit (step 0), so the docs follow exactly (VS-D76). |
| E2b | The same rename + rewrite happens **outside** the editor (a pull, an agent, VS Code closed) | **Missing → Find it…**, with the top candidate preselected (VS-D75). |
| E3 | A documented node is gone and its docs are **orphaned in this commit** (an edit, a pull, a rebase…) (VS-D74) | **Missing** (⚠) + one grouped notification per sync (none while HEAD is detached): "2 documented nodes can't be found [Review]" → **Archive / Find it… / Remove** per node. |
| E4 | **Switches branch**, and a documented node isn't on the new one | **Nothing pops up** when the node's docs live on another branch: it moves to a quiet "Not on this branch" list and comes back on its own when the code does. If the checked-out commit has the node's `node.toml` but not its code, it's Missing, as in E3 (VS-D74). |
| E4b | **Pulls a teammate's deletion** | Missing + the normal prompt, unless the teammate already archived or removed that node (their decision arrives with the pull). |
| — | A node **without** user data disappears | Nothing. It's cache. |

**Archive** = soft delete in place (`status = "archived"` in the node's one `node.toml`), listed under "Archived". If that `node.toml` is shared, teammates see it archived too. After 90 days (configurable): private-only nodes expire to the V-NOC trash, shared ones prompt (VS-D56).
**Find it…** = relink right now. **Relink to…** (from Archived) = pick a node and the docs follow it. **Remove** = after a confirmation listing the files, every file (private or shared, committed or not) moves to the **V-NOC trash**, `.vnoc/local/trash/` (VS-D70). **Restore** from the "Recently removed" list moves them back; if the node no longer exists, it comes back as Missing. Nothing is deleted until you click **"Empty V-NOC trash"**.

## F. Changes settings any time

Command Palette ▸ "V-NOC: Settings", or edit `vnoc.toml` (shared) / `local.toml` (just me). The settings UI asks **"for the team or just me?"** for every change and writes to that file.

| | Change | Effect |
|---|---|---|
| F1 | ID mode → None (project or folder) | Existing IDs stay; no new ones. |
| F2 | ID mode → All / On attach | Preview (on a team repo with `all`: "big diff, commit it separately"), then inject, skipping `never` nodes and pulled-in nodes. |
| F3 | Scope, excludes, **call-tree depth**, **auto / manual sync**, open-time indexing | Re-index of what changed. Documented nodes that leave the scope become pulled-in or Missing, never deleted. |
| F4 | Rebuild index | `local/cache/` deleted and rebuilt. User data untouched. |
| F5 | **Team ↔ solo** | Changes where new docs go, the ID default, and `.vnoc/.gitignore` (`local/` + `local.toml` ↔ `*`) (VS-D58, VS-D63). Sharing a single doc never touches `.gitignore`. Solo → team: `vnoc.toml` is created (preview); docs stay private until "Share all docs…" moves the folders you tick into `nodes/`. Team → solo: shared folders stay shared (moving them would delete them from the repo for teammates; "Make all private…" warns about that). Team → solo: `.gitignore` becomes `*`; files already committed stay in history (we say so). |

## G. Shares with git

- **Team mode:** `vnoc.toml` and `nodes/` are committed; `local.toml` and `local/` are ignored by `.vnoc/.gitignore`. **Solo:** only the local side exists and `.vnoc/.gitignore` is `*`, so nothing from `.vnoc/` shows in `git status`. **Share / Make private** move a doc or a whole node folder between `local/nodes/` and `nodes/` (VS-D58).
- The repo's root `.gitignore` is never touched.
- A teammate clones → their cache is built locally → shared `node.toml` files (and in-code `ID:` lines) find their nodes → same shared docs, plus their own private ones.
- Two people documented the same function at once → the **same derived ID** → the same folder and the same `ID:` line → git merges it cleanly (VS-D50). Only after a rename on one side do two IDs appear; V-NOC merges them on the next sync (VS-D40).
- Merge cases: `02-technology/04`.

Next: [`03-functional-requirements.md`](03-functional-requirements.md)
