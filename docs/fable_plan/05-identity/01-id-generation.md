# 05.01 How an ID is made

```
id generation
├── 1. the rule today (already built)
├── 2. my opinion: keep the hash, know its price
├── 3. two small fixes before any ID reaches a user
│   ├── paths from the git root
│   └── Unicode in paths
├── 4. the write flow: persist() must become two steps
└── 5. IDs for files and folders
```

## 1. The rule today

Implemented and tested in `crates/vnoc-domain/src/id.rs`:

```
input   =  repo_path  \0  qualname  \0  occurrence          (+ \0 attempt, only after a collision)
hash    =  BLAKE3 in key-derivation mode, context "v-noc node id v1"
id      =  first 16 bytes → a 128-bit number → 26 characters of Crockford base32, upper case

example    src/orders.py \0 OrderService.parse_order \0 1   →   0Q8V5W4T2F7KX9M3B6D1RZ4H7K
```

- **Made once, at the first attach. Never recomputed.** After a rename the ID no longer "matches" its path, and that is correct: it is a name, not a checksum.
- **Collision** (the node folder for that ID already exists): hash again with attempt 2, 3… until a free one comes out.
- The shard is the last two characters: `.vnoc/nodes/7K/0Q8V…H7K/`.

## 2. My opinion: keep it, and know its price

There were two candidates in your history: **random** IDs (round 5) and **hash-derived** IDs (round 6, built).

| | Hash-derived (what you have) | Random |
|---|---|---|
| Two people document the same function on two branches | **same ID** → same folder, same `ID:` line → git merges cleanly | two IDs → a merge conflict inside the source file's docstring |
| Tests | deterministic: golden values | need a fake random source |
| Complexity | higher: `occurrence`, collision counter, exact path bytes matter | one line: generate 128 random bits |
| A function deleted and re-created under the same name | hashes to the old ID; the collision check must notice the old (archived or trashed) folder | no interaction |

**Keep the hash.** It is built, it is tested against an independent oracle, and its one real benefit (clean merges) is exactly the kind of thing that makes
a team trust a tool. The price is the list of small rules below. They are manageable as long as everyone remembers one sentence:

> **The hash is only a way to pick a good ID at creation time. After that the ID is opaque.** No code may recompute it to "verify" a node.

If some future case makes the derivation painful, you can switch new IDs to random **without any format change**: the text format, the folders and the
markers stay identical. That escape hatch is a good reason not to over-polish the derivation rules.

## 3. Two small fixes before any ID reaches a user

Both are free today, because no real ID has been committed anywhere yet. Both become impossible to change once one has.

### a) Paths must be relative to the git root (review C1)

The core currently uses the **opened folder** as the root. Open `repo/` and a function is `services/api/orders.py::f`; open `repo/services/api/` and the
same function is `orders.py::f`. Two people who open different folders would derive different IDs, which defeats the reason for hashing.
→ Find the git root once ([`../04-storage/01-vnoc-folder.md`](../04-storage/01-vnoc-folder.md) §1) and build every `RepoPath` from it.

### b) Unicode in paths (review C7)

`RepoPath`'s documentation says "take the path from git, never from a directory listing", because macOS returns file names in decomposed form (`e` + accent
as two characters) while git usually records the composed form (`é` as one). The scan takes paths from the disk, so on a Mac a file named `café.py` would
hash differently than on Linux.

Reading paths from git's index for this one reason is a heavy dependency. A lighter rule gives the same guarantee:

> **Normalise the path to NFC before hashing it.** NFC is the composed form. Every machine then feeds the same bytes to the hash, whatever its file system
> returns.

> **🦀 Rust note.** The `unicode-normalization` crate adds `.nfc()` to any string iterator:
> ```rust
> use unicode_normalization::UnicodeNormalization;
> let for_hash: String = path.as_str().nfc().collect();
> ```
> Do this **inside `NodeId::derive`**, on the hash input only. `RepoPath` keeps the real bytes, because that is what must be used to open the file.
> Update the Python oracle with `unicodedata.normalize("NFC", path)` so the golden values still agree.

This only matters for non-ASCII file names, but it costs four lines now and cannot be done later.

## 4. The write flow: `persist()` must become two steps

Today:

```rust
pub fn persist(location, is_taken, writer: &mut dyn IdWriter) -> Result<NodeId, PersistError>
//   picks the ID, calls writer.write_id(...), returns the ID only if the write worked
```

