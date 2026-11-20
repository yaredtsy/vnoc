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
  - `Document::from_disk(path)`: URI of the file, version `0`, the text decoded verbatim (line ends as they are; a leading BOM is dropped, see below); a file that is not UTF-8 is
    `InvalidData`. `document_for` returns a `MarkerDoc` (`Folder`, `Open(Cow<Document>)`, `Unreadable(reason)`): with `Disk` and no document it reads the file, and
    an unreadable one is `Unreadable`, which `write_marker` turns into the `Err` the flows already handle (by name, with the notice "…(the file is not UTF-8 text)…"). It is not an
    `AttachError`: Relink and Write ID give their own notice for it, as they do when the editor refuses.
  - **A BOM** (A7 first shifted a driver edit at (0, 0) past it; **replaced in A7b**, 2026-11-20, by the rule below: nothing is shifted any more).
  - `write_atomic` now gives the new file the permissions of the one it replaces (a script keeps its executable bit): the files it writes into were never its own before. A symlink
    at the target is replaced by a plain file (rename), as for any atomic write.
  - The extension is unchanged: `Env.edits` is `EditSink::Editor(client)` there, with `apply_edits` moved word for word. `tests/write.rs` is the proof for `Disk`.

- **The file open and unsaved in an editor:** the editor will report a conflict when it saves. That is true of every edit an agent makes to a file;
  the guide says "work on saved files". Nothing more is done.
- `document_for` in `attach.rs` today refuses a def or a file with no open document (`MissingDocument`). With `Disk`, the caller builds the
  document from the file; the check stays for the editor.

**The byte-order mark** *(settled 2026-11-20, review of A7).* A7 kept U+FEFF in the `Document` text read from disk and moved a driver edit at
(0, 0) past it. Replaced by the editor's own rule: VS Code's buffer has no mark, so **`Document::from_disk` drops a leading U+FEFF**, the driver
plans against the same text it gets from the editor, and the `Disk` sink re-reads the file, drops the mark for the comparison, applies the
edits, and **writes the mark back** if the file had one. No position is shifted and the driver is not touched; one rule for both sinks.
*As built (A7b):* `from_disk` and `apply_on_disk` share `split_bom`; a file that gained or lost its mark between the plan and the write is still the same text, and the
write puts back the mark the file has now. `apply_text_edits` is the plain rule, as before. `tests/write.rs` keeps its BOM cases (a file with the mark keeps it, the
`# FileID:` comment goes after it) and adds a def further down a file that has one.

## 3. `init`

`UserData::initialize(markers)`: writes `.vnoc/.gitignore` (`local/`) and `.vnoc/vnoc.toml` (`format = 1`; `[ids] default = "none"` with
`--markers off`). Does nothing if they exist, and says so. Prints the two paths.

It is its own command, not something `doc new` does on the way, **on purpose**: starting V-NOC puts `ID:` lines into the code from then on, and
that is the owner's decision (in the extension it is the first-attach question, `extension/src/addDoc.ts`). `doc new` in a project with no
`.vnoc/` exits `2` with: `this project has no .vnoc/ yet. Starting it adds ID: lines to code when a note is attached. Ask the user, then:
vnoc init`.

**As built in A8a.** `cli/write.rs` `init`; no driver is looked for (`open_without_driver`). Text, on a new project:

```
created      .vnoc/.gitignore
created      .vnoc/vnoc.toml
ids          attaching a note writes an ID: line into the code
```

(with `--markers off` the `ids` row says `no ID: lines are written; notes are attached by name`; `--quiet` leaves it out). Already started: `already      .vnoc/ is already set up`, `in           .vnoc/vnoc.toml`
and the `ids` row, exit 0, nothing written; **the first answer stays**, so `init --markers off` on a project that writes IDs
changes nothing and the line says what the project does. `--json`: `{ "schema": 1, "command": "init", "created": […], "already": false, "markers": true }`
(`created` is empty and `already` true for a project that was started). Paths are as every command prints a path: relative to the root, absolute when `.vnoc/`
is above it. `UserData::init_files()` names the two files (the layout stays in `layout.rs`). Note: `initialize` writes `[ids] default = "none"` as the
inline table `ids = { default = "none" }`, which is the same TOML; it was that way before A8a.

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
- *(Added 2026-11-20, A12d.)* **A body that starts with the title is not given the title twice.** If its first non-blank line is a `# `
  heading equal to `--title` (case and runs of spaces ignored), that line and the blank lines after it are left out, and the output adds
  `             the body's own "# <title>" line was left out: V-NOC writes the title`. A different first heading is the author's and stays.
  Seen on the user's project: an agent's overview note had `# Knowledge base API: overview` twice.
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
- *Add Doc* in the extension has no place to give a tag or an author. **Decided 2026-11-20: left for later**, after release 0.5: only the
  command and a hand edit of the frontmatter set them. The sidebar still shows and finds them (A8b). Not on the CLI's path.

