# 05.02: S2 Python driver

**Goal:** `vnoc-driver-python` runs as its own process, spawned by the core from the global drivers folder, and serves outlines and lazy call trees from pylspt.
**Answers:** C1 (does `render_depth` bound the solving?), C2 (stdout hygiene), C4 (binary size), C11 (OS quarantine / Defender), C12 (lock crate, data dir),
C17 (per-function parse errors), C22 (which `pyproject.toml` sections count as dependencies).
**Time box:** 3 days. **Code:** `spikes/s2-python-driver/`. **Needs:** S1's `spike-core`.

## Build

1. **The adapter crate** `spike-driver-python`, depending on pylspt by `rev`, with a gitignored `.cargo/config.toml` `[patch]` pointing at `../pylspt` (VS-D65).
   pylspt itself gets **no** dependency on any V-NOC crate.
2. **Handshake:** the core sends `initialize { protocol: 1 }`; the driver answers `{ protocol: 1, name: "python", version }`. Wrong protocol → the core refuses and says why.
3. **`outline(file)`** → files, classes, functions with ranges. **`callTree(fn, depth: 3)`** and **`expand(callNode, depth: 3)`**. Both go through the adapter, which turns
   pylspt's types into protocol types.
4. **C1:** time `callTree` at depth 1, 3, 5 and "unbounded" on a real mid-size repo (e.g. `httpx` or `rich`). If depth 3 costs nearly as much as unbounded, solving isn't bounded
   and lazy loading needs work inside pylspt.
5. **C2:** run the driver with a stdout guard in the core: any byte that isn't a framed message fails the run. Include runs that trigger ruff/ty warnings and panics.
6. **C4:** release build with `strip = true`, `lto = "fat"`, `codegen-units = 1`, `panic = "abort"`; measure per platform; zip it.
7. **Copy-on-first-run (VS-D30):** the core copies the bundled binary into `<data dir>/vnoc/drivers/python/<ver>/` under a lock file, then spawns it from there.
   **C11:** does macOS refuse to run the copy (quarantine attribute, Gatekeeper)? Does Windows Defender slow the first start? **C12:** `fd-lock` vs `fs4` on macOS,
   Linux, Windows and a network home directory; what `dirs::data_local_dir()` returns on Windows.
8. **C17:** a file with a syntax error in one function: can pylspt say *which* defs contain an error, so save-time IDs skip only those?
9. **C22:** list the `pyproject.toml` tables that hold dependencies across tools (`[project]`, `[dependency-groups]`, `[tool.poetry]`, `[tool.uv]`) and the lockfiles
   (`uv.lock`, `poetry.lock`, `pdm.lock`, `requirements*.txt`). This becomes the driver's "environment inputs" list (VS-D53).

> **🦀 Rust note: one process, two jobs, no shared stdout.**
> The driver's stdout is the protocol, so in the adapter:
> ```rust
> #![deny(clippy::print_stdout, clippy::print_stderr)]   // even eprintln! goes through tracing
> fn main() {
>     tracing_subscriber::fmt().with_writer(std::io::stderr).init();
>     let (conn, _io) = lsp_server::Connection::stdio();      // the only thing that writes stdout
>     // …
> }
> ```
> A vendored crate that calls `println!` would still break the stream. Step 5 is there to catch exactly that.

## Measure

- `outline` time per file and for the whole scope; `callTree` time at each depth (C1).
- Driver binary size, raw and zipped, per platform (C4).
- First-start time after copy, per OS (C11).

## Pass / fail

- ✅ The core spawns the driver from the data dir, the handshake works, and a depth-3 tree renders in the S1 canvas.
- ✅ Each of C1, C2, C4, C11, C12, C17, C22 has an answer with numbers below.

## Findings

*Core loop done (2026-11-05): steps 1-3, 5, 8, 9. Steps 6 (partly), 7 (C11, C12) are the second pass. Code: `spikes/s2-python-driver/`.*

**What works.** `spike-driver-python` (adapter over pylspt) does the handshake (wrong protocol is refused with a reason), `outline`, `callTree` and `expand`.
The main loop only dispatches; each request runs on its own thread with a cancel flag (VS-D68). `drive.py` plays the core and passed on `httpx`:
handshake, outline, depth 1 and 3, cancel (`-32800`), unknown method, `shutdown` + `exit`.

**C2 stdout hygiene: answered.** `drive.py` fails the run if any byte on stdout is outside a `Content-Length` frame. A deliberately noisy driver (`noisy`) is caught,
so the guard works. The normal driver was clean, including a made-up notification and an unknown method. Not tried: panics and ruff/ty warnings that write to stdout
(none happened in these runs). Keep `deny(clippy::print_stdout, print_stderr)` and keep this harness as a CI test.

