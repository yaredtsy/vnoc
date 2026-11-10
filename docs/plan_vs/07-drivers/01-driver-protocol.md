# 07.01: The core ↔ driver protocol

> **Status:** ✅ written after **S2** (and S3, S4) · **Program:** driver protocol · **Milestones:** M1 (outline, handshake), M3 (`ids.plan`), M5 (call trees)

**Purpose:** The contract every language driver implements, versioned so drivers ship separately (VS-D29.1).
**This file is the wire spec.** It does not depend on any crate: `vnoc-driver-protocol` provides Rust types and helpers that follow it, and a driver may use any transport library (VS-D68).
The Python driver uses `lsp-server`; the core uses `async-lsp` (VS-D67).

**Builds on:** [02-07 §2](../02-technology/07-repo-and-drivers.md) · spikes S2 (`spikes/s2-python-driver/`), S3 (`vnoc/planIds`), S4 (`vnoc/defs`).

---

## 1. The wire

- **JSON-RPC 2.0** over the driver's **stdin / stdout**, each message framed LSP-style: `Content-Length: <bytes>\r\n\r\n<json>`.
- **stdout is the protocol and nothing else.** Not a log line, not a warning, not a panic message. Logs go to **stderr**, filtered (`VNOC_LOG=debug`).
  A vendored crate that calls `println!` would still break the stream, so the core's test harness fails a run on **any byte outside a frame** (S2/C2).
- Requests carry an `id`; **responses may arrive in any order**. Notifications have no `id` and no reply.

## 2. Lifecycle

```
core                                   driver
 │ ── initialize { protocol: 1 } ───►   │   refuses a protocol it does not speak (error -32600, with the reason)
 │ ◄── { protocol: 1, name, version, capabilities } ──
 │ ── initialized (notification) ───►   │   REQUIRED before any request: a request first ends the driver
 │ ── vnoc/… requests ───────────────►  │
 │ ── shutdown ──────────────────────►  │   stop accepting work, answer null
 │ ── exit (notification) ───────────►  │   leave. stdin EOF means the same.
```

- `initialize` result: `{ "protocol": 1, "name": "python", "version": "0.1.0", "capabilities": { … } }`. **`capabilities` must be present** (an empty object is fine): stock LSP clients refuse a reply without it.
- **Optional methods** are announced as booleans in `capabilities`, and the core checks before calling: `"outline"`, `"ids.plan"`, `"callTree"`, `"environmentInputs"` (a driver that cannot carry IDs says `"ids.plan": false`).
- **Unknown notifications are ignored. Unknown requests get `-32601`.** A driver never exits because of a message it does not understand.
- The core refuses a driver whose `protocol` is outside its supported range and says why; the driver never has to guess.

## 3. Concurrency and cancellation (VS-D68)

- The driver's main loop **only dispatches**. Each request runs on a worker with its own cancel flag, so **`$/cancelRequest { id }`, `shutdown` and stdin EOF are handled while analysis runs**.
- A cancelled request answers **`-32800` (RequestCancelled)**. If the analysis is already running it may finish; its answer is **dropped**. (pylspt has a `deadline` but no external cancel flag; real interruption is deferred with pagination, VS-D69.)
- On `shutdown`, `exit` or EOF every running request is told to stop.

## 4. Positions and text