The intention is good: "an ID that exists is already in the code". The problem is that the real write is not a function call. It is a conversation:

```
core ──► driver        "where does this ID go in this text?"      (async)
core ◄── driver        text edits
core ──► VS Code       "apply these edits"                         (async)
core ◄── VS Code       applied: true / false
```

A plain trait method cannot wait for that. And since tracking is now optional, "no ID without a write" is no longer even the rule: a node with
`id_in_code = "no"` has an ID and no write.

**Suggested shape:** keep the part that is pure, and move the conversation into the attach handler.

```rust
// vnoc-domain: pure, easy to test, no I/O
pub fn first_free_id(location: &LocationKey, is_taken: impl Fn(&NodeId) -> bool) -> Result<NodeId, NoFreeId> {
    for attempt in 1..=MAX_ATTEMPTS {
        let id = NodeId::derive(location, attempt);
        if !is_taken(&id) { return Ok(id); }
    }
    Err(NoFreeId(MAX_ATTEMPTS))
}
```

```rust
// crates/vnoc/src/identity/attach.rs: the async flow (04-storage/03 §3 has the full list of steps)
async fn attach(ctx: &Ctx, req: AttachParams) -> Result<AttachResult, AttachError> {
    let id = match ctx.index.id_at(&req.location) {
        Some(id) => id,                                                    // already has one
        None     => first_free_id(&req.location, |id| ctx.userdata.is_taken(id))?,
    };
    let in_code = if ctx.config.markers_on() {
        write_marker(ctx, &req, &id).await                                 // Yes, or No if it could not be written
    } else {
        IdInCode::No
    };
    ctx.userdata.create_node(&id, &NodeToml::new(&req.location, in_code))?;
    let doc = ctx.userdata.create_doc(&id, &req.title, &req.location.to_string())?;
    Ok(AttachResult { id, doc, in_code })
}
```

> **🦀 Rust note: `async fn` and `.await`.** An `async fn` does not run when you call it; it returns a *future*, a value that means "this work, not done
> yet". `.await` runs it and pauses **this** function (not the thread) until the answer arrives, so the core can serve other requests meanwhile. That is
> why the flow above can talk to the driver and to VS Code in the middle without blocking anything. The `?` after a call means "if this is an error, stop
> here and return it".

### How the core asks VS Code to apply an edit

You do not need a custom message for this. The core already speaks LSP framing, and LSP has a standard request **from the server to the editor**:
`workspace/applyEdit`. `vscode-languageclient` handles it by itself and answers `{ applied: true | false }`.

```
extension ──► core     vnoc/attach { location, title, document: { uri, version, text } }
core ──► driver        vnoc/planIds { text, version, ids: [...] }        → edits
core ──► extension     workspace/applyEdit { edits for uri at version }   ← standard LSP, no code to write in the extension
core ◄── extension     { applied: true }
core                   writes node.toml + the doc
extension ◄── core     { id, doc path }       → the extension saves the source file if it was not dirty before, opens the doc
```

- Send the edit as a **versioned** document edit (`TextDocumentEdit` with the `version` the extension gave you). If the user typed in between, the versions
  differ and the edit is refused instead of landing in the wrong place **[check: I believe vscode-languageclient compares versions; test it]**.
- In `async-lsp`, the `ClientSocket` passed to `MainLoop::new_server(|client| …)` is how the core sends requests to the editor. Today it is ignored
  (`_client`). Keep it in `Core` **[check the exact method name: `client.request::<ApplyWorkspaceEdit>(…)` or `client.apply_edit(…)`]**.

This removes the `IdWriter` trait entirely. To test the flow without VS Code, the lifecycle test harness plays the editor and answers `applied: true`.

## 5. IDs for files and folders

Same formula, with an empty `qualname`:

```
file     src/orders.py  \0  ""  \0  1
folder   src/billing    \0  ""  \0  1
```

A file and a folder cannot share a path at the same time, so they cannot collide at creation. If a path was a file last year and is a folder today, the
normal collision check ("is that node folder taken?") handles it with attempt 2.

The **root folder** has no path (`RepoPath` refuses an empty string), so it cannot be a node in 0.1. Project-wide docs are a later question
([`../08-decisions-to-log.md`](../08-decisions-to-log.md), open question Q3).

Next: [`02-functions-and-classes.md`](02-functions-and-classes.md)