**As built in A8a** (`cli/write.rs`, `identity/attach.rs`, `userdata/`).

- **The flow in the command:** the project is opened without a driver and checked for `.vnoc/` first (so nothing is read from standard input in a project nobody
  started), then the body is read (a file, or `-`; reading stops one byte past 256 KB), then `check_note` (the library's rules, so a refusal does not start a
  driver), then the index is built, the node resolved (one argument; a second is clap's usage error), the **write lock taken**, and `attach` run with `EditSink::Disk`.
  The index is a moment old by then when another writer got the lock first; `attach` asks the disk which ID a node has (04 §7), and the race test below proves it.
- **The library's options:** `attach(env, params, options) -> Attached`. `AttachOptions { body, author, tags, again, no_marker }`; the server passes
  `AttachOptions::editor()` (`again: true`, the rest empty), so the extension still makes `-2.md` for a second same-title attach (the new `lifecycle.rs` test pins it;
  neither lifecycle nor the window suite did before). `Attached { result: AttachResult, marker: Option<MarkerWritten> }`, where `MarkerWritten` is `Line(n)` (1-based, from the plan's
  `written` span) or `FolderFile`; `None` when the code was not touched. `AttachResult`, the wire, is unchanged.
- **The refusals, all in `attach.rs`, asked before anything is read or written** (`check_note`, public; `TitleTaken` after the node's ID is known): `BadTitle` (empty, or any control
  character, so one line), `BadAuthor` (the same), `BadTag` (not lower-case ASCII letters, digits and `-`; an upper-case tag is refused, not lower-cased), `BodyTooLarge`
  (over `MAX_BODY_BYTES`, 256 × 1024; exactly that is fine), `BodyHasFrontmatter` (the body, leading white space aside, begins with a `---` line **and** has a closing
  `---` line: a lone `---` is a Markdown rule and is allowed), `TitleTaken { title, path }` (a doc of the node has the same title **ignoring case and the space around it**; the path is that
  doc's file; refused only without `again`). A command's exit: all of these `2`; `NotInIndex` `2`; a lock that cannot be had, a `vnoc.toml` too new, anything else `3`.
- **The sentence for a title taken:** `this node already has "Why we parse twice" → .vnoc/nodes/…/why-we-parse-twice.md (edit that file, or --again)`: the path as every command prints a path.
- **`--no-marker`** is `AttachOptions::no_marker`, which turns the project's answer off for that attach and nothing else (a node that has its ID in the code keeps it;
  a node found by name is not retried: that is `repair`, A9). With `[ids] default = "none"` the same: no `ID:` line, no `changed` line, no notice (nothing failed).
- **A function in a file with a syntax error:** a def that parses gets its `ID:` line as in any file; the def with the error is attached by name, exit 0, with the notice
  `The ID could not be written (syntax error inside this def); this doc is attached by name.` and the file untouched.
- **A file that is not UTF-8 is not a node** for any command: the driver cannot read it, so it is not in the index, and `doc new` is exit `2` with `latin.py could not be read: …`
  (as `show`), nothing written. The "by name, with the notice" path for an unreadable file is only reached when a file stops being UTF-8 *between* the index and the write; the library test
  `tests/write.rs` pins that.
- **The doc:** `create_doc(id, &NewDoc { title, hint, body, author, tags })`. The frontmatter is `node:`, `hint:`, `author:` and `tags: [a, b]` only when given; then `# <title>`, a blank
  line, and the body (a last line break is added if it has none). Tags are written in the order given; **one given twice is written once** (A8a2, in `attach`: the first place is kept).
- **Reading:** `userdata::frontmatter(text) -> Frontmatter { title, author, tags }`. `author: x` (one pair of quotes is taken off); `tags: [a, b]` on one line (brackets optional, **lower-cased on reading**,
  empty entries dropped) or, **since A8a2**, the block form a person writes by hand (`tags:` then `- plan` lines, indented or not, quotes allowed, blank lines between them do not end it; `tags: plan` is one
  tag); the first line of a key wins; a key in the body, an unclosed block, or a block with nothing the reader knows reads as none. `DocInfo` and `DocText` keep the two; `DocText`'s feed the search (A8b).
- **`show` and `doc list`** print `  <title>  [tags]  <path>`, the tags dim, in the order written, only for a doc that has some (`Doc::row`). `--json` docs gain `tags` and `author`, **only when there
  are any** (so every earlier golden is as it was).

**As built in A8b** (2026-11-20; the spec above, with what the code settled).

- **The crate** (`vnoc-search`): `DocEntry { …, tags, author }` and `Query { tags, not_tags, authors, not_authors }` as in the list above. `Query::has_doc_filters()` (public) is "the query has a `tag:` or an
  `author:`" and `has_filters()` includes it. `run.rs` makes, once per run, the set of nodes that have a doc that passes (`Filters::with_such_doc`); a node passes when it is in the set **and** passes the node filters;
  a doc passes when it has what is asked of the doc itself **and** its node passes the node filters (`passes_doc`). The doc index and its key are untouched; no dependency was added and the wasm build passes.
  Both sides of `matches` use the same two functions, and `tests/matches.rs` has the rows (23 queries × 4 values of `exact`, with each node's `docs` count as the core builds it).
- **A query of filters alone** lists docs when it has a `tag:` or `author:` (`docs_by_filters_alone`): `words` empty, no phrase, a doc filter. The rows are the docs that pass, in their nodes' reading order, then
  by file; the snippet is the start of the body, no ranges, score `0`; `docs_total` counts them all, and `limits.docs` cuts. The old rule stands without one of the two (`kind:fn` alone lists no doc).
- **The core:** `corpus_of` copies `tags` and `author` from `DocText`; `SearchDoc` and `DocInfo` gain `tags: Vec<String>` and `author: Option<String>` (`#[serde(default)]`; always written, `[]` and `null` for a doc
  with neither). A hand edit of a doc's tags is a change of its stamp, so `DocTexts::refresh` reads it again and the next search sees it: **no `indexChanged`, the revision does not move** (a lifecycle test pins both).
- **The sidebar:** a doc row in the search results shows `[plan, billing]` after the title, in the row's dim colour and a little smaller (`tagsLabel` in `webview/src/sidebar/search.ts`, drawn in
  `SearchResults.tsx`); a doc with none has no element, so its row is as it was. `shared/messages.ts` has `tags?` and `author?` on `SearchDoc` and `DocInfo` as optional fields (a core older than A8b sends neither).
- **`find`:** a doc row prints `  <title>  [tags]  on <node>` (dim tags) and `--json` doc rows gain `tags` and `author`, only when there are some (the earlier goldens are unchanged).
  `find 'tag:plan'`, `find 'kind:fn tag:plan'` and `tree --where 'tag:plan'` needed no other change: they go through the crate.
- **`doc list --where` and `doc read --where`** (`cli/doc.rs`, `Narrow`): the query is parsed by `vnoc_search::parse`, and `run` is asked with `Limits { nodes: 0, docs: ∞ }`; a doc is kept when it is among
  the doc rows (matched by its file's path), and the docs of a node are put in `run`'s order. Notes on standard error, then the answer anyway: a value that means nothing gives the note `find` gives and nothing is
  found (`kind:fnn`); `has:doc` and `no:doc` give ``note: `has:doc` and `no:doc` are about nodes, not docs: ignored here`` and are dropped from the query; a query with nothing about a doc (`kind:fn` alone) gives
  `note: nothing in this query is about a doc: …` and finds none. `kind:` and `in:` narrow by the doc's node. A blank `--where` is exit `2`. An empty result says `no docs match` (`no docs on <node> match` for
  `doc read`); a node that had docs and has none left prints `  no docs match`. **`--where` needs the index, so a node named by its ID starts the driver too.** With a file named (`doc read <node> <file>`), the file is
  chosen first, then the query narrows what is left. `doc path` has no `--where`.

### Listing, editing and describing tags *(settled 2026-11-20)*

**Where tags live.** Only in each doc's frontmatter. There is no list of tags to keep in step: the list is **computed from the docs** every
time. The doc texts the core reads on every run (doc 20; 17 ms for 1,000 docs, A0) already carry each doc's `tags` and `author` since A8a,
so listing them is a count over memory, not a second read.

**`vnoc tags [--authors]`** *(renamed 2026-11-20 from `vnoc doc tags`, built so in A9b: `vnoc doc tag` was a prefix of it, so a permission rule
that asks before the write would also catch the read; 01 §3)*: every tag with how many docs carry it, and its description from `.vnoc/tags.toml` when it has one;
described tags no doc carries yet are listed with `0 docs`. `--authors` lists authors the same way. Needs **no driver** (tags are not about
code): `.vnoc/` and the doc texts only, about 60 ms on 1,000 docs by A0's numbers. Sorted by count, then by name. JSON:
`{schema, command: "tags", tags: [{tag, docs, description}], authors?: [{author, docs}]}`.

```
$ vnoc tags
plan        4 docs   A change we intend to make. Retag as done when it ships.
billing     2 docs
decision    1 doc    Why we chose this way over the others.
done        0 docs   A plan that shipped.
```

**A tag nobody has used yet is accepted, never an error.** There is nothing to register first: once a doc carries it, it exists. Because of
drift (`plan`, `planning`, `todo`), `doc new` and `doc tag --add` print a **notice** (stdout, exit `0`) when a tag is on no other doc and is not
described: `note: "planning" is a new tag in this project (described: plan, decision, done); did you mean "plan"?`. The "did you mean" is
`nearest` over the known tags (described ones first), with the node names' edit rule. `--quiet` keeps the notice: it is about the data, not
a hint.

**`vnoc doc tag <node> [file] [--add <t>]… [--remove <t>]…`**: one doc per call (C3: no bulk), the doc chosen as `doc read` chooses it
(the node's only doc, or `file`; a node with several docs and no `file` → `2` with their file names). It rewrites **only the `tags:` line**:

```
before                          after `--add done --remove plan`
---                             ---
node: 4KF0…                     node: 4KF0…
hint: billing/tax.py::rate_for  hint: billing/tax.py::rate_for
tags: [plan, billing]           tags: [billing, done]
---                             ---
```

- The `node:` and `hint:` lines, the title and the body stay byte for byte; the write is atomic (temp file, then rename) under the write
  lock (A7), and refused if the file changed between read and write.
- Order: the doc's tags as they were, removals taken out, additions appended in the order given; a tag already there is not added twice.
- The last tag removed: the `tags:` line goes. No `tags:` line and an `--add`: the line is written after `author:` (or `hint:`).
- A hand-written block list (`tags:` then `- plan` lines) is rewritten as one line: the only formatting change, and only of those lines.
- Removing a tag the doc does not have, or adding one it has: nothing changes for that tag, the output says `already without "plan"` /
  `already has "done"`, exit `0`. Running the same command twice is never an error.
- A bad tag (upper case, a space…) → `2` with the rule, nothing written.
- Output: `tags  <doc path>  [billing, done]`, plus the new-tag notice if any. JSON: `{schema, command: "doc tag", doc, tags, added,
  removed, unchanged, notice}`.

**`vnoc tag describe <tag> "<text>"` / `vnoc tag describe <tag> --clear`** *(added 2026-11-20, A11c: the user found no way to describe a
tag; hand-editing a file nobody is told about is not a way)*: writes one key of `.vnoc/tags.toml`, creating the file if missing, with
`toml_edit` so a person's comments, order and other keys stay as they are; a new key is appended. Atomic, under the write lock, refused in a
project with no `.vnoc/`. A bad tag → `2`. A description is one line (a line break → `2`). Output `described  plan  "<text>"` and `in
.vnoc/tags.toml`; `--clear` removes the key (`not described` when it had none, exit `0`). It is a **person's** command: the skill tells agents
not to run it unless the user asked, and `vnoc tag` is in the write list (01 §3). `vnoc tags` ends with a dim hint when no tag has a
description: `→ describe one: vnoc tag describe <tag> "<what it means>"` (`--quiet` drops it).

**What is not built:** renaming a tag across the project (a bulk write: C3); a whitelist (a refused tag would stop an agent mid-task for a word). If a team asks for strict
tags, an optional `strict = true` in `tags.toml` is where it would go.

**In the skill (A10):** before tagging, run `vnoc tags` and reuse a tag that fits, preferring described ones; never invent a synonym of
one that exists; move a plan's tag when its state changes (`vnoc doc tag … --add done --remove plan`); read `tags.toml`, do not change it (`vnoc tag describe` is the user's).

**Later, not in 0.5:** the extension shows descriptions on hover of a tag in the sidebar's doc rows, and gets a `vnoc/tags` request (a thin
wrapper over the same library function, C2) when it has a screen that lists or edits tags.

