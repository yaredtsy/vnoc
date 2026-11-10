# 06. Code: the Python driver (`drivers/python/`)

## What a driver is

A **language driver** is a separate program that knows one language. The core starts it and talks to it over stdin/stdout (JSON-RPC, LSP framing), the same way VS Code
talks to the core. The driver answers questions like "what defs are in this file?" and "where would an `ID:` line go?". It knows nothing about `.vnoc/` or storage.
**The core picks IDs; the driver picks places** (06-01 rule 6).

Why separate: other languages (TS/JS) can ship later as separate downloads, and the same drivers work from a CLI or MCP server without VS Code.

## How it's built

- **Its own Cargo workspace** (`[workspace]` in its `Cargo.toml`), with its own `rust-toolchain.toml` (**Rust 1.98.0**, pinned by pylspt) and its own `Cargo.lock`
  (ruff only compiles against the `salsa` version pylspt locks: S2 finding).
- Depends on **pylspt** (`../../../pylspt`, a sibling repo: the Rust Python analyzer built on vendored ruff + ty) and on three ruff crates directly, because pylspt has no facade yet.
- **pylspt depends on no V-NOC crate** (VS-D65); this adapter is the only place that knows both.
- Transport: `lsp-server` (plain threads, VS-D68). The main loop only dispatches; each request runs on its own thread with a cancel flag.
- `#![deny(clippy::print_stdout, clippy::print_stderr)]`: stdout is the protocol, nothing else may write there.
- Dev profile compiles dependencies at `opt-level = 2` (unoptimised ruff is too slow). Release: `lto = "fat"`, `strip`, `panic = "abort"`.

| File | Job |
|---|---|
| `src/main.rs` | logging (filtered: `VNOC_LOG`, default `warn` — ty/salsa trace every query otherwise), handshake (refuses a wrong protocol with a reason), dispatch loop, `$/cancelRequest`, shutdown |
| `src/outline.rs` | **`vnoc/outline`**: every def and class of a file, flat, in source order, parents before children; qualname (with `<locals>`), kind, params, an `ID:` read from the docstring, per-def syntax-error flag |
| `src/positions.rs` | ruff's byte offsets → 0-based line + **UTF-16** column (what VS Code uses), at the boundary so nobody else sees a byte offset |

## Status

| Method | Spike S2/S3/S4 driver | Product driver |
|---|---|---|
| handshake (`initialize`, protocol 1, capabilities) | ✅ | ✅ |
| `vnoc/outline` | ✅ | ✅ |
| `vnoc/planIds` (where to put `ID:` lines) | ✅ (S3) | ❌ needed in **M3** (write the ID on attach) |
| `vnoc/defs` (per-def fingerprints for matching) | ✅ (S4) | ❌ P1 (ladder steps 2–6); M4 needs only step 1 |
| `vnoc/callTree`, `vnoc/expand` | ✅ (S2) | ❌ **M5** |
| `vnoc/environmentInputs` | listed (C22) | ❌ M5 (cache invalidation) |

**Tests:** `cd drivers/python && cargo test` → ✅ **10 / 10** (handshake, wrong protocol refused, qualnames + IDs match the domain's golden oracle, parents/kinds/params, docstring `ID:`
read, syntax error flags only the broken def, UTF-16 positions, reading by path, unknown messages, bad params).

## Open items for the driver

- **TODO(M1) in `Cargo.toml`:** depend on pylspt by git `rev` (+ a gitignored `[patch]` for local work) once pylspt is pushed; drop the three ruff crates when pylspt has a facade.
- `planIds` with `replace_existing` (replace an old UUID `ID:` line in place, VS-D79/D80 drafts).
- Cancel doesn't stop a running solve (pylspt has only a 10 s deadline); lazy call-tree pagination needs pylspt changes (VS-D69).
- With `panic = "abort"`, a panic kills the whole driver → the core must restart it (driver host, M1).

Design doc: `plan_vs/07-drivers/03-python-driver.md` (outline). Wire spec: `plan_vs/07-drivers/01-driver-protocol.md` (written).

Next: [`07-code-extension.md`](07-code-extension.md)