- **Every position on the wire is 0-based line and a UTF-16 code-unit `character`**, as in LSP and VS Code. A range is `{ "start": Position, "end": Position }`. (pylspt's own positions are 1-based line and character-counted, and ruff works in byte offsets: **the driver converts at its boundary**, so nothing else ever sees a byte offset.)
- Requests that look at a document take **`text`** (the editor's unsaved buffer) or **`root` + `file`** (a path the driver reads). With `text`, the driver needs no project and no file on disk; it is fast (§7).
- **A driver never writes a file.** It returns `TextEdit`s (`{ range, newText }`, in the coordinates of the text it was given) and spans; the host applies them through `WorkspaceEdit`.

## 5. Methods (protocol 1)

### `vnoc/outline` · capability `outline` · M1
`{ text }` or `{ root, file }`, optional `fingerprints: bool` → 

```json
{ "defs": [
    { "qualname": "OrderService.parse_order", "name": "parse_order", "kind": "function",
      "parent": "OrderService", "parent_kind": "class",
      "range": { "start": {"line": 4, "character": 4}, "end": {"line": 12, "character": 30} },
      "params": ["self", "raw"], "id": "3RGJ87CMS6TK775XRJV8BW977Y", "syntax_error": false } ],
  "syntax_errors": [ { "range": …, "message": "Expected `:`, found `return`" } ] }
```
- `defs` are **flat, in source order, parents before children**. The core counts the `occurrence` of each qualname itself (`vnoc-domain`: one rule, one place).
- **`qualname` is the language's own name, opaque to the core.** For Python it is `__qualname__` **including `<locals>`** (`top.<locals>.inner`).
- **`id`** is the text of the ID the driver **read in the code** (or `null`), **as written**: the core parses it with `NodeId::parse`, and the core classifies it: a `NodeId` is V-NOC's; an old UUID is replaced (`replace_existing` in `planIds`); anything else is the user's own `ID:` text and is left alone.
- **`syntax_error`** is `true` for a def whose lines contain a parse error (S4/C17). It is on the safe side: an unclosed `(` after an operator can pull the next def into the broken one, so a neighbour may be flagged too. Never the other way round.
- `fingerprints: true` adds `{ "body_hash", "tokens" }` per def (the body with comments, blank lines and indentation removed). **Post-MVP (P1)**: the matching ladder's steps 4-5 use it.

### `vnoc/planIds` · capability `ids.plan` · M3
**The core picks the ID; the driver picks the place.**

```json
request:  { "text": "…", "version": 7,
            "ids": [ { "qualname": "OrderService.parse_order", "occurrence": 1, "id": "3RGJ87CMS6TK775XRJV8BW977Y", "replace_existing": false } ] }
response: { "version": 7,
            "edits":   [ { "range": {…}, "newText": "\n    ID: 3RGJ…\n" } ],
            "written": [ { "id": "3RGJ…", "qualname": "…", "occurrence": 1, "span": {…}, "text": "\n    ID: 3RGJ…\n" } ],
            "skipped": [ { "qualname": "…", "occurrence": 1, "reason": "syntax error inside this def" } ],
            "unmatched": [ ],
            "text_hash": "<blake3 of the new text, lowercase hex>" }
```
- Planned on the **buffer** (`text`), never the file on disk. **`version` is echoed**: the host drops the plan if the document changed meanwhile and retries once (S3).
- `edits` are in **old-text** coordinates (a `WorkspaceEdit` applies them all against the original). `written[].span` is in **new-text** coordinates and `written[].text` is the exact text inserted; the core stores both (removal later needs the exact text, S3).
- A def with a syntax error is **skipped on its own**; the others still get their ID. **The driver acts on every item it is given; the core decides which defs need an ID** (it never asks for a def that already has a valid one) and the driver does not interpret an existing `ID:` line. **`replace_existing: true`** means: replace the existing `ID:` line **in place (its exact span)** instead of inserting; the core sets it **only when that line's value parses as an old UUID** (VS-D79). With `false` (the default) the driver **inserts V-NOC's own line, even if the docstring already holds other `ID:` text**, which is the user's. `unmatched` lists requested `(qualname, occurrence)` pairs that are not in the text.
- Python writes an **`ID: <id>` line in the def's or class's docstring** (extending a docstring that exists, adding a `"""ID: …"""` statement otherwise). Nothing else: no comment form.
- `text_hash` lets the core recognise "this is exactly what I predicted" after the save and skip a re-parse.

### `vnoc/callTree`, `vnoc/expand` · capability `callTree` · M5
`{ root, file, entry, depth? }` → a tree `{ "target": { "file", "qualname" }, "call_site": Range | null, "children": [ … ], "marks": [ … ] }`. `depth` shortens the **output**; the solver still does the whole tree (S2/C1), which is cheap on real repos (about 100 ms on httpx). `expand` is `callTree` rooted at a callee.
**The exact shape and the `marks` are finished when M5 is designed**; the names above are reserved.

### `vnoc/environmentInputs` · capability `environmentInputs` · M1
`{ root }` → `{ "files": ["pyproject.toml", "uv.lock", "poetry.lock", "pdm.lock", "Pipfile", "Pipfile.lock", "requirements*.txt", "constraints*.txt"] }`: globs, relative to `root`, whose content decides "the environment changed" (VS-D53). The core hashes those files. (Python's list is S2/C22's.) **Whole files are accepted for the MVP**: a change only clears the call-resolution data, the structure index is unaffected. **The narrower trigger (only the `pyproject.toml` tables that hold dependencies, D4) is post-MVP.**

### Reserved, not in the MVP
`vnoc/planRemoveIds` (Remove ID, P4), `vnoc/planFileId` (the `# FileID:` line, P4). Both were prototyped in S2/S3 and work on exact text.

## 6. Errors

Standard JSON-RPC codes, plus the LSP one for cancel. The `message` is for people; the core branches on the code.

| Code | When |
|---|---|
| `-32600` | handshake refused (the wrong `protocol`), or a malformed request |
| `-32601` | unknown method |
| `-32602` | bad params (a missing `text`, a position outside the text) |
| `-32603` | the driver failed (a file that cannot be read, a project that cannot be opened) |
| `-32800` | the request was cancelled |

## 7. Hygiene and what to expect (measured in the spikes)

- **A crash ends the driver; the core restarts it.** Release builds use `panic = "abort"`: a panic in any worker kills the whole process and every request in flight fails. The host retries nothing silently.
- **Speed.** A `text` request is fast: planning IDs for a 4,200-line file takes about **11 ms (p50)** including the outline, and a hook that only reads a precomputed plan costs 0.2 ms. A request that needs the **project** pays about **130 ms once** per process (loading typeshed) and is warm after.
- **Size.** The Python driver is about 16 MB, 8.4 MB zipped (macOS arm64, LTO).
- **Logging.** Unfiltered, the type checker traces every query (tens of MB per run). A driver must filter by default.

## 8. Versioning

- `PROTOCOL_VERSION` is an integer, sent in `initialize`. **Additive changes keep the number**: a new optional field, a new method behind a capability. Readers **ignore fields they do not know**.
- **A breaking change bumps it**: a changed meaning, a removed field, a changed position convention. The core compiles in the range it speaks (`SUPPORTED_DRIVER_PROTOCOLS`) and picks the newest driver in that range.
- **Until `1.0` the protocol may change freely** (nothing outside one machine depends on it). **At `1.0` it is frozen as protocol `1`** (VS-D29.1).

## MVP vs stable

| MVP (0.x) | Stable (1.0) |
|---|---|
| handshake, `outline`, `planIds`, `callTree` / `expand`, `environmentInputs` | frozen as protocol 1; `planRemoveIds`, `planFileId`, `fingerprints` (P1, P4) |

## Decisions in this doc (agreed; draft row VS-D80 in `spikes/s4-git/DRAFT-doc-rows.md`)

1. **One position convention for the whole wire:** 0-based line, UTF-16 `character`; the driver converts ruff's byte offsets at its boundary.
2. **`planIds` is keyed by `(qualname, occurrence)`**, not by a position. The spike keyed by position; this is a change from what was tested. The core also decides `replace_existing`.
3. **`outline` is one flat request** with optional fingerprints, replacing the spike's two.
4. **`environmentInputs` returns whole files to hash** (MVP); the narrower pyproject trigger is post-MVP.
5. **The qualname rule (`<locals>`) is the driver's job.** The spike driver builds dotted names without `<locals>` and is **left as it is**: nothing in the product may read IDs the spike produced.
