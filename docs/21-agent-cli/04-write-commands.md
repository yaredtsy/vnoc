# 21.04 The write commands

```
write commands
├── 1. the list
├── 2. the seam            who applies the planned edit to the source file
├── 3. init
├── 4. doc new             the attach flow, from a terminal
├── 5. editing a doc       no command: it is a file
├── 6. relink · repair
├── 7. two writers         VS Code's core and a command, at the same moment
└── 8. what a write prints
```

Four commands write (C3). None removes anything.

## 1. The list

```
vnoc init [--markers on|off]                     create .vnoc/ (asks nothing; a person runs it, or tells the agent to)
vnoc doc new <node> --title "…" [--body-file <file> | -] [--no-marker] [--again] [--author <name>] [--tag <t>]…
vnoc relink <ID> <node>                          a Missing node's docs go to where the code is now
vnoc repair [<node>…] [--all]                    write lost ID lines back
```

`<node>` can be `/`, the project root, once step A17 is in ([`10-root-docs.md`](10-root-docs.md)).

## 2. The seam

The extension's rule is "the core creates, VS Code edits" (`04-storage/03-writing-safely.md` §1): the driver **plans** the `ID:` line as text
edits, and `identity/attach.rs` `apply_edits` sends them to the editor with `workspace/applyEdit`. `Env` holds a `ClientSocket` for that. In a
terminal nobody is on the other end.

**The change (step A7), and nothing else about the flows changes:**

```rust
// crates/vnoc/src/identity/edits.rs
pub enum EditSink {
    Editor(ClientSocket),        // today's code, moved: workspace/applyEdit with the document's version
    Disk,                        // the file on disk is the document
}
impl EditSink {
    pub async fn apply(&mut self, document: &Document, label: &str, edits: &[TextEdit]) -> Result<(), String>
}
// Env { project, index, host, edits: EditSink }        was: client: ClientSocket
```

An enum, not a trait: there are two cases, both known, and `async fn` in a trait object would need a crate for it.

**`Disk`:**

1. the caller read the file to make the `Document` (its text; version `0`);
2. the driver planned against that text, as it does for the editor;
3. `apply` reads the file **again**: if its bytes are not the text the plan was made for, it answers `Err("the file changed while the ID was
   being written")`, and the flow does what it already does when the editor refuses: the doc is attached **by name** (`id_in_code = "no"`), with
   the notice;
4. else it applies the edits to the text and writes with `userdata::atomic::write_atomic` (temp file, then rename).

- Line ends and a byte-order mark are kept: the edits insert text, nothing is re-encoded. A file that is not UTF-8 gets no marker (by name, with
  the notice).
- **[check]** in A7: the core has no function today that applies the driver's `TextEdit`s to a string (the extension does it in VS Code). It
  needs one, using the same position rule as the driver (`drivers/python/src/positions.rs`: which unit a `character` counts). A table test with a
  non-ASCII line before the edit.
- **[check] answered (A7, 2026-11-19): `identity::edits::apply_text_edits(text, edits)`**, the driver's position rule exactly: 0-based `line`, lines split
  on `\n` (a `\r` is part of its line), `character` in UTF-16 units from the line's start. The edits are sorted by position (stable) and applied last to first.
  An edit outside the text (a line past the end, a `character` past the line's end or inside a two-unit character), one that ends before it starts, or two that
  overlap are `Err` with nothing applied; ranges that only touch are fine, and inserts at one place keep their order. The table test has a line with `é` and an emoji
  before the edit, CRLF, a BOM, two edits out of order, one past the end, and an overlap.
- **As built in A7:**
  - `Document::from_disk(path)`: URI of the file, version `0`, the text decoded verbatim (a BOM stays as U+FEFF, line ends as they are); a file that is not UTF-8 is
    `InvalidData`. `document_for` returns a `MarkerDoc` (`Folder`, `Open(Cow<Document>)`, `Unreadable(reason)`): with `Disk` and no document it reads the file, and
    an unreadable one is `Unreadable`, which `write_marker` turns into the `Err` the flows already handle (by name, with the notice "…(the file is not UTF-8 text)…"). It is not an
    `AttachError`: Relink and Write ID give their own notice for it, as they do when the editor refuses.
  - **A BOM and the start of the file.** The driver plans a file's comment at line 0, character 0, because it has no notion of a BOM: right for an editor (VS Code's
    buffer has no mark), wrong for text read from disk, where U+FEFF is character 0 and the comment would land before it, leaving the mark at the start of line 2
    (a Python syntax error). So `Disk` moves a position at (0, 0) to (0, 1) when the text starts with U+FEFF (`past_the_bom`, in `edits.rs` only; `apply_text_edits`
    stays the plain rule). The driver is unchanged; planning after the mark there would make this unnecessary.
  - `write_atomic` now gives the new file the permissions of the one it replaces (a script keeps its executable bit): the files it writes into were never its own before. A symlink
    at the target is replaced by a plain file (rename), as for any atomic write.
  - The extension is unchanged: `Env.edits` is `EditSink::Editor(client)` there, with `apply_edits` moved word for word. `tests/write.rs` is the proof for `Disk`.

