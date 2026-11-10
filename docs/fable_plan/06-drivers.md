# 06. Talking to language drivers

You will write more drivers, so this file is about making the second one easy.

```
drivers
├── 1. the picture
├── 2. what is good in the protocol (keep)
├── 3. changes to make before the second driver
│   ├── the driver says which files it owns
│   ├── the driver uses the shared types
│   ├── three tiers of capability
│   └── additions for file IDs and ID styles
├── 4. the driver host in the core
│   ├── 0.1: add a timeout
│   └── 0.2: many requests at once (the reader-task pattern)
├── 5. several drivers at once
└── 6. writing a new driver: checklist + conformance test
```

## 1. The picture

```
VS Code extension ──(stdin/stdout, JSON-RPC)──► core (vnoc) ──(stdin/stdout, JSON-RPC)──► driver (one program per language)
                                                    │                                       ├── vnoc-driver-python  (wraps pylspt)
                                                    │                                       ├── vnoc-driver-ts      (later)
                                                    └── owns: index, .vnoc/, IDs            └── …
```

A driver is a separate program. The core starts it, sends it questions about source text, and gets facts and text edits back.
A driver **never** writes a file, never sees `.vnoc/`, and never chooses an ID. That split is the best decision in the design: a driver can be written in
any language and cannot damage a repo.

The wire spec is `plan_vs/07-drivers/01-driver-protocol.md`. It is good; this file only lists what to add.

## 2. What is good (keep)

| Choice | Why it is right |
|---|---|
| LSP-style framing (`Content-Length`) + JSON-RPC | every language has a library for it; you can debug it by eye |
| `initialize` with a protocol number and capability flags | a driver can be older or simpler than the core and both know it |
| `text` **or** `root` + `file` | the same method works on a saved file and on an unsaved buffer |
| positions as line + UTF-16 column | identical to VS Code, so edits apply without conversion in the core |
| the core picks the ID, the driver picks the place | one ID rule for all languages |
| `qualname` is opaque to the core | each language keeps its own naming (`<locals>` in Python) |

## 3. Changes to make before the second driver

### a) The driver says which files it owns (review C5)

Today the core's scan is hard-coded: `scan::python_files`, `extension == "py"`. Add this to the `initialize` result:

```json
{ "protocol": 1, "name": "python", "version": "0.1.0",
  "files": { "extensions": ["py", "pyi"] },
  "capabilities": { "outline": true, "ids.plan": true, "ids.file": true, "callTree": false } }
```

The core's scan then collects "files with an extension some driver claims" and sends each file to the driver that claimed it. Adding a language becomes
"start one more driver", with no core change.

### b) The driver uses the shared types (review D1)

`crates/vnoc-driver-protocol` has the Rust types for the wire, but only the core uses them. The Python driver builds its answers with `json!({...})` by
hand, so a renamed field is found at run time, by a user.

> **🦀 Rust note: `Serialize` and `Deserialize`.** These two `serde` traits turn a struct into JSON and back. Today `Def` only derives `Deserialize`
> (the core reads it) and `OutlineParams` only `Serialize` (the core writes it). Derive **both** on every wire type:
> ```rust
> #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
> pub struct Def { … }
> ```
> and in the driver replace `json!({ "qualname": …, … })` with `serde_json::to_value(Def { qualname, … })`. Now if you rename a field in the protocol
> crate, the driver **does not compile** until it is fixed. The compiler is doing a test for you.

`drivers/python` is its own workspace, but a path dependency across workspaces is fine (it already has one on `vnoc-domain` for tests):
`vnoc-driver-protocol = { path = "../../crates/vnoc-driver-protocol" }`. Keep that crate tiny (serde only) so it compiles with any toolchain.

For drivers **not** written in Rust, add example messages as files: `crates/vnoc-driver-protocol/fixtures/outline.request.json`,
`outline.response.json`, … A test in the protocol crate parses each one. The files are then both documentation and a check.

### c) Three tiers

Make it explicit that a driver can be useful long before it is complete:

```
Tier 1  STRUCTURE    initialize + vnoc/outline
                     → the language appears in the tree and on the canvas; docs attach by name
Tier 2  MARKERS      + vnoc/planIds (ids.plan), vnoc/planFileId (ids.file)
                     → docs survive renames and moves
Tier 3  CALL TREES   + vnoc/callTree, vnoc/expand, vnoc/environmentInputs
                     → the 0.2 feature
```

The identity model already supports this: a node from a tier-1 driver is simply `id_in_code = "no"`. A new language can ship at tier 1 in a weekend
(a tree-sitter grammar gives you an outline) and grow from there.

### d) Additions for this plan

All additive: an old driver that does not know them keeps working.

| Addition | For | Shape |
|---|---|---|
| `file_id` in the `outline` result | file markers ([`05-identity/03-files.md`](05-identity/03-files.md)) | top-level, next to `defs`: the ID text found in the file's `# FileID:` comment, or `null` |
| `vnoc/planFileId`, capability `ids.file` | file markers | `{ text, version, id }` → `{ version, edits, written, text_hash }` |
| `style` on `planIds` / `planFileId` requests; capabilities `ids.style.docstring`, `ids.style.comment` | the comment style, 0.4 ([`05-identity/02-functions-and-classes.md`](05-identity/02-functions-and-classes.md) §3) | reserve the names now; the Python driver announces only `ids.style.docstring` |

## 4. The driver host in the core

