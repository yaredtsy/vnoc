# fable_plan: review of V-NOC and the fastest road to an MVP

Written 2026-11-10, after reading `plan_vs/`, `report/`, and all the product code (`crates/`, `drivers/python/`, `extension/`, `webview/`).

This folder does **not** replace `plan_vs/`. It is a second opinion: what I think of the current state, what I would cut to ship sooner, and how to
grow from that first release to a stable 1.0 without breaking anything users have committed. Where it disagrees with `plan_vs/`, the difference is listed in
[`08-decisions-to-log.md`](08-decisions-to-log.md) as draft rows for your decision log. I did not edit your log.

## The short version

1. **Your foundations are good.** The domain crate, the driver protocol, the sidecar and the file formats are careful work. Nothing needs to be thrown away.
2. **The plan is much bigger than the product.** About 5,000 lines of plan against about 2,800 lines of code, with three sources of truth that disagree
   (`report/12-contradictions.md`). The risk is no longer "wrong design". The risk is never shipping.
3. **Ship one loop first:** *browse the code → attach a doc → the doc is still there after a rename*. That is release 0.1. Call trees are 0.2. Editing code in
   the canvas is 0.3.
4. **Only one thing has to be right on day one: the files users commit** (`.vnoc/`, the ID text, the markers in code). Everything else runs on one machine
   inside one VSIX and can change in any release.
5. **Do the packaging first.** A VSIX that installs on your own machine will show more real problems than any more design. I found four that block it today
   ([`01-review.md`](01-review.md) §3).

## What you decided today (2026-11-10)

| # | Question | Your answer |
|---|---|---|
| F1 | Where does a function/class ID live? | **Docstring `ID:` line**, as now. A "comment above the def" style comes later as an option. |
| F2 | Folder tracking | A hidden **`.vnoc-id`** file inside the folder, created **only when you attach** something to that folder. |
| F3 | Storage for the MVP | **In memory first.** SQLite comes later behind the same interface. |
| F4 | What is in 0.1 | **The docs loop only.** Call trees → 0.2, canvas editing → 0.3. |

## The product, top-down

Read this tree from the root. Each branch has a file (or a folder) in this plan.

```
V-NOC
├── WHY      browse code as a graph, attach docs that survive refactors
│
├── WHAT SHIPS WHEN ─────────────────────────────── 02-mvp-and-releases.md
│   ├── 0.1  docs loop        tree · canvas · attach · find again · Missing
│   ├── 0.2  call trees       (+ SQLite if startup is slow)
│   ├── 0.3  edit in canvas   (after spike S5)
│   ├── 0.4+ no-ID matching · private docs · settings UI · playground
│   └── 1.0  hardened, protocol frozen, all platforms
│
├── HOW IT IS BUILT
│   ├── the repo ────────────────────────────────── 03-repo-structure.md
│   │   ├── crates/      Rust core (domain · protocol · the core program)
│   │   ├── drivers/     one program per language
│   │   ├── extension/   the VS Code side (TypeScript)
│   │   ├── webview/     the canvas (React)
│   │   └── shared/      message types both TypeScript sides use
│   │
│   ├── storage ─────────────────────────────────── 04-storage/
│   │   ├── A. the user's code        truth, plus small ID markers
│   │   ├── B. .vnoc/ files           what a person wrote (committed)
│   │   ├── C. the index              recomputable (memory now, SQLite later)
│   │   └── D. view state             per machine (VS Code keeps it)
│   │
│   ├── identity ────────────────────────────────── 05-identity/
│   │   ├── the ID                    26 characters, made once, never changes
│   │   ├── markers (all optional)
│   │   │   ├── function / class      ID: line in the docstring
│   │   │   ├── file                  # FileID: comment at the top
│   │   │   └── folder                .vnoc-id hidden file
│   │   └── finding a node again      marker → anchor → Missing
│   │
│   ├── language drivers ────────────────────────── 06-drivers.md
│   │   ├── the wire                  JSON-RPC over stdin / stdout
│   │   ├── what a driver must do     outline · plan IDs · (call trees)
│   │   └── adding a language         checklist + conformance test
│   │
│   └── UI ──────────────────────────────────────── 07-ui/
│       ├── state                     who owns what, how it flows
│       └── design                    sidebar · canvas · node card · doc panel
│
├── WHAT I FOUND ────────────────────────────────── 01-review.md
│
└── LATER (ideas, not scheduled) ────────────────── futureplan/
    ├── graph diff        changes as functions and as a graph; fixing them per function
    └── agent access      CLI + skill · MCP · native plugins for coding agents
```

## Reading order

| If you want… | Read |
|---|---|
| my honest opinion of where things stand | [`01-review.md`](01-review.md) |
| the work list for the first release | [`02-mvp-and-releases.md`](02-mvp-and-releases.md) |
| the storage design (you asked for extra care here) | [`04-storage/`](04-storage/README.md), all five files |
| how IDs, files and folders are tracked | [`05-identity/`](05-identity/README.md) |
| what to paste into your decision log | [`08-decisions-to-log.md`](08-decisions-to-log.md) |

## Conventions in these files

- Every file starts with a small tree of what is inside it, then goes branch by branch.
- **🦀 Rust note** and **🧩 VS Code note** boxes explain a concept the first time it matters. `report/02-vscode-extension-basics.md` and
  `report/11-glossary.md` already cover the basics, so I link to them instead of repeating.
- **[check]** means I believe it but did not run it. Verify before you depend on it.
- "0.1", "0.2"… are release numbers from [`02-mvp-and-releases.md`](02-mvp-and-releases.md). They replace M0–M6 in this plan.
