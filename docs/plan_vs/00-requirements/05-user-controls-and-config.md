# 00.05: What the user controls, and where it's stored

Decision: VS-D47 (configuration). Four places, chosen by **"who should this apply to?"**

| Level | Stored in | In git? | Examples |
|---|---|---|---|
| **Project (team)** | `.vnoc/vnoc.toml` | ✅ in team mode | scope, ID default + path rules, sync mode, call-tree depth |
| **Project (just me)** | `.vnoc/local.toml` | ❌ never | my override of any key above (in solo mode: the only config file) |
| **Per node** | that node's one `node.toml` (side picked by 02-04 §5) | shared side: ✅ in team mode | `id_in_code`, archived |
| **Personal, all projects** | VS Code settings (`vnoc.*`) | ❌ | CodeLens on/off, canvas layout, proxy (from `http.proxy`) |

**Precedence:** per-node `id_in_code` > `local.toml` > `vnoc.toml` > built-in defaults. Inside one file, the **most specific `[[path]]` section** beats the general keys.

**The two TOML files are the only source of truth.** The settings UI reads and writes them and holds no state of its own. Every field in the UI has a
**"Team / Only me"** toggle that decides which file it's written to.

> **🧩 VS Code note: `contributes.configuration`.**
> Personal settings are declared in `package.json` under `contributes.configuration`. VS Code shows them in its Settings UI and the extension reads them with
> `vscode.workspace.getConfiguration("vnoc")`. We use this **only** for per-person preferences. Project settings live in `vnoc.toml` / `local.toml` because they must also work
> in CLI/MCP mode, where there is no VS Code.

---

## `vnoc.toml`: draft 🟡

```toml
#:schema https://vnoc.dev/schema/vnoc-1.json
# .vnoc/vnoc.toml — the team's settings (committed in team mode)
version = 1

[project]
sharing = "team"                  # "team" | "solo"   → where NEW docs go, the ID default, .vnoc/.gitignore (VS-D58, VS-D63)

[index]
on_open = true                    # index when the folder opens
sync    = "auto"                  # "auto" | "manual"   (VS-D22)
include = ["src/**", "tools/**"]  # the SCOPE: where call trees START (and where IDs may go). Default ["**"]
exclude = ["**/migrations/**"]    # on top of .gitignore
respect_gitignore = true
languages = ["python"]            # which drivers are needed (installing one is always the user's choice)

[ids]
default = "on-attach"             # "on-attach" | "all" | "none"   (solo default: "none")

[call_tree]                       # lazy is the only mode (VS-D19)
depth  = 3                        # levels per open / expand
budget = 5000                     # max nodes per tree before "truncated" (plan/P20)

[retention]                       # VS-D56
archive_days = 90                 # archived nodes expire after this (private → V-NOC trash, shared → prompt)
cache_days   = 30                 # unused cache versions are evicted after this

# ── Monorepo / per-folder rules: one file, [[path]] sections. Most specific path wins. ──
[[path]]
path = "services/billing"         # proposed by setup because it has its own pyproject.toml
ids  = "all"

[[path]]
path = "services/billing/legacy"
ids  = "none"
```

- **`vnoc.toml` holds only V-NOC's own settings:** scope, IDs, sync, call trees. Drivers read the project's **existing** tool config themselves (`[tool.ty]` in `pyproject.toml`, the venv,
  `tsconfig.json` later). We don't copy it.
- **Monorepo:** `[[path]]` sections in this one file replace the earlier `[[package]]` idea. **No cascading per-folder config files.** Setup **auto-detects packages**
  (`pyproject.toml`, `package.json`) and proposes `[[path]]` entries.
- **Effective config view:** right-click any folder ▸ "V-NOC: Effective config" shows each value and where it came from (default / `vnoc.toml` / a `[[path]]` / `local.toml`).
  It also warns about rules outside the scope (they do nothing).

## `local.toml`: same keys, my overrides

```toml
#:schema https://vnoc.dev/schema/vnoc-1.json
# .vnoc/local.toml — never committed
[index]
sync = "manual"                   # I prefer to sync by hand on this big repo

[call_tree]
depth = 5
```

Any key may appear here. In **solo** mode there's no `vnoc.toml`, so `local.toml` holds everything, including `project.sharing = "solo"` (VS-D37).
In team mode, "my new docs default to private" is the personal setting `vnoc.docs.defaultSide`, not a `local.toml` override of `sharing`.

## How the files are read and written