**As built in A9b** (2026-11-20; `userdata/tags.rs`, `cli/tags.rs`).

- **`tags.toml`** is read by `userdata::tags::read_descriptions(root)` into `Descriptions` (tag → text, trimmed, sorted by tag). Not there: nothing and no problem. A key that is not a tag
  (`Bad_Key`), a value that is not a string (a number, a table), each: **one problem**, skipped, the rest read; a file that is not TOML: one problem and nothing read. The problems are appended to
  `load_all`'s report, so `check` lists them as `broken` with the file, and `status` counts them. `UserData::tag_descriptions()` is the descriptions alone. V-NOC never writes the file.
- **`tags [--authors]`** reads `.vnoc/` and the doc texts only (`--driver /nonexistent` works; a project nobody started says `no tags`). `tag_counts`: a tag counts once for a doc; described tags no doc
  has are listed with `0`; **most docs first, then by name**. `author_counts`: authors that differ only in case are one, shown as first met. Text (columns: name, count, description; the count is padded only when a
  description follows):
  ```
  billing   2 docs
  plan      2 docs  A change we intend to make. Retag as done when it ships.
  decision  1 doc   Why we chose this way over the others.
  done      0 docs  A plan that shipped.
  ```
  `--authors` prints the authors instead (`no authors` when none); `--json`: `{schema, command: "tags", tags: [{tag, docs, description}], authors?: [{author, docs}]}` (`authors` only with `--authors`,
  and then `tags` is there too).
