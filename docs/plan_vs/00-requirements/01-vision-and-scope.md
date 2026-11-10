# 00.01: Vision and scope

## One sentence

> **V-NOC for VS Code turns a Python project into a navigable graph (folders, files, classes,
> functions and their call trees) right inside the editor. Notes and playgrounds you attach to
> a node stay attached while the code moves.**

The vision is the same as the old product (`~/Documents/Programing/ide/v-noc/doc/01-vision.md`):
"programming like Google Maps", killing the side quest. The **delivery** is what changed:

| Old V-NOC | V-NOC for VS Code |
|---|---|
| Separate web app + Python/Rust server + TerminusDB | An extension inside the editor people already use |
| Code copied into the DB | **Code never stored.** We keep paths, spans and hashes, and read the code from disk when needed |
| Project lives in a DB server | Project lives in the repo's **`.vnoc/`** folder, next to the code |
| User must accept in-source IDs | **IDs are optional.** The user chooses, and can change their mind |
| Every call tree computed up front | **Lazy call trees**: computed when opened, 3 levels at a time |

---

## Who it's for (personas)

These drive the controls in [`05-user-controls-and-config.md`](05-user-controls-and-config.md).

| Persona | Wants | Must not get |
|---|---|---|
| **The quick checker** | "Show me how this folder hangs together." Done in 30 s. | Questions, config files, IDs in their code, files in the repo. |
| **The documenter** | Attach notes to the functions that matter and have them survive refactors. | Their docs silently lost. Hundreds of IDs injected into code they didn't touch. |
| **The team** | Docs shared through git, the same view on every machine. | Merge conflicts on a binary DB. A teammate's cache breaking theirs. |
| **The careful one** | Control over what gets indexed and written. | Surprise edits to source files. |

---

## v1 scope

### In

| Area | What |
|---|---|
| Language | **Python only**: the pylspt driver, a separate executable bundled in v1. More languages are later downloadable drivers (VS-D28–D30). |
| Index | Folders, files, classes, functions **of the scope** (structure, up front). Call trees **lazy only**; they may reach outside the scope and pull those files in. Auto or manual sync. |
| Views | Native sidebar tree · canvas webview with the old v-noc **node card** (expand toggle, code toggle, zoom dialog) and **Canvas \| Code** tabs · **focus stack** · call tree · jump to code · reveal from editor |
| User data | Documents (Markdown) attached to nodes · playground snippets attached to nodes |
| Identity | Three ID modes (`none` / `on-attach` / `all`) as a project default plus per-folder `[[path]]` rules, changeable any time. **Per-node override**: add or remove (exact span only) a permanent ID on any file/class/function. IDs are derived by hash, and the driver places them (Python: a docstring `ID:` line for defs, a `# FileID:` comment at the top of a file). Folders never get one in code. Path + fallback matching when there's no ID. Missing → Archive → Relink flow. |
| Setup | Quick look (nothing in the repo) · guided setup (scope, team or just me, may V-NOC edit code) · hand-written `vnoc.toml` / `local.toml` |
| Sharing | `.vnoc/` (at the git root) has a **shared** side (committed in team mode) and a **local** side (never committed: private docs, overrides, cache). **Solo** mode creates only the local side and keeps all of `.vnoc/` out of git. |
| Branches | Branch switches don't prompt for docs that live on another branch: those nodes are "not on this branch", silently (VS-D74). |
| Trust | Restricted Mode = read-only indexing. |
| Hosts | VS Code, plus forks that read Open VSX (Cursor, Windsurf, VSCodium) |

### Out (v1), with the reason

| Out | Why | Hook that keeps it possible |
|---|---|---|
| History / versioning | Git already does versioning for the user data, because it's text files in the repo. | All user data is text in `.vnoc/`, so git history *is* the history. |
| Agent / walkthroughs | Big feature; needs the graph first. | An MCP mode of the same binary (see `02-technology/06`). |
| Tasks | Not core to "navigate + document". | Attach to nodes by ID like docs. |
| Logs (`vn_logger`) | Needs runtime instrumentation. | Attach to nodes by ID. |
| Groups (virtual folders) | Not in your brainstorm. **Confirm** in the review. | `plan/01-data-model/03` parent/owner model can be added later without changing IDs. |
| Other languages | Ship Python first. | `LanguageDriver` trait (`plan/04-drivers-and-ids/01`). |
| Remote / web (vscode.dev) | Needs a WASM build. | Open item VS-O3. |

---

## Success for v1 (how we know it worked)

1. A quick checker can right-click a folder and see a canvas in **under 30 seconds** on a ~1,000-file Python repo, with nothing written into the repo.
2. A documenter can attach a doc, rename the function, move it to another file, and the doc **is still attached**, in all three ID modes. (In "never" mode the move may need one confirmation click.)
3. Two people on two branches both add docs, merge, and get **no conflicts in `.vnoc/`** beyond normal text conflicts in the same doc.
4. Deleting `.vnoc/local/cache/` and reopening gives **the same graph and the same attached docs**.
5. Switching branches back and forth **never shows a notification**, and every doc is attached again after switching back.
6. Typing and saving in a file with unsaved changes **never** shows "file changed on disk" because of V-NOC.

Next: [`02-user-journey.md`](02-user-journey.md)