| | |
|---|---|
| Writing | **`toml_edit`** (keeps the user's comments, key order and formatting). Through `WorkspaceEdit`, since the file may be open (VS-P24). |
| Schema | A published JSON Schema; the first line `#:schema <url>` lets TOML extensions (Even Better TOML, Tombi) give autocomplete **[check] C6**. |
| Hand edits | Picked up by the file watcher and reloaded. |
| Invalid file | Show the error **with its line** (a diagnostic in the TOML file + a status-bar warning) and **keep using the last valid config**. |
| Restricted Mode | **Every file that comes with the repo is untrusted input**, committed or ignored: `vnoc.toml`, `local.toml`, `.vscode/settings.json`, `pyproject.toml` (VS-D51). While the workspace is untrusted, **only VS Code user-level settings may name an executable** (a driver path, an interpreter). Any such field in a repo file is ignored, with a warning on that line. |

> **🦀 Rust note: `toml_edit` vs `toml`.**
> The `toml` crate parses into your structs (`serde`) and writes them back **from scratch**: the user's comments and ordering are gone. `toml_edit` keeps a
> **document tree** that remembers every comment and space, so you can change one value and write the rest back byte-for-byte:
> ```rust
> let mut doc: toml_edit::DocumentMut = text.parse()?;
> doc["call_tree"]["depth"] = toml_edit::value(5);
> let new_text = doc.to_string();        // everything else untouched
> ```
> Read with `toml` + `serde` into typed settings (easy to use), and write with `toml_edit` (safe to save). The core returns `new_text` as a `WorkspaceEdit`.

## Setup asks only three things (VS-D47, confirms VS-P26)

1. **What to index?** (the scope; call trees are always lazy, so there's no eager/lazy question)
2. **Team or just me?** (team → new docs shared by default; just me → only the local side is created, `.gitignore` = `*`)
3. **IDs in code?** ○ On attach (team default) ○ All ○ None (solo default). The question shows what an ID looks like in this language (Python: an `ID:` line in the docstring; a `# FileID:` comment at the top of a file).

Plus, in a monorepo: "Found 3 packages. Add a `[[path]]` section for each?" (pre-ticked, no new question to think about).

## Personal settings (VS Code `settings.json`) 🟡

| Setting | Default | |
|---|---|---|
| `vnoc.codeLens.enabled` | `true` | `📄 2 docs · call tree` above defs |
| `vnoc.canvas.layoutDirection` | `"LR"` | or `"TB"` |
| `vnoc.canvas.openOnFocus` | `"sameTab"` | focus in the same tab, or a new one |
| `vnoc.docs.defaultSide` | `"auto"` | `auto` (team → shared, solo → private), `shared`, `private` |
| `vnoc.drivers.manifestUrl` | (built in) | mirror for companies (signature still checked against the embedded key) |
| `vnoc.server.path` | — | dev only: run a local core build. **Restricted in untrusted workspaces** (`restrictedConfigurations`). |
| `vnoc.trace.server` | `"off"` | log JSON-RPC traffic to the Output panel |

Proxy: not our own setting. The extension passes VS Code's `http.proxy` / `http.proxyStrictSSL` to the core (VS-D29.6).

## Per-node and per-folder controls (right-click)

| Action | Writes | Allowed when |
|---|---|---|
| Add permanent ID | 1 `ID:` line placed by the driver (Python: in the docstring) via `WorkspaceEdit` + `node.toml` (`id_in_code = "yes"`) | trusted; node in scope (otherwise "add to scope?"); a node the driver can carry (Python v1: file, class, function) |
| Remove permanent ID | removes the driver's recorded span **only if its text still matches exactly**, else shows the line (VS-D49); `id_in_code = "never"` | trusted; the node has an in-code ID |
| IDs for this folder and below ▸ All / On attach / None / Inherit | a `[[path]]` section (Team / Only me toggle) | trusted |
| Remove IDs in this folder | exact-span removals, with a preview; `never` on each; mismatches listed | trusted |
| Attach doc / New playground | a file on the chosen side (+ 1 ID line if the rules say so) | trusted |
| Share this doc / Make private | moves the file between sides (and `node.toml` if 02-04 §5 says so) | trusted |
| Archive / Relink / Remove | `node.toml` (+ files on Remove) | trusted |
| Update anchors | `node.toml`'s `path` / `qualname`, from the move sync found but didn't write (02-04 §5, VS-D74) | trusted; a move already committed in HEAD |

## Personas → defaults

| Persona | Does | Gets |
|---|---|---|
| Quick checker | Quick look | Nothing in the repo, nothing asked. |
| Documenter (team) | Setup: scope, team, defaults | `on-attach`: one ID line per documented node. |
| Solo user on a team repo | Setup: "Just me" | Local side only, `.vnoc/` invisible to git, no ID comments, private docs. |
| Monorepo lead | Setup accepts the detected `[[path]]` sections | `all` in one service, `none` in legacy code, one config file. |
| Careful one | `default = "none"`, manual sync, IDs by hand | Full control; prompts only for documented nodes that really disappeared. |

Next: [`../01-decisions/01-decision-log.md`](../01-decisions/01-decision-log.md)