- **`rewrite_tags(text, add, remove) -> Result<(String, Change), TagError>`** is pure and is 04 §4's list, with these settled: the doc's tags are read as `frontmatter` reads them (**so an upper-case or quoted hand-written
  tag comes out lower case and unquoted** when the line is rewritten); removals are taken first, then additions are appended, so `--add x --remove x` on a doc that has `x` moves it to the end; a tag to add must be
  a tag, **a tag to remove must be one unless the doc has it** (so a hand-written `café` can still be taken off); a doc with no frontmatter (no `---` line at its top, or none closing it with a line break) is
  `TagError::NoFrontmatter`, exit 2: no frontmatter is made for it. The new line takes the line end of the line it replaces or follows (CRLF stays CRLF). When nothing changes the text comes back
  untouched (a hand-written block list stays a block list until a tag is really added or removed). Only the **first** `tags:` line is read and rewritten, as the reader has it.
- **`doc tag <node> [file] --add … --remove …`:** at least one of them (else exit 2); a tag to add is checked first (`` `Plan` is not a tag: lower-case letters, digits and `-` ``, 2, nothing written). The doc is chosen
  as `doc read` chooses it, by `doc::choose_doc` (the node's only doc, or `file` with or without `.md`; **several docs and no file is `2`** with `<node> has 2 docs: name one of a.md · b.md (the file name goes after the
  node)`; none: `no docs on <node>`). A node named by its ID needs no driver. The write lock is taken, then `UserData::edit_doc_tags` reads the file, rewrites, **reads it again and compares** (`write_if_unchanged`:
  refused with `the file changed while the tags were being written` if it moved, the file as the other writer left it) and writes atomically; nothing is written when nothing changes. A file that changed under the
  command is exit **3** (like a busy lock: try again); the other refusals are 2. Output (A9c): `tagged       [billing, done]`, `in           <doc path>`, then `already has "done"` / `already without "plan"` lines, then
  the new-tag notes. `--json`: `{schema, command: "doc tag", doc, tags, added, removed, unchanged: [{tag, state: "has"|"without"}], notice}` (`notice` is the notes joined by a line break, or `null`).
- **The new-tag note** (`cli/tags.rs` `new_tag_notices`): for each tag a command added that **no other doc** carries (the doc itself is left out of the counts) and `tags.toml` does not describe:
  `note: "planning" is a new tag in this project (described: decision, done, plan); did you mean "plan"?`. The parenthesis lists every described tag, **sorted by name** (not in the file's order), and is left out when
  `tags.toml` describes none; the question is left out when nothing is near. It is on standard output, exit `0`, kept under `--quiet`. In `doc new` the same lines are the `tag_notices` field of `--json`
  (left out when empty), and `tests/golden/doc-new.json` was blessed for it (its `--tag plan` was the first in that project).
- **"Near"** is `vnoc_search::nearest_word(known, tag)`, **not quite the node names' rule**: the edit rule without the length floor (1 edit up to 4 letters, 2 from 5) **or** a word that starts the other (3 letters or
  more in the shorter), counted as that many edits as the lengths differ. The example of this section, `planning` for `plan`, is 4 edits away, so the edit rule alone would never suggest it. `known` is the described tags first,
  then the used ones, most used first; the closest wins and a tie goes to the first (so to a described tag). A tag that equals a known one is never "near" itself.
- Not built, as written above: renaming a tag across the project, a command that writes `tags.toml`, a whitelist.

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

**As built in A9** (2026-11-20; `cli/relink.rs`, `identity/{relink,write_id}.rs`).

- `identity::relink` and `identity::write_id` return `Done<T>` (`result`, and `marker: Option<MarkerWritten>`, as `attach` does); the server keeps using `.result`.
- **`relink <ID> <node>`:** the ID is parsed first (`` `x` is not a node ID ``, exit 2), an ID no `node.toml` has is `no node has the ID …` (2), then the index is built, the target resolved
  (a Missing target is `relink to code that is there: … is Missing too`, 2), the **write lock** taken, and `identity::relink` run with `EditSink::Disk`. Each refusal of the library is exit `2` with its
  words: *that node is not Missing: its docs are shown where its code is*, *that node already has its own docs* (for a place with an `ID:` in the code and for one the index knows a node at by name),
  *this is a class, and the docs belong to a function* (the library's *no readable node.toml* is not reachable here: the command's own check says `no node has the ID …` first). Output (labels 13 wide, as 04 §8):
  ```
  relinked     7N1HNCPA163HHSN91HX48T9QGE
  was          api/routes.py::legacy_export
  to           function  api/routes.py::get_order
  changed      api/routes.py
               an ID: line was added at line 5; read the file again before you edit it
  ```
  A project that writes no IDs, or a marker that could not be written: no `changed` line, and the notice (`notice       The ID could not be written (…); the docs are attached by name.`); a
  folder: `created      <folder>/.vnoc-id`; a place that already carries this ID: nothing changed, and the line is left out. `--json`: `{schema, command: "relink", id, was, node: {kind, location, node_id},
  created, changed, id_in_code, notice}` (`id_in_code` is whether the ID is in the code at the new place).
- **`repair [<node>…] [--all]`:** the work is `identity::write_id`. With no argument, `Index::lost_markers()` (a `node.toml` that says `id_in_code = "yes"` and a node found by its name only). `--all` adds
  `Index::attached_by_name()` (the records that say `"no"`: attached by name on purpose, or whose marker could not be written; the extension's Write ID). `--all` with nodes is a usage error. With nodes, those
  (`-` reads names from standard input); an argument that names nothing, or a Missing node, is an error in its own place (`doc`-style hints) and exit `2`.
- **One write lock for the whole run; the nodes are written one at a time, each flow reading its file again just before it plans** (`Document::from_disk` per call; the Disk sink re-reads and compares). The
  text from before the first edit is never reused. **A file's nodes are written from the top** (sorted by file, then the line the def starts at; a file or a folder is line 0): an `ID:` line put in
  above the others shifts them down, so doing the upper one first makes every line reported the line it stays at. The rows are printed in that order.
- **Output (A9c):** `repaired     <location>` with `ID: line written at line N` on the line under it (a file: `# FileID: comment written at line N`; a folder: `.vnoc-id written`), or `left         <location>` with the reason under it, in the library's
  words (*this node was set to never have an ID in the code*, *this project does not write IDs …*, *the ID of that node is already in the code*, *that node has no docs, so there is no ID to write*, or the notice
  when the marker could not be written). Nothing to do: `nothing to repair`, exit 0 (also in a project nobody started). **Exit:** `2` when a node the user NAMED was left (the `left` line is on standard
  output, in its place); a sweep that leaves some is `0`. `--json`: `{schema, command: "repair", repaired: [{location, node_id, line}], left: [{location, node_id, reason}], errors?: […]}`.
