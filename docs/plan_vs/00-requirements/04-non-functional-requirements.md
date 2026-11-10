# 00.04: Non-functional requirements

The numbers are **targets to confirm** 🟡. We'll measure them on a fixture corpus in phase 2
(a small repo, a ~1k-file repo, a ~10k-file repo).

---

## Speed

| # | Target | Why it matters |
|---|---|---|
| NFR-01 | Extension activation **< 50 ms** of extension-host time. The sidecar starts *after* activation, async. | Slow extensions get flagged by VS Code ("Show running extensions") and annoy everyone. |
| NFR-02 | Quick look on ~1k Python files: **canvas visible < 30 s** cold, **< 2 s** warm (cache present). | The quick checker's whole experience. |
| NFR-03 | Save → tree/canvas updated **< 1 s** for a single-file edit. | Feels live. |
| NFR-04 | Open a lazy call tree (depth 3): **< 1 s** for a typical function, warm. | Lazy only works if it's fast. |
| NFR-05 | Canvas stays smooth (≥ 30 fps pan/zoom) up to **~2,000 visible nodes**. Beyond that, collapse/virtualise. | xyflow limits. The focus stack keeps views small anyway. |

## Safety (never lose, never surprise)

| # | Rule |
|---|---|
| NFR-10 | **V-NOC never permanently deletes user-written content without a click** (VS-D52). Automatic steps (archive expiry of private docs) only move things to the **trash**; the fallback `.vnoc/local/trash/` is emptied only by the user. |
| NFR-11 | **Source files are only edited with consent**: an ID mode the user chose, or a per-node action. Multi-file edits are previewed (FR-21). IDs are only ever written inside the scope (VS-D21). |
| NFR-12 | **Rust never writes a source file, or any file a user might have open** (docs, playgrounds, `vnoc.toml`, `local.toml`). It returns edits; the extension applies them with `WorkspaceEdit`, so they show in **Undo**, open editors update, and there's never a "file changed on disk" conflict (VS-D16, VS-P24). |
| NFR-13 | The cache can be deleted at any time; nothing is lost. A corrupt or old-version cache is rebuilt automatically. |
| NFR-14 | A crash of the Rust sidecar never takes VS Code down; it's restarted automatically (with backoff), and the UI shows "V-NOC restarting". |
| NFR-15 | **No network access except driver downloads the user asked for** (signed manifest + minisign-verified files, VS-D29). No telemetry in v1. |
| NFR-16 | Respect VS Code **Workspace Trust** (VS-D18): in Restricted Mode, read-only indexing only. No source edits, no playground runs, no user-data writes, and `vnoc.server.path` is ignored from workspace settings. |
| NFR-17 | **A branch switch never prompts for docs that live on another branch.** A Missing notification needs docs orphaned in the checked-out commit (or a floating node whose branch is current or gone), and never appears while HEAD is detached (VS-D74). |
| NFR-19 | **Remove never destroys content git doesn't have**: anything not in HEAD goes to a trash first (VS-D35). |
| NFR-18 | A driver release can **never break an installed core**: cores only install driver versions whose protocol they speak (VS-D29.1). |

## Git-friendliness

| # | Rule |
|---|---|
| NFR-20 | Committed `.vnoc/` files are text, deterministic (sorted keys, stable order, `\n` line endings), and small. Re-saving an unchanged item produces **zero diff**. The only timestamp is `archived_at`, written once. Config files are rewritten with `toml_edit`, so the user's comments and order survive (VS-D47). |
| NFR-21 | One item per file (one `node.toml`, one doc, one playground), so parallel work on different nodes never touches the same file. |
| NFR-22 | No machine-specific data in committed files (no absolute paths, no usernames, no "last opened" timestamps). |

## Platforms

| # | Target |
|---|---|
| NFR-30 | macOS (arm64, x64), Linux (x64, arm64, glibc; musl/Alpine **[check]**), Windows (x64; arm64 later). |
| NFR-31 | VS Code engine `^1.9x` (pick the exact minimum in phase 2), and current Cursor / Windsurf / VSCodium. |
| NFR-32 | VSIX download size per platform **< 40 MB** 🟡. The bundled Python driver (pylspt) is the bulk: ~38 MB uncompressed today (no strip/LTO), so ~10–15 MB zipped is likely **[check]**. The core binary itself is small. |

## Maintainability

| # | Rule |
|---|---|
| NFR-40 | The Rust core has **no VS Code knowledge**. Only the TS extension knows VS Code APIs. (So `vnoc mcp`, a CLI or another editor can reuse the core.) |
| NFR-41 | Storage behind traits (as in `plan/03-storage/01`), so SQLite is an adapter, not the architecture. |
| NFR-42 | The protocol between the extension and the sidecar is typed on both sides from one source (Rust types → generated TS types). |

Next: [`05-user-controls-and-config.md`](05-user-controls-and-config.md)
