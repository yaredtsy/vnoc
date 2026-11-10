# 03. Progress overview: phases, milestones, timeline

Legend: ✅ done · 🟡 partly done · ❌ not started

## The four phases

```
PHASE 1  requirements + decisions          ✅  plan_vs/00- … 03-        (2026-11-05, 8 review rounds)
   ▼
PHASE 2a spikes = throwaway experiments    🟡  plan_vs/05-spikes/, code in spikes/
PHASE 2b design docs, one per program      🟡  plan_vs/06- … 10-       (4 written, 17 outlines)
   ▼
PHASE 3  build the product                 🟡  crates/, drivers/, extension/, webview/   ◄── we are here
         MVP milestones M0–M6 → 0.1 pre-release → post-MVP → 1.0
   ▼
PHASE 4  later: driver downloads, TS/JS, MCP / CLI, history…
```

Phases 2 and 3 overlap on purpose: the walking skeleton was built while the last design docs were still outlines, and the design docs that exist (06-01, 06-02, 07-01, 10-01)
were written **together with** the code ("only what exists is specified").

## Phase 1: done

| What | Where | Status |
|---|---|---|
| Vision, user journey, functional + non-functional requirements, config | `plan_vs/00-requirements/` | ✅ (parts now stale after the MVP cut, see [09](09-plan-docs-guide.md)) |
| Decision log: **VS-D1…D77** decided, VS-P1…P35 proposed, VS-O1…O10 open | `plan_vs/01-decisions/01-decision-log.md` | ✅ up to D77. **D78–D80 are drafts, not in the log** |
| Architecture, sidecar, webview, storage, identity, distribution, repo | `plan_vs/02-technology/` | ✅ (identity + storage docs need the D79 sweep) |
| Review checklist (A product questions, B proposals, C facts to check) | `plan_vs/03-next/01-review-checklist.md` | ✅ A + B answered; C mostly answered by spikes |

## Phase 2a: the spikes

| Spike | Teaches | Status | Leftovers |
|---|---|---|---|
| S1 hello core | extension + Rust sidecar + webview + trust | ✅ | Restricted Mode + file watcher combination not checked |
| S2 Python driver | driver process, pylspt adapter, timings | ✅ core loop | C11 (macOS quarantine / Windows Defender), C12 (lock crate), copy-on-first-run |
| S3 edits and saving | `WorkspaceEdit`, save hook, V-NOC trash | ✅ 26/26 tests | save reasons `AfterDelay`/`FocusOut`; Windows/Linux/remote |
| S4 git and matching | gix, branch rules, the matching ladder | ✅ | step 0 checks (post-MVP); findings §9–10 still only in the draft file |
| S5 canvas editors | Monaco in a webview, ⌘S, doc panel | ❌ **not run** | all of it (C15, C21, C23). Needed before M6 |

Details: [`04-spikes-results.md`](04-spikes-results.md).

## Phase 3: MVP milestones

This table uses the **new MVP cut (draft VS-D78)**. `plan_vs/04-roadmap/02-milestones.md` still shows the old text for M3 and M4.

| M | Delivers | Status | What exists |
|---|---|---|---|
| **M0** | monorepo skeleton, core answers `ping`, **CI builds one VSIX** | 🟡 ~80% | ✅ monorepo, ✅ `vnoc/ping`, ✅ F5 dev loop, ✅ tests in 4 programs. ❌ **CI / VSIX** |
| **M1** | driver host (copy-on-first-run), structure index, **SQLite cache**, **sidebar tree**, **Quick look**, Go to code, Restricted Mode | 🟡 ~20% | ✅ core starts the driver + handshake, ✅ structure of the opened folder (`vnoc/structure`, in memory), ✅ trust-safe binary paths. ❌ SQLite, sidebar tree, Quick look, Go to code, real scope rule, copy-on-first-run |
| **M2** | **canvas** with node card, Canvas \| Code tabs, **focus stack**, Reveal in V-NOC, auto/manual sync | 🟡 ~10% | ✅ a canvas tab that draws the structure, click-to-expand, state restored after reopen. ❌ node card, tabs, focus stack, reveal, sync |
| **M3** | guided setup (scope only), minimal `vnoc.toml`, `node.toml`, doc folders, **doc panel**, **write the ID into code on attach** | 🟡 ~10% | ✅ `NodeId` format + `node.toml` reader/writer in `vnoc-domain` (final format, tested). ❌ everything the user sees |
| **M4** | Missing / Not on this branch, relink by hand, archive, V-NOC trash + Empty / Restore commands, restore lost ID (step 1), rename-event anchor updates | ❌ | logic proven in spike S4 (`spikes/s4-git/spike-match`), nothing in product code |
| **M5** | lazy **call trees** | ❌ | spike S2 proved `callTree` / `expand` in the spike driver only |
| **M6** | **editing in the canvas** (Monaco as a view of VS Code's document), ⌘S | ❌ | blocked on spike S5 |
| ⭐ | **MVP 0.1 pre-release** | ❌ | |

After the MVP: the **P-list** (P1 no-ID mode + ladder steps 2–6 + step 0; P2 private docs / solo mode; P3 settings UI, `all` mode, save-time IDs; P4 archive expiry,
doc freshness, `# FileID:`, Remove ID, the "Recently removed" panel), then M8 playground, M9 polish, M10 hardening → **1.0**.

## Timeline (git history)

| Date | Commit | What happened |
|---|---|---|
| 11-04 | `37f83f5` fist commit | the old `plan/` (Rust server backend) |
| 11-05 | `f734fb6` plan for vs code | **pivot to a VS Code extension**, `plan_vs/` created |
| 11-05 | `09defd7` phase 1 · `5111043` phase 2 plan | phase 1 closed, roadmap + spikes planned |
| 11-05 | `c4fd1d5` spike version · `8933b09` watcher fix | spike S1 |
| 11-05 | `8b5cb27` s2 done · `6da1f05` c3 | spike S2, and the async-lsp vs lsp-server comparison (C3) |
| 11-06 | `63e6849` git based storage · `fe71e20` · `ee18562` | spike S3, storage rules (VS-D70–D74) |
| 11-06 | `843f21d` provisional tier | spike S4: the ladder, tuning, the provisional tier (VS-D75) |
| 11-06 | `7d329a1` storage final plan · `a596d0b` trash storage | VS-D76/D77, the final MVP file formats, the trash |
| 11-07 | `ad27a93` checkpoint | **product code starts**: `vnoc-domain` (`NodeId`, `node.toml`), driver protocol, Python driver |
| 11-07 | `461cb33` basic view | **walking skeleton**: core + extension + webview, end to end |

Next: [`04-spikes-results.md`](04-spikes-results.md)