- The three "not built yet" texts (`missing --help`, `check --help`, `doc new` on a Missing node) now name the commands.

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
created      .vnoc/nodes/7K/0Q8V5W4T2F7KX9M3B6D1RZ4H7K/why-we-parse-twice.md
             ← write the note here
changed      src/orders.py
             an ID: line was added at line 141; read the file again before you edit it
```

Every file a write created or changed is one line. The last line is there for a reason: the agent's picture of `src/orders.py` is now one line
out of date, and an edit made from the old picture either fails or removes the line just added.

With `--json`: `{ "schema": 1, "command": "doc new", "node": {…}, "doc": "…", "created": […], "changed": […], "id_in_code": true, "notice": null }`
(snake_case, as every other answer: 03 §11; the first version wrote `idInCode`).

**As built in A8a.** The labels are one column, 13 wide (`attached to  `, `created      `, `changed      `, `notice       `), dim in a terminal.

- `attached to  <kind>  <location>   ID <id>`: the head line every command uses.
- `created      <doc path>` and, **only when no `--body-file` was given**, `← write the note here` **on the line under it** (A9c: a path and an arrow on one row wrap; a hint: `--quiet` leaves it out). For a folder, a second `created      <folder>/.vnoc-id`.
- `changed      <file>` and under it `an ID: line was added at line N; read the file again before you edit it`: the line is the plan's, 1-based; for a file the words are `a # FileID: comment was added at line N`.
  `--quiet` keeps that line: it is a warning, not a hint.