**C1 render_depth vs solving: NOT bounded.** `httpx/_client.py`, release build, ty warmed up first, each depth on a fresh database:

| entry | depth 1 | depth 3 | depth 5 | unbounded | JSON at depth 1 / unbounded |
|---|---|---|---|---|---|
| `Client.send` | 100 ms | 73 ms | 81 ms | 85 ms | 1.9 KB / 287 KB |
| `Client.request` | 83 ms | 79 ms | 81 ms | 92 ms | 0.9 KB / 443 KB |
| `Client.get` | 89 ms | 77 ms | 112 ms | 93 ms | 0.6 KB / 444 KB |

Depth 3 costs the same as unbounded, so `render_depth` shortens the **output** (hundreds of times smaller) but the solver still does the whole tree.
It is cheap on httpx (under ~100 ms; a repeat call on the same database takes 6-28 ms), so this is not a problem *yet*. A larger repo may behave differently.
Also: the first request in a fresh process pays ~130 ms for loading ty / typeshed. **Pagination (lazy expansion with caching and resuming) needs changes inside pylspt: deferred, VS-D69.**

**C17 per-function parse errors: yes, with one caveat.** Nodes carry no error flag, but ruff's parser lists every syntax error with a range, and the adapter marks each def whose lines contain one.
- One error in one function: only that function is flagged (missing `:`, unclosed `foo(x,`, bad expression `x +* 2`). Two independent errors: each flagged alone.
- **Caveat:** an unclosed `(` after an operator (`y = (x +`) makes the parser swallow the next `def` into the broken one, so the neighbour is flagged too.
  The error is on the safe side: a few extra functions get skipped for ID writing, never a wrong one.

**C4 binary size (macOS arm64, `lto="fat"`, `codegen-units=1`, `strip`, `panic="abort"`): 16.2 MB raw, 8.4 MB zipped.** Clean release build: 7 min 18 s. Incremental relink after a code change: ~2.4 min.
Linux and Windows not measured. Note `panic = "abort"`: a panic in any worker thread kills the whole driver, so the core must restart it (a design-doc item).

**C22 environment inputs (from the tools' own formats; httpx confirmed only `[project]` and `requirements.txt`; the other tools were not run).**
- Tables that hold dependencies: `[project] dependencies`, `[project.optional-dependencies]`, `[dependency-groups]` (PEP 735), `[tool.poetry] dependencies` and `group.*.dependencies`,
  `[tool.uv] dev-dependencies` / `constraint-dependencies` / `override-dependencies` / `sources`, `[tool.pdm] dev-dependencies`, and `[tool.hatch.envs.*] dependencies`.
  `[build-system] requires` is build-time only; leave it out.
- Lockfiles: `uv.lock`, `poetry.lock`, `pdm.lock`, `Pipfile.lock` (+ `Pipfile`), `requirements*.txt`, `constraints*.txt`.
- Rule for VS-D53: hash these tables and files; any change means "environment changed".

**Other findings.**
1. **Lockfile must be pylspt's.** A fresh `Cargo.lock` picked `salsa 0.28.5` and ruff failed to compile (`HashEqLike`); `0.28.2`, pylspt's lock, works. The monorepo must pin it.
2. **Toolchain:** pylspt pins Rust 1.98.0; a path dependency does not inherit `rust-toolchain.toml`, so the spike folder pins it too.
3. **pylspt needs a facade.** It re-exports no ruff crate, so the adapter depended on `ruff_db`, `ty_project` and `ruff_source_file` directly.
4. **Logging:** with no filter, ty/salsa trace-log every query (43 MB for one run, which also ruined the first timings). The driver now filters (`VNOC_LOG`, default `warn` + our `info`).
5. **No external cancel flag in pylspt** (only `Limits::deadline`, 10 s). A cancelled request's answer is dropped but a running solve is not stopped. Deferred with pagination (VS-D69).
6. **`lsp-server` enforces the LSP handshake:** a request before `initialized` ends the driver. Fine, but the core must always send `initialized`.
7. **Still open (second pass):** C11, C12, copy-on-first-run (step 7).
   - **C11 scope (macOS first, by hand):** (a) the browser-download + `vnoc driver install <file>` path, where quarantine applies; (b) the core's own HTTP download, which should carry
     **no** quarantine attribute (confirm with `xattr`); (c) the same two on Windows later: Mark-of-the-Web, SmartScreen, Defender's first-run scan delay.
   - **C4 addition: macOS arm64 binaries must be at least ad-hoc signed, or the OS kills them on launch.** If CI cross-compiles from Linux, sign with `rcodesign`, or build on macOS runners.
     Check this for the binaries inside the v1 VSIX too, not only for downloaded drivers.


Next: [`03-edits-and-saving.md`](03-edits-and-saving.md)
