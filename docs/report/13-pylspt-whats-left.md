# 13. pylspt: what's built and what's left

**pylspt** (`~/Documents/Programing/rust/pylspt`, remote `yaredtsy/rust_python_parser`) is the Rust Python analyzer that the V-NOC Python driver wraps. It replaces the old
Jedi-based Python driver with **ruff** (parser) and **ty** (type checker), vendored at `vendor/ruff` (`ac201b8`, Rust 1.98.0).
It has its own plan (`pylspt/plan/`), its own milestones, and knows nothing about V-NOC (VS-D65).

This page has two lists: **what V-NOC needs from pylspt** (by V-NOC milestone), and **pylspt's own roadmap** (its call-tree build steps).

---

## Part A: what V-NOC needs from pylspt

| # | Feature | pylspt today | V-NOC needs | For | Status |
|---|---|---|---|---|---|
| A1 | **Def scanner** (defs, qualnames with `<locals>`, ranges) | ✅ `defs::scan_defs`, `definitions` | used by `vnoc/outline` | M1 | ✅ done |
| A2 | **Per-def syntax-error flags** | ❌ none; the driver computes them from ruff's error list (S2/C17) | fine in the adapter | M1 | ✅ adapter does it |
| A3 | **A facade crate** (re-export the ruff types the adapter needs) | ❌ re-exports nothing; the driver depends on `ruff_python_parser`, `ruff_python_ast`, `ruff_text_size` by path | one `pylspt::…` import surface | M1 | ❌ **left** (TODO in `drivers/python/Cargo.toml`) |
| A4 | **Pinned by git `rev`** | path dependency only; uncommitted changes in the repo (`run-jarvis.sh`, `cli-wire.json`) | push, pin a commit, `[patch]` for dev | M0/M1 (CI) | ❌ **left** |
| A5 | **ID placement** (`plan_ids`, `doc_slot`, `apply`) | ✅ `src/edit.rs`: finds the docstring slot, handles one-liners, `r"""`, no docstring. IDs come from a closure, default **UUID v4** (`uuid` crate) | the **core's** `NodeId`, keyed by `(qualname, occurrence)` (VS-D80) | **M3** | 🟡 works with a workaround: S3 called `plan_ids` once per def with `|| id.clone()`, because the closure doesn't know which def it's for |
| A6 | **Replace an old UUID `ID:` line in place** (`replace_existing`) | ❌ | VS-D79 guard: replace only if the value parses as a UUID; any other `ID:` text is the user's | M3 | ❌ **left** (adapter or pylspt) |
| A7 | **Exact inserted text + span** of each ID edit | 🟡 `Edit` has offset + text; spans were computed in the S3 adapter | the core stores the exact text (S3: it can't be re-derived) | M3 (store), P4 (Remove ID) | 🟡 adapter can do it; open question in `07-drivers/03` |
| A8 | **Reading `ID:` lines** | ✅ via the def scan + docstring | used by the driver's outline (`id` field) | M1/M3 | ✅ done (driver test passes) |
| A9 | **Call trees** (`call_tree`, `call_graph`, `call_wire`) | ✅ `src/interp/` (~4,300 lines): walker, calls, solver, recursion groups, closures, cuts, render, stable IDs | `vnoc/callTree` + `vnoc/expand` | **M5** | 🟡 works (S2 timed it on httpx: ~80–100 ms). Not wired into the product driver yet |
| A10 | **Bounded solving / lazy expansion** | ❌ `render_depth` only trims the **output**; the solver solves the whole tree (S2/C1: depth 3 ≈ unbounded) | expand a node later without re-solving everything; resume from a frontier | M5+ | ❌ **deferred (VS-D69)**, needs pylspt changes. Fine for now on httpx-size repos |
| A11 | **External cancel** | ❌ only `Limits::deadline` (10 s) | stop a running solve when the core cancels (`$/cancelRequest`) | M5 | ❌ **deferred with A10**; today the answer is just dropped |
| A12 | **Cross-request cache** (keep summaries while ty's revision is unchanged) | ❌ per-request only (pylspt "v1.1" in its growth path) | warm repeat requests (S2: 6–28 ms repeat vs ~130 ms first) | M5 / M10 | ❌ pylspt-side, later |
| A13 | **Third-party calls as external leaves** | ✅ `Target::External` for definitions outside the project | V-NOC defines "external" by **location** (outside the repo, `site-packages`, `.venv`, `node_modules`) | M5 | 🟡 to check: pylspt's notion of "external" vs V-NOC's location rule |
| A14 | **Unsaved-buffer text** | the outline is parsed from text by the adapter; call trees read files through ty's database | call trees over a dirty buffer (the user's unsaved edits) | M5 / M6 | ❌ to check: overriding a file's text in the salsa database |
| A15 | **Environment inputs** (`pyproject.toml` sections, lockfiles) | ❌ (pylspt uses `ProjectMetadata::discover`, falls back to an ad-hoc project) | a list of files whose change means "clear resolution caches" (VS-D53, S2/C22) | M5 | ❌ **left** (adapter can do it from the C22 list) |
| A16 | **Per-def fingerprints** (`vnoc/defs`: body hash, tokens) | ❌ (built in the S4 spike driver) | ladder steps 2–5 | **P1** (post-MVP) | ❌ not needed for the MVP |
| A17 | **`# FileID:` writer** | ❌ (prototyped in the S3 adapter, pylspt untouched) | file IDs at the top of a file | **P4** | ❌ not needed for the MVP |
| A18 | **No stdout writes** | ✅ (S2/C2: clean; the driver denies `print_stdout`) | stdout is the protocol | always | ✅ keep the CI guard |
| A19 | **Quiet logging** | ty/salsa trace every query (43 MB per run) unless filtered | the driver filters (`VNOC_LOG`) | always | ✅ done in the driver |

### What this means for the MVP

- **M1 needs:** A3 (facade) and A4 (push + pin). Both are small and only change how the driver builds.
- **M3 needs:** A5–A7: ID placement with the core's IDs, the UUID guard, exact spans. They can live in the adapter; moving them into pylspt is optional.
- **M5 needs:** A9 wired into the driver, A13 and A15 checked. A10–A12 can wait as long as repos stay small (VS-D69), but they are the main **performance risk** for big repos (the 1.0 target is 10,000 files).
- **Not needed until after the MVP:** A16 (P1), A17 (P4).

---

## Part B: pylspt's own roadmap (`pylspt/plan/04-build/02-milestones.md`)

pylspt's original goal was to be a drop-in, 10–100× faster replacement for the Jedi driver of the old web V-NOC. Since the VS Code pivot, V-NOC uses it as a **library** through
`drivers/python`, so pylspt's **M7 (its own JSON-RPC-over-HTTP transport)** is no longer needed by V-NOC.

| pylspt M | What | Status (from the code and git history; pylspt has no status file) |
|---|---|---|
| M0 | golden files from the Jedi driver | ❓ not found in the repo |
| M1 | build wiring, Python-version report | ✅ builds; `pylspt-dev` CLI exists |
| M2 | syntax layer (defs, positions) | ✅ `defs.rs`, `positions.rs`, `scan.rs` |
| M3 | ID injection | ✅ `edit.rs` (UUID IDs, see A5–A6) |
| M4 | MRO | ✅ `mro.rs` (+ `bin/mro.rs`) |
| M5 | call tree core: build steps S1–S5 | 🟡 S1–S4 look done (keys, summaries, walker, calls, solver, recursion groups). **S5, the benchmark against Jedi, has no recorded numbers** |
| M6 | call tree part 2: S6–S11 | 🟡 partly: S7 closures ✅ ("closure cache", "closure fix"), S8 cuts and limits ✅ ("cut added", `Limits`), S9 graph output + stable IDs ✅ ("graph and stable id", 2026-11-03). **S6 CPA combos ❌** (no `combo` in the source). **S10** known-limit fixtures and **S11** PyCG / Jarvis / full Jedi parity: ❓ started (`run-jarvis.sh`), not finished |
| M7 | JSON-RPC over HTTP transport | ⏭ **not needed** by V-NOC (the driver does the transport) |
| M8 | performance: parallelism, cross-request cache (v1.1) | ❌ (= A12) |

**Last pylspt commit: 2026-11-03** ("output to json added for debug"). Nothing since the V-NOC pivot.

### Known limits pylspt documents (`plan/03-call-tree/15-checks-and-build-order.md`)

These are accepted gaps in call-tree precision, listed so you know what a "missing edge" in V-NOC's call tree might be:

- aliasing is tracked for top-level parameters only;
- writes to module globals are dropped;
- `fp_depth = 2` loses reads at depth 3;
- effects of a call inside a recursion cycle reach only the exit state; identity doesn't go around a cycle;
- rebinding a captured variable isn't shared between closures (`nonlocal`); a closure passed as an argument keeps the values it had at the call.

Planned fixes are in pylspt's "growth path": **v1.1** cross-request cache, **v1.2** precision (literals in keys, containers, `nonlocal` cells, loop fixpoint), **v1.3** read-set keys, **v2** summaries as salsa queries (incremental re-solving after an edit).

---

## Suggested order for pylspt work

1. **Commit or clean** the two uncommitted files, push, and pin the driver to a `rev` (A4). Fix the URL in `plan_vs/02-07` (`rust_python_parser`, not `pylspt`).
2. **A facade module** that re-exports the parser/AST/text-size types the adapter uses (A3).
3. **Let `plan_ids` take the ID per def** (a closure that receives the `DefNode`, or a map keyed by `(qualname, occurrence)`) and add **`replace_existing`** for UUID lines (A5, A6). Needed by M3.
4. Before M5: **record the S5 benchmark** (pylspt's own gate) on one real repo, and check A13 / A14.
5. After the MVP, when repos get big: **A10 + A11 together** (lazy expansion and real cancel, VS-D69), then the v1.1 cache (A12).

Next: back to [`README.md`](README.md)