- **The file open and unsaved in an editor:** the editor will report a conflict when it saves. That is true of every edit an agent makes to a file;
  the guide says "work on saved files". Nothing more is done.
- `document_for` in `attach.rs` today refuses a def or a file with no open document (`MissingDocument`). With `Disk`, the caller builds the
  document from the file; the check stays for the editor.

## 3. `init`

`UserData::initialize(markers)`: writes `.vnoc/.gitignore` (`local/`) and `.vnoc/vnoc.toml` (`format = 1`; `[ids] default = "none"` with
`--markers off`). Does nothing if they exist, and says so. Prints the two paths.

It is its own command, not something `doc new` does on the way, **on purpose**: starting V-NOC puts `ID:` lines into the code from then on, and
that is the owner's decision (in the extension it is the first-attach question, `extension/src/addDoc.ts`). `doc new` in a project with no
`.vnoc/` exits `2` with: `this project has no .vnoc/ yet. Starting it adds ID: lines to code when a note is attached. Ask the user, then:
vnoc init`.

## 4. `doc new`

The extension's attach (`identity::attach`), unchanged in what it does: which ID (the one in the code, else the one this location has, else a new
one) → the marker → `node.toml` → the doc file.

| Argument | Does |
|---|---|
| `<node>` | one node: a function, class, file or folder (and `/`, the root, after A17) |
| `--title "…"` | required. Becomes the `# heading` and the file name (`slug`) |
| `--body-file <file>` or `-` | the Markdown after the heading, from a file or standard input. Without it the doc holds the heading only, and the agent writes into the path printed |
| `--no-marker` | attach by name; write no `ID:` line. Otherwise `vnoc.toml`'s `[ids] default` decides, as in the extension |
| `--again` | allow a second doc with a title this node already has |
| `--author <name>`, `--tag <t>` (repeatable) | the frontmatter's `author:` and `tags:` (C16) |

Guard rails in the command itself:

- **One node, one doc, per call.** No list of nodes, no standard-input list.
- **A title this node already has is refused** (exit `2`) with the existing doc's path: `this node already has "Why we parse twice" →
  .vnoc/…/why-we-parse-twice.md (edit that file, or --again)`. Today a second attach quietly makes `…-2.md`; that is right for a person who
  clicked twice on purpose and wrong for an agent that ran the command twice.
- A body over 256 KB is refused: a note is a note.
- The frontmatter is V-NOC's: `node:`, `hint:`, `author:`, `tags:`. A body that starts with its own `---` block is refused with a
  line saying to leave the frontmatter out.

The file:

```markdown
---
node: 0Q8V5W4T2F7KX9M3B6D1RZ4H7K
hint: src/orders.py::OrderService.parse_order
author: claude-code
tags: [plan]
---
# Split parse_order in two

