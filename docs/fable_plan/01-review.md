# 01. Review: the plan and the code as they are today

```
review
├── 1. what is good (keep it)
├── 2. what worries me about the plan
├── 3. what blocks a release today      ← four packaging problems
├── 4. findings in the code
│   ├── core (Rust)
│   ├── driver
│   ├── extension
│   └── webview
└── 5. what I would stop doing
```

## 1. What is good

| Thing | Why it is good | Where |
|---|---|---|
| **The domain crate has no I/O** | `NodeId`, `RepoPath`, `LocationKey`, `NodeToml` can be tested without a disk, a driver or VS Code. Golden values come from an independent Python script. | `crates/vnoc-domain/` |
| **`node.toml` is edited in place** | It is held as a `toml_edit` document, so keys this version does not know are kept. A newer V-NOC can add a field and an older one will not destroy it. This is the single most important property for "incremental update to stable". | `node_toml.rs` |
| **Invalid states do not compile** | `RepoPath::new` refuses `..`, backslashes and absolute paths. `NodeId` can only be read or made by `persist`. | `location.rs`, `id.rs` |
| **The driver never writes a file** | It returns text edits; VS Code applies them. Undo works, and there is no "file changed on disk" fight. | `07-drivers/01` |
| **stdout is the protocol** | `#![deny(clippy::print_stdout)]` in both programs. A stray `println!` is a compile error, not a 2 a.m. bug. | `main.rs` ×2 |
| **Unknown messages never kill a process** | Catch-all handlers in the core, ignore-unknown in the driver, and a test for it. | `server.rs`, `tests/lifecycle.rs` |
| **The spikes** | The five riskiest questions were tested before product code. S5 is the only one not run, and 0.1 does not need it. | `spikes/` |
| **Restricted Mode from day one** | Binary paths cannot come from a cloned repo's settings. | `extension/package.json` |

## 2. What worries me about the plan

**a) It is three plans.** `plan_vs/` stops at VS-D77. The newest decisions (D78–D80) live in a draft inside a spike folder. The code follows the draft.
`report/12-contradictions.md` lists about 30 places where they disagree. Every hour spent keeping these in sync is an hour not spent shipping.
→ *Suggestion:* freeze `plan_vs/` as history. Keep **one** short "what is true now" file per domain, written after the code, not before.

**b) The MVP is six milestones wide.** Tree, canvas, docs, IDs, lost-node handling, git branch logic, call trees, a Monaco editor wired to VS Code's document,
SQLite, guided setup, four platforms. That is a 1.0, not an MVP. → You already cut it today (F4).

**c) Machinery is designed for problems no user has yet.** Examples: the six-step matching ladder, the provisional tier, key-move tables, copy-on-first-run with
a lock, signed driver manifests, Missing vs "not on this branch". Each is a reasonable answer. None is needed to learn whether people want the product.
→ [`02-mvp-and-releases.md`](02-mvp-and-releases.md) says which release each one belongs to.

**d) Seven crates are planned, three exist.** `vnoc-store`, `vnoc-store-sqlite`, `vnoc-store-memory`, `vnoc-driver-host`, `vnoc-userdata`, `vnoc-sync` are
all in the "what's left" list. → Make them **modules** inside `crates/vnoc` first. Split a module into a crate only when a second program needs it.
([`03-repo-structure.md`](03-repo-structure.md) §3 explains the difference.)

## 3. What blocks a release today

You cannot build a VSIX that works on another machine yet. These four are small, but they are the real distance to "released":

| # | Problem | Where | Fix |
|---|---|---|---|
| R1 | **The webview is loaded from outside the extension folder.** `distUri` = `extension/../webview/dist`. A VSIX only contains `extension/`. | `extension.ts` `distUri()` | Build the webview **into** `extension/media/webview/` (Vite `outDir`), and point `distUri` there. |
| R2 | **The binaries are found in the repo's `target/debug`.** | `extension.ts` `binary()` | Look in `<extension>/bin/` first, then fall back to the dev path. The package step copies release binaries into `extension/bin/`. |
| R3 | **The extension is not bundled.** It is compiled with plain `tsc`, so `vscode-languageclient` must ship as `node_modules`. | `extension/package.json` | Bundle with esbuild into one `dist/extension.js` (spike S1 already has an `esbuild.js` to copy). Then package with `vsce package --no-dependencies`. |
| R4 | **The Python driver depends on a folder outside this repo.** `pylspt = { path = "../../../pylspt" }`. CI and other machines do not have it. | `drivers/python/Cargo.toml` | Push pylspt and depend on it by `git` + `rev`. Note its remote is `yaredtsy/rust_python_parser`, not `pylspt`, and `vendor/ruff` must come along (as a submodule or committed) **[check]**. |

There is also no CI (`.github/` does not exist). That is not a blocker for a first local VSIX, but it is for publishing four platforms.

## 4. Findings in the code

Severity: 🔴 will cause a wrong result · 🟠 will hurt soon · 🟡 small

### Core (Rust)

