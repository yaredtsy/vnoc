# V-NOC for VS Code: Plan 2 (`plan_vs/`)

> **What this is:** the plan for shipping V-NOC as a **VS Code extension** (also Cursor, Windsurf, VSCodium). The Rust core runs next to the
> editor, keeps an embedded SQLite cache inside the project's `.vnoc/`, and talks to **language drivers** (separate executables; Python first).
>
> **How it relates to [`../plan/`](../plan/README.md):** that plan was for a server backend (TerminusDB + gRPC + a web app). This plan
> **diverges** where the product changed (storage, API, packaging, identity, drivers) and **points back** to `plan/` where the Rust core
> stays the same (scan, diff, reconcile, call-tree layers). See [`01-decisions/02-relation-to-old-plan.md`](01-decisions/02-relation-to-old-plan.md).
>
> **Phase:** 🟢 **Phase 2: spikes, then design.** Phase 1 (8 review rounds) is in the [decision log](01-decisions/01-decision-log.md). Start at
> [`04-roadmap/01-mvp-vs-stable.md`](04-roadmap/01-mvp-vs-stable.md): what ships in the MVP (0.x) vs stable (1.0).
> Phase 2 starts once [`03-next/01-review-checklist.md`](03-next/01-review-checklist.md) is answered.
>
> **Style:** written to teach. New VS Code ideas get a **🧩 VS Code note**, Rust ideas a **🦀 Rust note**.

---

## The whole product as one tree: what the user does

Each letter has its own section in [`00-requirements/02-user-journey.md`](00-requirements/02-user-journey.md).