…
```

`author` is whatever `--author` says; no flag, no line. The skill tells an agent to pass its own name. `tags` are lower-case words (letters,
digits, `-`); V-NOC gives none of them a meaning.

**Both keys are read wherever docs are fetched, not only by search** (C16):

| Where | How |
|---|---|
| the query (doc 20, 01 §2) | `tag:plan`, `author:claude-code`: in `find`, in the search box, in `tree --where` (nodes that have such a doc) |
| `doc list`, `doc read` | `--where 'tag:plan'`; `doc list` with no node lists the whole project's ([`03`](03-read-commands.md) §3) |
| `vnoc/docs` (the extension's request) | each doc's `tags` and `author` are in the answer (an added field), so the doc panel can show them and, later, filter by them |
| `doc list`'s rows, `show`'s doc lines | the tags after the title: `Split parse_order in two  [plan]` |

One reader does all of it: `userdata/read.rs` gets `frontmatter(text) -> { title, author, tags }` beside `title_of`, and the doc texts of doc
20 (03 §5) keep the two values with each doc. A doc written before these keys, or by hand without them, has neither: it is never an error.

**What changes in search for it** (settled 2026-11-17, from the built code; all in step A8):

- `DocText` (the core) and `DocEntry` (the crate) gain `tags: Vec<String>` and `author: Option<String>`. A doc is read again when its stamp
  moves, as now. The `DocIndex` reads titles and bodies only, so it and its key (the doc-text generation) do not change.
- `Query` gains `tags`, `not_tags`, `authors`, `not_authors` (lower case; `-tag:draft` excludes). Two `tag:` are both required, as two `in:`
  are; a value is never wrong, so these give no note.
- **On a doc:** it passes when it carries every `tag:` and the `author:`. **On a node:** it passes when at least one of its docs passes
  (one doc carrying all of them). `run` makes the set of such nodes once per query from the docs: no per-node scan.
- **A query of filters alone.** Today it lists nodes and no docs (20, 01 §3: every doc of every function under `kind:fn` would be noise).
  New rule: when the filters include `tag:` or `author:`, a query with no words and no phrases lists **the docs that pass**, in the reading
  order of their nodes and then by file name, with the start of the body as the snippet and no ranges; and the nodes that have one. Without
  one of the two, filters alone still find no doc. The row goes into 20's 01 §3 in the same commit.
- `matches` follows `run`, as built: the table that holds them together (`tests/matches.rs`) gets rows with tags.
- `SearchDoc` (the wire) gains `tags` and `author`; the sidebar's doc row shows the tags after the title, dim.
- **[decide]** *Add Doc* in the extension has no place to give a tag or an author. Until it has one, only the command and a hand edit of
  the frontmatter set them. A small step after A8 (an optional second input in the title prompt), or left for later: your call.

## 5. Editing a doc

No command. A doc is a Markdown file; `vnoc doc path` (or the path `doc new` printed) is all an agent needs, and its own edit tool does the rest.
The one rule, in the skill: **leave the lines between the two `---` as they are.** The watcher tells a running VS Code (`.vnoc/**`), and the next
search has the new text (doc 20, 03 §5).

Deleting a doc file is not something an agent is told how to do. A person deletes the `.md` (it is how the extension says to drop one doc), and
git shows it.

## 6. `relink` and `repair`

**`relink <ID> <node>`** is `identity::relink`: only for a node that is Missing; refused when the target has its own docs or is another kind
(the library's three refusals, with its words). It writes the ID at the new place if the project writes IDs, and the new place into `node.toml`.

**`repair`** is `identity::write_id`, for the case agents cause: with no arguments, every node `check` calls a **lost marker**
([`03`](03-read-commands.md) §9) gets its `ID:` line back; with nodes, those; `--all` also writes IDs for docs attached by name on purpose
(`id_in_code = "no"`), which is the extension's *Write ID*. A node set to never carry an ID is left alone (the library's rule). Each line of
output is one node and what happened.

## 7. Two writers

VS Code's core and a command can act on one project at the same moment. What already holds, from the storage rules:

- every write is temp-file-then-rename (`atomic.rs`): nobody reads half a file;
- the ID is a hash of the location, so two writers attaching to the same function want the **same** node folder; `create_node` leaves an existing
  one as it is;
- a doc file is made with `create_new`: a name that is taken becomes `-2`;
- the extension's watcher covers `.vnoc/**`, `.vnoc-id` and `*.py`, so the running core sees what a command wrote.

What is added (A7), because "one attach at a time" is today a mutex **inside** one process (`State.attaching`):

- **A lock file, `.vnoc/local/write.lock`**, held with `std::fs::File::lock` (in the standard library since Rust 1.89; the toolchain is 1.94)
  for the length of an attach, a relink or a write-ID, by the server and by the commands. A writer that cannot get it within 5 seconds stops with
  `another V-NOC is writing in this project` (exit `3` for a command). `local/` is git-ignored. **[check]** on Windows, by hand once.
- **Checked in A0 (2026-11-17, Rust 1.94.1, macOS; `oneshot lock <dir>`).** Two separate `File` handles on one `write.lock`, as two processes
  have them: the first `lock()` returns at once; while it is held, the second's `try_lock()` returns `Err(TryLockError::WouldBlock)` (its text:
  "lock acquisition failed because the operation would block"); after the first handle is dropped, the second `try_lock()` gets the lock.
  Any other failure is the other variant, `TryLockError::Error(io::Error)`. **`std` has no wait with a timeout:** `File` has `lock`,
  `lock_shared`, `try_lock`, `try_lock_shared` and `unlock`, and `lock_timeout` does not exist (the compiler says so). `lock()` alone waits
  for ever. So "within 5 seconds" is a loop: `try_lock`; on `WouldBlock` sleep about 50 ms and try again until 5 s have passed, then stop with
  the message above; on `Error(e)` stop at once (a failure to lock is not a busy lock; also exit `3`). The lock is held by the handle, so
  the guard in A7 is a value that owns the `File` and releases on `Drop`.
- **As built in A7:** `userdata/lock.rs`: `WriteLock::take(vnoc_root, wait)` (creates `local/`, `try_lock` every 50 ms until `wait`; `LockError::Busy` with the text above, `LockError::Io` at
  once), `WriteLock::take_if_initialized` (`Ok(None)` when there is no `.vnoc/`, and nothing is created), `UserData::write_lock()` (5 s, `DEFAULT_WAIT`). The guard owns the `File`.
  The path is in `layout.rs` (`write_lock_file`). **Blocking:** `take` sleeps, so the server calls it through `spawn_blocking`.
  - **The server** takes it in `vnoc/attach`, `vnoc/relink` and `vnoc/writeId`, **after** the in-process `attaching` mutex and held to the end of the handler. Order: mutex, then file lock; whoever
    holds the file lock is never waiting for the mutex, so the two cannot deadlock. A request waits on the mutex first for as long as another request in the same core takes, then up to 5 s on the file.
  - **Busy** answers `REQUEST_FAILED` (the request was fine; the project was not free) with `another V-NOC is writing in this project`; **Io** answers `INTERNAL_ERROR`
    with `could not lock .vnoc/local/write.lock: …`. The extension's catch shows it as an error message (`addDoc.ts`).
  - The editor sink waits for the editor's answer with the lock held: a command in another process that wants the project waits up to 5 s for it.

- It is taken **only in a project that is initialized**: the first attach in the extension creates `.vnoc/` itself and must not leave a
  `.vnoc/local/` behind if it fails early. Before `init` a command cannot write anyway (§3).
- **The order inside a flow is checked against the other writer acting between two steps** (futureplan 02.02 §2's `[check]`):

| Between | The other writer | Result |
|---|---|---|
| "which ID" and "create the node" | attached to the same node | the lock makes this impossible; without it, the same ID and one folder |
| the marker and `node.toml` | the user edited the file in VS Code | the marker write fails or is undone by the user: the doc is by name; `repair` later |
| a command's write and the server's next attach to that node | the server's index is a moment old | `attach` asks the disk (`find_node_at` reads `.vnoc/`), not its index, for an existing node: it reuses the ID and writes no second marker |

- A test races two commands (§A7): 20 rounds of two `doc new` on one function at once → one node folder, one `ID:` line, two docs.

## 8. What a write prints

```
$ vnoc doc new src/orders.py:142 --title "Why we parse twice"
attached to  function  src/orders.py::OrderService.parse_order   ID 0Q8V5W4T2F7KX9M3B6D1RZ4H7K
created      .vnoc/nodes/7K/0Q8V5W4T2F7KX9M3B6D1RZ4H7K/why-we-parse-twice.md      ← write the note here
changed      src/orders.py   (an ID: line was added at line 141; read the file again before you edit it)
```

Every file a write created or changed is one line. The last line is there for a reason: the agent's picture of `src/orders.py` is now one line
out of date, and an edit made from the old picture either fails or removes the line just added.

With `--json`: `{ "schema": 1, "command": "doc new", "node": {…}, "doc": "…", "created": […], "changed": […], "idInCode": true, "notice": null }`.