| # | Sev | Finding | Suggestion |
|---|---|---|---|
| C1 | 🔴 | **Paths are relative to the opened folder, but IDs hash the path relative to the git root.** `structure()` uses `params.root` (the opened folder). If the user opens `repo/services/api`, the same function gets a different location key, and later a different ID, than when they open `repo`. | Before the first ID is written: find the git root (walk up until a `.git` entry exists; no `gix` needed) and make every `RepoPath` relative to it. |
| C2 | 🟠 | **`persist()` cannot be used as it is.** `IdWriter::write_id` is a normal (blocking) function, but the real write is a round trip: core → driver (plan the edit) → VS Code (apply it) → answer. That needs `await`. | Split it in two: a pure "first free ID" function, and the async handler that does the write. See [`05-identity/01-id-generation.md`](05-identity/01-id-generation.md) §4. |
| C3 | 🟠 | **Every `vnoc/structure` call re-reads and re-outlines every file**, and both the sidebar and the canvas call it. `Core.structure` is written but never read. | Build the index once, keep it, update only changed files. [`04-storage/02-index.md`](04-storage/02-index.md). |
| C4 | 🟠 | **A hung driver hangs the core forever.** `request()` has no timeout, and it holds the `io` mutex while it waits, so every later request queues behind it. | Wrap the call in `tokio::time::timeout`; on timeout call `host.forget()` (which kills the driver, thanks to `kill_on_drop`). |
| C5 | 🟠 | **The scan is hard-coded to `.py`** (`scan::python_files`). A second driver would need a core change. | The driver says which extensions it owns in `initialize`. [`06-drivers.md`](06-drivers.md) §3. |
| C6 | 🟡 | The driver host talks to the driver **one request at a time**. Fine for outlines. Too slow for call trees and for "cancel". | Leave it for 0.1. The reader-task pattern for 0.2 is in [`06-drivers.md`](06-drivers.md) §4. |
| C7 | 🟡 | The scan takes paths **from the disk**, but `RepoPath`'s own doc says to take them from git (macOS returns decomposed Unicode). | Normalise to NFC when deriving an ID. [`05-identity/01-id-generation.md`](05-identity/01-id-generation.md) §3. |
| C8 | 🟡 | The workspace root is also a package (`vnoc_playground`, `src/main.rs` = Hello world). | Make the root a pure workspace. [`03-repo-structure.md`](03-repo-structure.md) §2. |

### Driver

| # | Sev | Finding | Suggestion |
|---|---|---|---|
| D1 | 🟠 | **The driver does not use `vnoc-driver-protocol`.** It builds JSON by hand with `json!`. The core's types are `Deserialize`-only. If a field is renamed on one side, nothing fails until runtime. | Derive both `Serialize` and `Deserialize` on the protocol types and use them in the driver. A path dependency across the two workspaces works (the driver already does it for `vnoc-domain` in tests). |
| D2 | 🟡 | One OS thread per request, with no limit. Harmless while the host sends one at a time; a problem the day it sends 1,000. | Add a small limit (a counting semaphore or a thread pool) when the host becomes concurrent. |
| D3 | 🟡 | `capabilities` says `"ids.plan": false`. The S3 spike code for `planIds` is not ported yet. | This is the first driver task of 0.1. |

### Extension

| # | Sev | Finding | Suggestion |
|---|---|---|---|
| E1 | 🟠 | **Only the first workspace folder is used** (`workspaceFolders?.[0]`), in three places. | Fine for 0.1, but put it behind one function `projectRoot()` so there is one place to change. |
| E2 | 🟠 | **The message relay is a chain of `if (m.method === …)` with `any`.** Each new feature adds a branch and an untyped message. | One shared message map. [`07-ui/01-state.md`](07-ui/01-state.md) §3. |
| E3 | 🟡 | The CSP nonce uses `Math.random()`. | `crypto.randomBytes(16).toString("base64")`. A nonce should not be guessable. |
| E4 | 🟡 | `TreeNode` is declared twice by hand (`structureView.ts`, `webview/src/types.ts`) and once in Rust. | `shared/` folder now; generated types later. |
| E5 | 🟡 | `retainContextWhenHidden: true` keeps the whole webview in memory when the tab is hidden. | Keep it for 0.1 (fewer bugs). Revisit when memory is measured. |

### Webview

| # | Sev | Finding | Suggestion |
|---|---|---|---|
| W1 | 🟠 | **One Monaco editor per open code card.** Each instance costs memory and startup time. Ten open cards will be felt. The JS bundle is already 3.9 MB. | For **read-only** code, render Shiki's HTML directly (`codeToHtml`), which you already load. Keep Monaco for the one card being edited, in 0.3. Measure with 30 open cards before deciding. |
| W2 | 🟠 | **`App.tsx` owns five pieces of state and six effects.** Docs, selection and the doc panel will double that. | Split into a *data* slice and a *view* slice. [`07-ui/01-state.md`](07-ui/01-state.md) §4. |
| W3 | 🟡 | Card heights are constants (`ROW_H`, `CODE_H`). A doc badge or a longer name will overlap. | Fine now. Move to measured layout (dagre or elk) when the card grows. |
| W4 | 🟡 | View state (`collapsed`, `focus`) is keyed by location key, so a rename forgets it. | Correct behaviour for 0.1: `prune` already drops what no longer exists. Do not build `key_moves`. |

## 5. What I would stop doing

- **Stop writing design docs ahead of code.** You have enough. From here: build a slice, then write down what is true in ten lines.
- **Stop adding decision rows for things not built.** A decision about the provisional tier costs maintenance today and pays nothing until 0.4.
- **Stop treating all formats as frozen.** Only what gets committed to a user's repo is frozen. The two wire protocols, the index and the UI state live inside
  one VSIX and one machine. Change them whenever it helps.

Next: [`02-mvp-and-releases.md`](02-mvp-and-releases.md)