```
User installs "V-NOC" (VS Code Marketplace, or Open VSX for Cursor / Windsurf / VSCodium)
│   ships with: the core + the Python driver (copied on first run into the global drivers folder)
│
├── A. OPENS A FOLDER
│   ├── A1. .vnoc/vnoc.toml exists ──► index on open (if configured) ──► C
│   │     ├── untrusted workspace ──► read-only: index + browse; no code edits, no playground runs
│   │     └── HEAD moved while VS Code was closed ──► the same rule as E4/E5 (VS-D74)
│   ├── A2. no .vnoc/ ──► stay quiet; the V-NOC view offers "Set up" / "Quick look"
│   └── A3. a needed language driver is missing ──► "Install the X driver?" (always the user's choice)
│
├── B. SETS UP A PROJECT  (.vnoc/ always at the git root; what you click = the SCOPE)
│   ├── B1. Quick look        right-click ▸ "V-NOC: Quick look"
│   │     │                   no questions; cache in VS Code's own storage; NOTHING in the repo
│   │     └── "Make it a project" ──► creates .vnoc/ ──► B2
│   ├── B2. Guided setup      right-click ▸ "V-NOC: Set up here"  — asks only what YOU know:
│   │     ├── Q1 What to index?                        (the scope)
│   │     ├── Q2 Who sees the notes?    ○ My team (commit .vnoc/)   ○ Just me (local side only, out of git)
│   │     └── Q3 ID comments in your code?
│   │            ○ On attach (team default)  ○ All  ○ None (solo default)   + per-folder rules later
│   └── B3. Hand-written      .vnoc/vnoc.toml (team) and/or .vnoc/local.toml (just me) ──► "Reload"
│
├── C. EXPLORES
│   ├── C1. Sidebar tree      Root ▸ folders ▸ files ▸ classes ▸ functions  (pulled-in files dimmed)
│   ├── C2. Canvas            open any node as a graph; double-click = focus
│   │     ├── node card       [▸] expand · [</>] code inside the card · [⤢] zoom dialog · 🔗 ID · 📄 docs
│   │     ├── tabs            Canvas | Code   (code editable everywhere; ⌘S/Ctrl+S saves the focused one)
│   │     ├── doc panel       [📄 Docs] toggle: the selected node's docs · read · edit · [+ Add] · [🗑 Remove] · "code changed since…" (VS-D77)
│   │     └── ★ Focus stack   focus · Back · Clear · click the breadcrumb to jump
│   ├── C3. Call tree         lazy only: open ▸ N levels ▸ expand ▸ N more   (N = a setting, default 3)
│   │     ├── calls leaving the scope are still followed; those repo files get pulled in, never ID'd
│   │     └── third-party calls (site-packages, .venv, node_modules, outside repo) = external leaves (open read-only)
│   ├── C4. Jump to code      the editor opens at the right lines (code is never copied)
│   └── C5. Reveal in V-NOC   from the editor cursor
│
├── D. ADDS KNOWLEDGE
│   ├── D1. Attach a doc      ○ Shared (team sees it)  ○ Private 🔒 (only me)  → a .md file, normal editor tab
│   │     └── later: "Share this doc" / "Make private"
│   ├── D2. Playground        a snippet bound to a node, run in the VS Code terminal (trusted only)
│   └── D3. Per-node ID       right-click any file / class / function (in scope)
│         ├── [Add permanent ID]     ──► placed by the driver (Python: `ID:` in the docstring; files: `# FileID:` comment on top); undoable
│         ├── [Remove permanent ID]  ──► the recorded span, only if its text still matches; docs stay; id_in_code = never
│         └── folder ▸ IDs for this folder and below ▸ All / On attach / None / Inherit
│
├── E. KEEPS CODING (background)
│   ├── E1. auto sync:   edit · save · rename · move ──► small re-index
│   │   manual sync: "V-NOC: 12 changed · Sync"  (call trees stay correct anyway)
│   ├── E2. `all` rule: new functions get their ID inside YOUR save (one save, one undo)
│   │        (manual + focus-out saves; not auto-save mid-typing; only functions without syntax errors)
│   ├── E3. documented node found again (edits watched in the editor, VS-D76; else ID comment / path / hash / git rename / body hash) ──► silent
│   │        └── ID comments deleted by hand ──► one notice: Restore / Keep removed
│   ├── E4. branch switch: gone nodes whose docs live on another branch ──► "not on this branch", NO prompt (VS-D74)
│   └── E5. gone, and its docs are orphaned in this commit (edit, pull, rebase…) ──► "Missing" + one grouped notification (VS-D74)
│         ├── [Archive]  ──► soft-deleted in place, waits ──► [Relink to…] ──► active again (or expires after N days)
│         ├── [Find it…] ──► pick the node it became (top candidate preselected, VS-D75)
│         └── [Remove]   ──► move it and its docs to .vnoc/local/trash/ (confirmed; restorable from "Recently removed"; V-NOC never empties it)
│
├── F. CHANGES SETTINGS ANY TIME  ("for the team or just me?" → vnoc.toml or local.toml)
│   ├── F1. IDs → none       IDs already in code stay; no new ones
│   ├── F2. IDs → all / on attach   preview first, then inject (per-node choices always win)
│   ├── F3. scope · call-tree depth · auto/manual sync
│   ├── F4. Rebuild index    delete the cache, re-index; docs untouched
│   └── F5. team ↔ solo      only changes where NEW docs go; existing ones move only when you Share / Make private
│
└── G. SHARES WITH GIT
    ├── team: committed = vnoc.toml + nodes/ ;  ignored = local.toml + local/ (private docs, cache)
    ├── solo: only the local side exists (local.toml + local/); .vnoc/.gitignore is "*" → nothing from .vnoc/ in git status
    ├── .vnoc/.gitignore is written at setup (team: local/ + local.toml) and only a mode switch changes it; root .gitignore never touched
    ├── share / make private = MOVE the file or node folder between local/nodes/ and nodes/
    ├── one node can have shared docs AND private notes (same ID, a folder on each side)
    ├── two people document one function at once ──► same derived ID ──► same folder + same ID line ──► clean git merge
    └── a teammate clones ──► cache rebuilt ──► shared docs re-attach (ID comment or node.toml)