### 0.1: add a timeout (review C4)

Today, if the driver stops answering, the core's request waits forever, and it holds the lock on the driver's pipes while it waits.

> **🦀 Rust note: a timeout around any future.**
> ```rust
> use tokio::time::{timeout, Duration};
>
> match timeout(Duration::from_secs(10), request(&mut io, method, &params)).await {
>     Ok(answer) => answer,                                    // finished in time: Ok(value) or Err(HostError)
>     Err(_elapsed) => Err(HostError::Timeout(method.into())), // the future was dropped; treat the driver as broken
> }
> ```
> When this returns `Timeout`, the caller does what it already does for a broken connection: `host.forget()`. That drops the `DriverClient`, and because
> the child was started with `kill_on_drop(true)`, the stuck process is killed. The next request starts a fresh driver. You already built this recovery
> path; the timeout just makes it reachable.

That is all the host needs for 0.1. Outlines are fast and one-at-a-time is enough.

### 0.2: many requests at once

Call trees are slower, and the user will click "expand" on two nodes, or cancel one. Then the host needs to send several requests and match the answers
to them as they come back, in any order.

> **🦀 Rust note: the reader-task pattern.** One background task reads every message from the driver. Each waiting request holds a one-time channel
> (`oneshot`) and the reader delivers the answer through it.
> ```rust
> type Waiting = Arc<Mutex<HashMap<u64, oneshot::Sender<Result<Value, HostError>>>>>;
>
> // started once, when the driver starts
> tokio::spawn(async move {
>     while let Ok(msg) = read_frame(&mut stdout).await {
>         let Some(id) = msg.get("id").and_then(Value::as_u64) else { continue };   // a notification: ignore
>         if let Some(tx) = waiting.lock().unwrap().remove(&id) {
>             let _ = tx.send(to_result(msg));              // wake up the one request that was waiting for this id
>         }
>     }
>     // the driver closed its output: fail everyone who is still waiting
>     for (_, tx) in waiting.lock().unwrap().drain() { let _ = tx.send(Err(HostError::Closed)); }
> });
>
> // each request
> let (tx, rx) = oneshot::channel();
> waiting.lock().unwrap().insert(id, tx);
> send(&mut *stdin.lock().await, &message).await?;          // the lock is held only while writing, not while waiting
> let answer = timeout(LIMIT, rx).await;
> ```
> A `oneshot` channel is a pipe that carries exactly one value: one side sends, the other awaits. A `HashMap` of them, keyed by request id, is the
> standard way to match answers to questions on any connection.

With this in place, `$/cancelRequest` is one more `send`, and the driver side already handles it. Add a limit to the driver's thread-per-request loop at
the same time (review D2).

## 5. Several drivers at once

```rust
pub struct Drivers {
    by_extension: HashMap<String, Arc<DriverClient>>,     // "py" → python, "pyi" → python, "ts" → typescript
}
impl Drivers {
    pub fn for_file(&self, path: &RepoPath) -> Option<&Arc<DriverClient>> { … }
}
```

- The extension passes a **list** in `initializationOptions`: `drivers: [{ path: "…/bin/vnoc-driver-python" }]`. Keep reading the single `driverPath` until
  a second driver exists.
- The core starts each one lazily, reads `files.extensions` from its handshake, and fills the map. Two drivers claiming one extension: the first wins and
  a warning is logged.
- A driver that crashes takes down only its own language. The tree still shows the others.
- **Where drivers live:** in 0.1, inside the VSIX's `bin/` and nowhere else. The plan's shared drivers folder, copy-on-first-run, downloads and signatures
  solve a problem (drivers shared with a CLI, drivers updated separately) that does not exist until there is a CLI or a third-party driver. After 1.0.

## 6. Writing a new driver

### The checklist

```
□ reads LSP-framed JSON-RPC from stdin, writes it to stdout; logs ONLY to stderr
□ initialize: refuses an unknown protocol number; answers name, version, files.extensions, capabilities
□ requires `initialized` before work; `shutdown` + `exit` (and stdin closing) end the process
□ unknown notification → ignored; unknown request → error -32601; never exits because of either
□ vnoc/outline: flat list, source order, parents before children; ranges in UTF-16 columns
□ a file with syntax errors still returns what it can, with syntax_error flags
── tier 2 ──
□ outline reports the IDs it finds (defs and file), as written
□ vnoc/planIds / planFileId: edits in old-text coordinates; echo `version`; skip defs that do not parse
□ applying the returned edits and running outline again shows the ID   ← the round-trip rule
── tier 3 ──
□ vnoc/callTree, vnoc/expand, vnoc/environmentInputs
```

### The conformance test

Turn the checklist into a program, so a new driver is checked by running one command instead of by reading a document.

- **Now:** a test in the core, `crates/vnoc/tests/driver_conformance.rs`, that takes a driver path from an environment variable and runs: handshake · a
  wrong protocol number is refused · an unknown method gives `-32601` · any byte on stdout outside a frame fails the run · outline of a fixture with an
  emoji in it (catches UTF-16 mistakes) · the round-trip rule. Run it against the Python driver in CI.
- **Later:** the same checks as a command, `vnoc driver check <path>`, for people who write drivers outside this repo.

The parts that differ per language (the fixture file, the expected names) live in a small folder per driver:
`drivers/python/conformance/{input.py, expected-outline.json}`.

Next: [`07-ui/01-state.md`](07-ui/01-state.md)
