# 04.02: The order of work, milestones, and "done"

## The order of work

```
PHASE 2a  spikes (05-spikes/)          throwaway code; each answers some [check] rows (checklist C)
   │      S1 first, S2 second; S3 · S4 · S5 in any order (they don't depend on each other)
   ▼
PHASE 2b  design docs (06- … 10-)      one folder per program; each file says which spike it waits for
   │      written step by step, reviewed together, like phase 1
   ▼
PHASE 3   build, milestone by milestone (below). Each milestone is usable on its own.
   ▼
PHASE 4   later (driver downloads, TS/JS, MCP / CLI…)
```

**Rule for phase 2:** a design doc is written **after** the spike that answers its questions, never before. A spike's findings go into its own file
(the "Findings" section), and anything that changes a decision goes into the decision log as a new row.

**Open review rows** (checklist A2, A5, A6, A7, A8, A10, A14; B5, B6, B8, B11, B12, B15) are taken **at my proposal** so phase 2 can start. If you change one later, the design doc it affects is updated.

## MVP milestones (→ `0.1` pre-release)

| M | Delivers | The user can… | Needs spike | Design docs |
|---|---|---|---|---|
| **M0** | Monorepo skeleton (core crates, `drivers/python`, `extension/`, `webview/`); CI builds a VSIX for one platform; the core answers `ping` | install it, see "V-NOC ready" | S1 | 06-01, 06-02, 10-01 |
| **M1** | Driver host (copy-on-first-run, lookup, handshake), structure index of the scope, SQLite cache, **sidebar tree**, **Quick look**, Go to code, Restricted Mode | right-click ▸ Quick look ▸ browse the tree | S2 | 06-03, 07-01…03, 08-01, 08-02 |
| **M2** | **Canvas**: node card (expand · code view · zoom), **Canvas \| Code** tabs, **focus stack**, Reveal in V-NOC, auto / manual sync | explore visually; focus, Back, breadcrumbs | S1 | 09-01, 09-02 |
| **M3** | **Docs**: guided setup, `vnoc.toml` / `local.toml`, `.gitignore` per mode, `node.toml`, **doc panel** (read, add, remove), shared / private, Share / Make private | document code and share it through git | — | 06-04, 08-02 |
| **M4** | **Docs survive**: step 0 (observed moves, VS-D76) + matching ladder, Missing / Not on this branch (VS-D74) / Archive / Relink / Remove, V-NOC trash + Restore, strong two-ID merge | refactor and switch branches without losing docs | S4, S3 (trash) | 06-05, 06-06, 08-03 |
| **M5** | **Call trees**: lazy, depth setting, expand, pulled-in files, external leaves, markers | open any function's call tree | S2 | 06-07 |
| **M6** | **Editing in the canvas**: code (card, dialog, Code tab) and docs as views of VS Code's document; ⌘S / Ctrl+S; the version race | edit code and docs without leaving the canvas | S5 | 08-03, 09-03 |
| ⭐ | **MVP `0.1` pre-release** on both stores, once the exit criteria in 04-01 hold | | | 10-01, 10-02 |

**Why this order:** each step needs the one before it to be visible. Tree (M1) → canvas (M2) → docs to attach (M3) → docs that survive (M4).
**Step 0 ships in the same milestone as the ladder (M4), never after it** (VS-D76).
Call trees (M5) and canvas editing (M6) are independent of M3–M4 and can be swapped if one is blocked.

## Stable milestones (→ `1.0`)

| M | Delivers | The user can… | Needs spike | Design docs |
|---|---|---|---|---|
| **M7** | **In-code IDs**: driver placement (docstring `ID:`, `# FileID:`), `on-attach` / `all`, `[[path]]` rules, Add / Remove (exact span), Restore / Keep removed, previews, save-time IDs (precomputed) | make docs survive *any* refactor | S3 | 06-06, 07-03, 08-03 |
| **M8** | **Playground** (VS-P12) | try a function on its own | — | 08-02 |
| **M9** | **Polish**: CodeLens, hover, search, effective-config view, package detection, archive expiry + "Empty V-NOC trash", doc freshness (VS-D77), weak-match prompts, `vnoc driver install <file>` | use it every day | — | 08-02, 09-02 |
| **M10** | **Hardening**: 10k-file performance, Linux arm64, Remote SSH / WSL / dev containers, schema-mismatch tests from every 0.x, driver protocol frozen at `1`, user guide | trust it on any machine | — | 10-02 |
| ⭐ | **`1.0` stable**, once the exit criteria in 04-01 hold | | | |

## "Done" for every milestone

A milestone is done when:

1. **Its user story works end to end** in a real VS Code window (the "The user can…" column), on macOS and one of Linux / Windows.
2. **Tests** (10-02): Rust unit tests for the pure logic (table-driven: the ladder, placement rules, the Missing / not-on-this-branch rule), integration tests on the fixture repos,
   and at least one extension test (`@vscode/test-electron`) for the user story.
3. **Nothing in `plan_vs/` is contradicted by the code.** If the code had to differ, the decision log got a new row first.
4. CI is green on all MVP platforms.

Next: [`../05-spikes/00-how-spikes-work.md`](../05-spikes/00-how-spikes-work.md)