```

Not in v1, on purpose: **history/versioning, agent, tasks, logs** (and groups, pending review). What ships when: [`04-roadmap/01-mvp-vs-stable.md`](04-roadmap/01-mvp-vs-stable.md).

---

## Reading order

```
plan_vs/
│
├── README.md ........................................ you are here (the user tree above)
│
│   ═══ PHASE 1: what, why, with what (done) ═══
├── 00-requirements/ ................................. WHAT the user gets
│   ├── 01-vision-and-scope.md ....................... v1 in / out, personas, success criteria
│   ├── 02-user-journey.md ........................... the tree above, branch by branch
│   ├── 03-functional-requirements.md ................ FR-xx list with priorities
│   ├── 04-non-functional-requirements.md ............ speed, safety, git, platforms
│   └── 05-user-controls-and-config.md ............... who controls what; vnoc.toml + local.toml drafts
├── 01-decisions/ .................................... WHY
│   ├── 01-decision-log.md ........................... ✅ VS-D1…D66 · 🟡 VS-P1…P35 · 🔓 VS-O1…O10
│   └── 02-relation-to-old-plan.md ................... plan/ file by file: reuse · change · drop
├── 02-technology/ ................................... WITH WHAT (decision level)
│   ├── 01-architecture-at-a-glance.md ............... the four programs, the arrows, the main flows
│   ├── 02-running-rust-sidecar.md ................... how VS Code starts and talks to the Rust core
│   ├── 03-frontend-webview.md ....................... React + xyflow, node card, doc panel, editors, focus stack
│   ├── 04-storage-and-git.md ........................ where data lives, .vnoc/ layout, node.toml, moves, cache, collisions
│   ├── 05-identity-and-archive.md ................... IDs, modes + [[path]] rules, ladder, states, archive, save-time edits
│   ├── 06-distribution-and-integrations.md .......... VSIX per platform, stores, Cursor/opencode/pi/Claude Code later
│   └── 07-repo-and-drivers.md ....................... the monorepo; drivers as separate, signed executables
├── 03-next/ ......................................... the phase-1 gate
│   ├── 01-review-checklist.md ....................... what was asked and answered; facts for the spikes (C)
│   └── 02-phase-plan.md ............................. all phases on one screen
│
│   ═══ PHASE 2: in what order, test the risks, design each program ═══
├── 04-roadmap/ ...................................... ★ START HERE for phase 2
│   ├── 01-mvp-vs-stable.md .......................... what ships in 0.x vs 1.0; the frozen data format; exit criteria
│   └── 02-milestones.md ............................. order of work, M0–M10, "done"
├── 05-spikes/ ....................................... 2a: throwaway experiments (code in spikes/)
│   ├── 00-how-spikes-work.md ........................ rules, order, template
│   ├── 01-hello-core.md ............................. S1 extension + sidecar + webview + trust
│   ├── 02-python-driver.md .......................... S2 driver process, pylspt adapter, timings
│   ├── 03-edits-and-saving.md ....................... S3 WorkspaceEdit, save hook, V-NOC trash
│   ├── 04-git-and-matching.md ....................... S4 git lookups, branch cases, the refactor suite
│   └── 05-canvas-editors.md ......................... S5 Monaco as document views, ⌘S, doc panel
│                                                      2b: design docs, ONE FOLDER PER PROGRAM (the boxes in 02-01)
├── 06-core/ ......................................... the Rust core
│   ├── 01-crates-and-domain.md   02-editor-protocol.md   03-cache-schema.md   04-userdata-writer.md
│   └── 05-sync-and-matching.md   06-identity.md          07-call-tree.md
├── 07-drivers/ ...................................... language drivers
│   └── 01-driver-protocol.md     02-driver-host.md       03-python-driver.md
├── 08-extension/ .................................... the VS Code extension host (TypeScript)
│   └── 01-package-and-activation.md   02-commands-and-views.md   03-documents-and-saving.md
├── 09-webview/ ...................................... the canvas (React)
│   └── 01-app-and-state.md   02-canvas-and-node-card.md   03-editors-and-doc-panel.md
└── 10-release/ ...................................... shipping it
    └── 01-ci-and-packaging.md   02-testing.md
```

Design docs start as **outlines** (📝): each says what it will define, what it builds on, its MVP / stable split, and which spike it waits for.

Rule for this folder: **at most one sub-folder deep.** One topic per file.

---

## Start here

→ Phase 1 from the beginning: [`00-requirements/01-vision-and-scope.md`](00-requirements/01-vision-and-scope.md)
→ Phase 2: [`04-roadmap/01-mvp-vs-stable.md`](04-roadmap/01-mvp-vs-stable.md)
