# 04. Spike results

A **spike** is a small throwaway program that answers a question you can't answer by thinking ("does VS Code really do X?", "how fast is Y?").
Spike code lives in `spikes/` and is **never copied into the product**. Results are written in each spike's *Findings* section in `plan_vs/05-spikes/`.

```
S1 ──► S2        S3, S4, S5 only need S1's skeleton
  ├──► S3
  ├──► S4
  └──► S5  ❌ not run yet
```

---

## S1 hello core ✅ (`spikes/s1-hello-core/`, doc `plan_vs/05-spikes/01-hello-core.md`)

**Question:** can a VS Code command start a Rust binary, talk to it over stdio, and draw the answer in a webview?  **Answer: yes.**

| Learned | Why it matters |
|---|---|
| Core spawn → ping → shutdown: **~3 ms**; core binary **0.87 MB** | the sidecar is cheap |
| Webview bundle (React + xyflow) 335 KB (109 KB gzip) | small enough |
| A restorable panel needs `activationEvents: ["onWebviewPanel:<viewType>"]` | without it the tab hangs on reopen |
| Register the panel serializer **before any `await`** in `activate()` | same reason; now in `extension.ts` |
| A file watcher on the parent folder sees `.vnoc/` above the opened folder (C18) | `.vnoc/` lives at the git root even if you open a subfolder |
| `#:schema ./schema.json` in TOML gives completions in Even Better TOML and **Tombi** (C6) | recommend Tombi |
| **C3: `async-lsp` (tokio) for the core** (VS-D67), `lsp-server` (threads) for drivers (VS-D68) | 5 gotchas recorded; all handled in `crates/vnoc/src/server.rs` |

**Left:** whether watcher events arrive in Restricted Mode. The "still to check by hand" line at the end of the findings is stale (most of it was checked).

## S2 Python driver ✅ (`spikes/s2-python-driver/`, doc `02-python-driver.md`)

**Question:** can pylspt sit behind a driver process with a clean protocol, and how fast is it?

| Learned | Number / result |
|---|---|
| Handshake, outline, callTree, expand, cancel, unknown method, shutdown | all work (tested on `httpx`) |
| stdout hygiene (C2) | clean; a test fails the run on any byte outside a frame |
| **C1: `render_depth` does NOT bound the solving** | depth 3 costs the same as unbounded (~80–100 ms on httpx); it only shrinks the output. Lazy expansion needs pylspt changes (deferred, VS-D69) |
| Per-function syntax errors (C17) | yes; one edge case flags a neighbour too (safe side) |
| Release driver size (C4) | **16.2 MB**, 8.4 MB zipped; a clean release build takes **~7 min** |
| Which `pyproject.toml` sections / lockfiles mean "environment changed" (C22) | listed |
| Gotchas | must use pylspt's `Cargo.lock` (salsa 0.28.2); pin Rust 1.98.0; pylspt needs a facade; filter ty/salsa logs (43 MB otherwise); no cancel flag inside pylspt; macOS binaries must be signed |

**Left:** C11 (macOS quarantine / Windows Defender on downloaded drivers), C12 (file-lock crate), copy-on-first-run.

## S3 edits and saving ✅ (`spikes/s3-edits/`, doc `03-edits-and-saving.md`)

**Question:** can V-NOC put ID lines into code safely (dirty buffers, undo, save hooks), and move docs to its own trash?  **26/26 tests pass.**

| Learned | Result |
|---|---|
| Edit a dirty buffer through `WorkspaceEdit` | user text kept, **one ⌘Z undoes it**, file on disk untouched |
| Typing while we plan | the plan carries `document.version`; stale plan → retry once → give up cleanly |
| `# FileID:` writer and exact-text remove | works; the core must **store the exact inserted text**, it can't be re-derived |
| **The save hook budget (C8)** | VS Code cuts a hook at **~1.5 s** and **drops it for the session after 4 slow saves**. Rule: never wait on a driver in the hook; precompute |
| Timings | planning IDs for a 4,200-line file: 10.6 ms p50; hook with precomputed edits: 0.2 ms |
| V-NOC trash (VS-D70) | `.vnoc/local/trash/<time>_<id>/` + `trashed.toml`; open dirty files follow the move; Restore never overwrites |
| Who writes what (B8) | user files and moves → `WorkspaceEdit`; cache, `node.toml`, `trashed.toml`, `.gitignore` → the core directly |

**Left:** save reasons `AfterDelay` / `FocusOut`; Windows, Linux, remote. Spike shortcut: the extension started the driver itself; in the product **only the core starts drivers**.

## S4 git and matching ✅ (`spikes/s4-git/`, doc `04-git-and-matching.md`)

**Question:** after a refactor or a branch switch, can V-NOC find documented nodes again without ever attaching a doc to the wrong function?

| Learned | Result |
|---|---|
| The **matching ladder** on 6 refactors (+2 extra) | **no wrong silent match**. Renames → body hash; moves → file hash; move+edit → git rename detection; rename+rewrite → asks |
| Tuning on ~9,600 labelled cases from 4 real repos | zero wrong silent matches only at **Edit score ≥ 0.99**. In practice: only an edit-free rename/move is matched silently, **anything edited is asked** (and the right answer is usually first) |
| Twin functions (identical bodies) | caused silent mistakes → twins now **always ask** (VS-D73 amendment) |
| Provisional tier ("unconfirmed link") | built, **off** in v1; no threshold reached 95% accuracy (VS-D75) |
| Missing vs "not on this branch" (VS-D74) | decided by whether `node.toml` is committed in HEAD's tree, else `last_seen(id, branch)`. Tested with real git |
| **gix** for the three git lookups | agrees with the git binary everywhere, **7–17× faster** (~0.2–0.5 ms). No fallback |
| C9: `vscode.git` for HEAD changes | **unreliable** → the core watches `.git` itself. Rebase/merge/cherry-pick pause is built |

**Why S4 led to the MVP cut (VS-D78):** without IDs in code, "renamed **and** edited" always becomes a question. With an `ID:` line in the docstring, the ID travels with the function,
so the MVP only needs ladder **step 1** + VS Code's rename events. Steps 2–6 and step 0 move to P1.

**Left:** step 0 checks (reload test was contaminated, formatter measurement invalid), both post-MVP. Findings §9–§10 and rows D78–D80 are still in `spikes/s4-git/DRAFT-doc-rows.md`, not pasted into the plan.

## S5 canvas editors ❌ (doc `05-canvas-editors.md`)

**Question:** can Monaco (VS Code's editor component) run in our webview as a live view of a VS Code document, with ⌘S, without corrupting the file when you type in both places?
**Not run.** It must run before **M6** (canvas editing) and before the doc panel's edit mode. It answers C15 (open library files read-only), C21 (Monaco CSP, workers,
bundle size, the edit race), C23 (the ⌘S keybinding in a webview).

Next: [`05-code-rust-core.md`](05-code-rust-core.md)
