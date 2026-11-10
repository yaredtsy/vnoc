# 02.06: Shipping fast, and growing into Cursor, opencode, pi, Claude Code…

## 1. What we ship

```
vnoc-<version>@darwin-arm64.vsix        (a .vsix is a zip)
├── package.json                  manifest: commands, menus, views, settings, activation events
├── dist/extension.js             the TS extension host code, bundled (esbuild)
├── dist/webview/main.js|css      the React canvas, bundled (Vite)
├── bin/vnoc                      the Rust core for THIS platform (small)
├── drivers/python/0.3.0/         v1 only: the bundled Python driver + its .minisig, copied on
│                                 first run into <data dir>/vnoc/drivers/ and run from there (VS-D30)
└── schemas/vnoc.schema.json      vnoc.toml / local.toml autocomplete
```

Later languages are **not** in the VSIX: the core downloads them on the user's choice from a signed manifest (VS-D29, [`07-repo-and-drivers.md`](07-repo-and-drivers.md)).

Size: the bundled Python driver is the bulk. Today's pylspt release binaries are **~38 MB** uncompressed (`pylspt/target/release/pylspt`), without
`strip`/LTO. With `strip = true`, `lto = "thin"`, `codegen-units = 1` and zip compression, **~10–15 MB per VSIX** is a reasonable
guess **[check]** in phase 2, well inside NFR-32.

> **🧩 VS Code note: platform-specific extensions (VS-P13).**
> `vsce package --target darwin-arm64` builds a VSIX marked for one platform. You publish one per target
> (`darwin-arm64`, `darwin-x64`, `linux-x64`, `linux-arm64`, `win32-x64`, …), and the Marketplace gives each user the right one
> automatically. rust-analyzer and ruff ship exactly like this. Without it you'd have to put every OS's binary into one
> VSIX (5× the size) or download the binary on first run (needs network, breaks offline installs).

## 2. Where we publish

| Store | Who installs from it | Tool |
|---|---|---|
| **VS Code Marketplace** | VS Code | `vsce publish --target …` (needs a publisher + an Azure DevOps token) |
| **Open VSX** | Cursor, Windsurf, VSCodium, Gitpod, Theia… **[check]** each one's current default | `ovsx publish --target …` (Eclipse account + token) |

The same VSIX files go to both stores. So "**works in Cursor**" (FR-90) costs one extra CI step.

## 3. The fastest path to a public preview

| Step | What | Why it's quick |
|---|---|---|
| 1 | **Pre-release channel** (`--pre-release`) from the first public build | Users opt in; we can ship weekly without a "1.0" promise. |
| 2 | CI in the monorepo: `release-core.yml` builds `vnoc` + the bundled Python driver per target → `vsce package --target` → both stores. `release-driver.yml` builds a driver per target → signs (minisign) → uploads archives → regenerates and signs `manifest.json` | One tag = one release: `core-v…` or `driver-python-v…`. Runners: macOS (arm64 + x64), Ubuntu (x64 + arm64), Windows x64. |
| 3 | Start with **macOS arm64 + Linux x64 + Windows x64**, then add the rest | Covers most users on day one. |
| 4 | Feature order = the milestone list in [`../04-roadmap/02-milestones.md`](../04-roadmap/02-milestones.md): tree → canvas + focus stack → docs + `node.toml` → lazy call tree → IDs → playground | Each step is usable on its own. |

## 4. Integrations later (VS-O1, FR-91, FR-93)

The key enabler is already decided: **the core knows nothing about VS Code** (NFR-40). One binary, several front doors:

```
                       ┌──────────────── vnoc (same binary) ────────────────┐
   VS Code / Cursor ──►│ vnoc lsp      the extension's sidecar              │
   agents (MCP)     ──►│ vnoc mcp      MCP server over stdio                │──► .vnoc/ (cache + user data)
   pi / scripts / CI──►│ vnoc query …  CLI with JSON output                 │
                       └─────────────────────────────────────────────────────┘
```

| Tool | How it would connect | Notes |
|---|---|---|
| **Claude Code** | `claude mcp add vnoc -- vnoc mcp` | MCP over stdio. |
| **Cursor** (its agent) | `.cursor/mcp.json` → `vnoc mcp` | The extension itself already works there (Open VSX). |
| **opencode** | `opencode.json` → `mcp` section → `vnoc mcp` | **[check]** config format. |
| **pi** (coding agent) | **CLI**: `vnoc query call-tree --entry … --json`, documented for the agent | **[check]**: pi is known for preferring CLI tools over MCP; the CLI door covers it either way. |
| **VS Code Copilot agent mode** | The extension can register our MCP server with VS Code's MCP API, so it appears without any config | **[check]** API name/stability in phase 2. |

What agents would get (first idea): `tree(node)`, `call_tree(entry, depth)`, `docs(node)`, `attach_doc(node, markdown)`, `find(name)`.
Agents read the graph instead of grepping, and write their findings as docs attached to the right node.

**Two processes on one project** (VS Code open + an agent running `vnoc mcp`): the MCP process sees the writer lock in `.vnoc/local/cache/` (VS-P16) and opens the
cache **read-only**. Writing docs is still safe, because docs are just text files on the shared or local side; the VS Code side notices the new file
through its file watcher and picks it up. **Drivers are shared too**: both processes use the same `<data dir>/vnoc/drivers/`, and the install lock (VS-D29.4) stops them
from installing the same driver at the same moment. This is the reason drivers are downloaded by the core and not shipped as VS Code extensions: an agent in a terminal needs them with no editor involved.

## 5. Other editors (VS-O4)

Because the sidecar speaks LSP framing (VS-P7), Zed, Neovim and JetBrains (via LSP4IJ) could launch `vnoc lsp` too. They'd get the
standard bits (CodeLens, hover) immediately; the canvas would need its own UI per editor. That's a later decision; nothing in v1 blocks it.

Next: [`../03-next/01-review-checklist.md`](../03-next/01-review-checklist.md)