- `notice       <the notice>` when the ID could not be written (a def with a syntax error, a file that stopped being UTF-8): the doc is attached by name, exit 0, and there is no `changed` line.
- A command that was refused prints `error: …` on standard error and nothing on standard output; with `--json` an error about the *node* is `{ "schema": 1, "command": "doc new", "error": { "why", "message", "hints"? } }` on
  standard output (as `calls`), exit 2, and a refusal of a rule (title taken, body, tag) is the plain `error:` line.
- `--json` is exactly the object above: `node` is `{ kind, location, node_id }`; `created` lists the doc and a folder's `.vnoc-id`; `changed` lists the code file with an `ID:` line added, and is empty otherwise.
  `tests/golden/doc-new.json` pins it.

**A8a's tests** (`tests/cli.rs`, `tests/lifecycle.rs`): the race is 20 rounds of two `doc new` processes started together on one function with different titles, each round on a fresh started
copy, → one node folder, one `ID:` line, two docs, and `vnoc check` clean; a second test holds `.vnoc/local/write.lock` from the test and shows `doc new` exits `3` with the message after 5 seconds and writes nothing.

**As built in A9 and A9b.** `relink`, `repair`, `doc tag` and `tags` print as above (§6, §4), in the same style: labels 13 wide (`relinked`, `to`, `repaired`, `left`, `tags`, `changed`, `created`, `notice`),
dim in a terminal; `already has "x"` / `already without "x"` and the `note:` lines are whole lines with no label. Every file a write changed is a `changed` line, and the warning in it
(*read the file again before you edit it*) stays under `--quiet`; the new-tag `note:` stays too (it is about the data). A command that was refused prints `error: …` on standard error and nothing on standard
output, **except** `repair`, whose `left` line for a node the user named is on standard output in its place, with the exit `2`.
