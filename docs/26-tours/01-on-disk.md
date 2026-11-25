# 26.01 On disk

```
on disk
├── 1. the folders                one file per tour · private and project · this machine's state
├── 2. a tour is a log            why lines, why added; what is taken from Claude Code and Codex, what is not
├── 3. the structure              stops in stops, blocks in blocks; the reading order
├── 4. the records                tour · meta · plan · stop · step · drop · ids · pin · halt · halted · resume · end
├── 5. ids                        the tour, a stop, a block
├── 6. how a stop names its node  the name, the ID if there is one; writing IDs when asked
├── 7. the hashes                 what is hashed, what is left out
├── 8. the pin                    a commit only when it is true · committing the tour's files · always playable · never a checkout
├── 9. the state file             this machine's: the reader's place, the last check
├── 10. rules                     untrusted input, limits, line ends, a broken line
├── 11. what it adds to the frozen format
└── 12. the settings              [tours] in vnoc.toml: ids, commit
```

## 1. The folders

```
.vnoc/
├── vnoc.toml                          + an optional [tours] table (§12)
├── .gitignore                         local/
├── nodes/…                            (touched only by `vnoc tour ids`: §6)
├── tours/                             NEW · committed · the project's tours
│   └── checkout-order-7k2q.jsonl      one tour
└── local/                             not committed (as today)
    ├── write.lock
    ├── trash/                         a deleted tour, and its state, go here
    ├── tours.lock                     NEW · held for the length of one write
    ├── tours/                         NEW · this machine's tours: every tour starts here
    │   └── reserve-stock-m3xd.jsonl
    └── tour-state/                    NEW · this machine's state of each tour, private or project (§9)
        ├── checkout-order-7k2q.json   where the reader is · the last check
        └── reserve-stock-m3xd.json
```

- **One tour is one file.** Never several tours in a file. The folders are the list: every `*.jsonl` file in `tours/` or
  `local/tours/` is a tour. No index file.
- **Private** (`local/tours/`): written by an agent for the person at this machine. Not in git, not in a pull request.
- **Project** (`tours/`): someone shared it. The file is **moved**, with the same name and the same bytes. It is committed and
  reviewed like a doc.
- **State** (`local/tour-state/`): what is true of this machine and not of the tour: where the reader is, and what the last check
  found. Never committed, for a project tour either.
- A tour belongs to no node. It passes through many, so it cannot live in a node folder as a doc or a playground does.
- The same id in both folders (someone pulled a tour they also have privately): the project's one is listed, the private one is listed
  under it as `(private copy)`. Nothing is merged.

## 2. A tour is a log

The first line says what the tour is. Every later line is one thing that happened: a stop was written, a block was added, the reader
pressed Stop, the agent finished. **The step-by-step commands only add lines.** To read a tour, replay its lines from the top
([`03`](03-core.md) §1).

| Need | A log gives it |
|---|---|
| the agent adds a stop, reads some more code, adds the next | each command adds a few lines. There is no "whole tour" to send, and no draft to hold somewhere else |
| you read while it is written | the file grows; the reader replays it again when the watcher fires. A half-written tour is a valid tour |
| two writers: the agent adds stops, your window writes Stop (and, in 27, your question) | both add lines. Neither reads, changes and writes back the whole file, so neither can undo the other |
| "readers ignore what they do not know; writers keep it" (`04-storage/01` §5 rule 1) | a kind of line this version does not know is skipped when read and is not touched by an append |
| the interactive tour | a question, an answer and a side trip are more kinds of line, each pointing at the step it hangs from |
| fixing one stop of an old tour | a later line with the same id wins |

**The one command that does not add** is `import --replace` (D20): a person replaces the whole tour with a document they edited. It
writes a new file and renames it over the old one (`write_atomic`), under the same lock, and the old file goes to the trash.

**What is taken from how Claude Code and Codex keep a conversation, and what is not.** Both keep one `.jsonl` file per session, one
event per line, each with a type, and both resume a session by replaying its file. Codex starts the file with a line that describes the
session. Claude Code gives every line an id and the id of the line it follows, so a side branch is a line whose parent is in the middle.
(From what both write on this machine; neither format is a promise of theirs.) That shape is exactly this problem, so the shape is
taken: **one file per tour, a first line that describes it, one typed record per line, ids, a parent, replay.** Their fields are not
taken: those describe model messages and tool calls, their ids are long random strings nobody types, and their files are private to one
machine. A tour's records describe stops and blocks, its ids are typed by an agent (`s3`), and its file may be committed and read in a
pull request.

**Why not TOML and Markdown, as docs are.** A doc is edited by people in an editor. A tour's text is checked against the code when it
is written (the lines exist, the text fits its budget), so it goes through a command; a person who wants a file to edit gets one from
`export` and gives it back with `import` ([`02`](02-commands.md) §10), and it is checked on the way in.

## 3. The structure (D18)

A tour is a tree with two kinds of thing.

```
tour                                                 title · question
└── STOP    highlights one node: a folder · a file · a class · a function         "what this is for"
    ├── BLOCK    some lines of that node                                          the general: "what this part does, and why"
    │   └── BLOCK    lines inside its parent's                                    the detail
    ├── BLOCK
    └── STOP    a node reached from this one                                      a call it makes · a file of the folder · a method of the class
        ├── BLOCK
        └── STOP …
```

| Rule | |
|---|---|
| **Every stop highlights a node.** Any kind | the map draws it as that node's card; the stage shows its code, or its contents for a folder |
| **A stop has an intro:** one paragraph on what the node is for | it is the stop's first step, with nothing lit |
| **A stop on a file, a class or a function may have blocks.** A folder has none | a block is `[first line, last line]`, a title, one paragraph |
| **A block may have blocks.** A child's lines are inside its parent's; children of one parent do not overlap; neither do the blocks at the top of a stop | first the general (lines 11 to 24: "hold the stock, then charge"), then the detail (11 to 16, 18 to 24). Two levels are designed for; the file's format does not limit the depth, the command does ([`02`](02-commands.md) §4) |
| **A stop may hold stops:** `from` says which stop it was reached from, and `line` where in that stop's node the call is | a call made from it (with `line`); a file of the folder, a function of the file, a method of the class (no `line`) |
| **A stop with no `from` is at the top** | a tour need not be one tree: "from a request to an order" can be five top-level stops |
| **A stop under 8 lines has no blocks** (W9) | its intro is its only step, with the whole node lit |
| **A stop on a node the tour already explained** says `same_as`, has one sentence and no blocks | the design's *explained before* |

**The reading order is the tree's own**, and `Next` follows it:

```
stop s1:  intro  →  block b1  →  its details b2, b3  →  block b4  →  …        every step of the stop, the general before its details
then the next stop on the route                                              (s1's first nested stop, if it has one; else the stop after it)
```

The route (the order of the stops) is the order in which they were planned or added, with one rule: a nested stop comes after the stop
it is nested in. A **step** is what the stage shows at one moment: a stop's intro, or one block.

**A planned stop that is written** (decided at TR10's review, built in TR11): a `stop` record whose id is a planned stop keeps the
planned stop's place when its `from` is the plan's; with another `from` (or one where the plan had none), the stop moves, with the
stops nested in it, to where a new stop with that `from` goes. A stop already written that is replaced (`--replace`) stays where it
stands, as before.

**How V-NOC stores the tree: flat, with id chains.** Never nested arrays (`children: [ { children: [ … ] } ]`). The nested form
exists in one place only, outside the store: the document a person exports, edits and imports ([`02`](02-commands.md) §10), which is
translated to this on the way in and from this on the way out.

```
stops    a list, in route order.        Each stop:   id · from (the stop it is nested in, or none)
blocks   a list per stop, reading order. Each block:  id · parent (the block it is a detail of, or none)
```

The picture above, flat: `s1` · `s2 from s1` · `s3 from s1` · `s4 from s3`, and for `s1`: `b1` · `b2 parent b1` · `b3 parent b1` · `b4`.
The one containment is that a block belongs to its stop (its id says so: `s1.b2`); a block never moves to another stop.

| | Flat, with id chains | Nested arrays |
|---|---|---|
| the tour's file is a log: things are added one at a time | a new stop is one line that names its `from` | a child cannot be added to an array that was already written |
| replacing or dropping one stop | names one id. What is nested in it is found by the chain | rewrites the whole subtree, or needs a path (`stops[0].stops[2]`) that changes when a sibling is added |
| the reader's place, a question in 27, "explained at stop 2" | all point at an id | would point at a path |
| the screen | the route is rows with an indent; the stage walks a list; `Next` is "the next item" | every reader flattens it first |
| moving a stop under another | change one `from` | cut and paste a subtree |
| seeing the shape when you read it | less direct: the order and `from` say it. So a person never has to read this form: `vnoc tour show` draws the tree, and `vnoc tour export` gives the nested document | direct |
| a chain that points at nothing (a hand edit, a merge) | possible: such a stop or block is put at the top (§10) | not possible |

The flat shape is what V-NOC keeps and passes around: the records of §4, and what the pages get ([`03`](03-core.md) §8). The order
in a list is the order of reading; a `from` or `parent` names something **earlier** in the list. The nested document is a view of
it, made by one function each way (`tours/doc.rs`).

## 4. The records

One JSON object per line, UTF-8, ended by `\n`. Every record has `type`. Minicart's tour (the design's fixture), shortened:

```jsonl
{"type":"tour","schema":1,"id":"checkout-order-7k2q","title":"checkout_order","question":"what happens when the payment times out","author":"claude-code","at":"2026-11-23T18:40:02Z","detail":"normal"}
{"type":"plan","stops":[{"id":"s1","path":"app/checkout.py","qualname":"checkout_order"},{"id":"s2","path":"app/inventory.py","qualname":"check_stock","from":"s1","line":13},{"id":"s3","path":"app/inventory.py","qualname":"reserve_stock","from":"s1","line":16}]}
{"type":"stop","id":"s1","path":"app/checkout.py","qualname":"checkout_order","kind":"function","node":"0Q8V5W4T2F7KX9M3B6D1RZ4H7K","lines":[11,30],"hash":"9f2c61a07b3e4d15","text":"checkout_order turns an order request into a completed order: it holds the stock, takes the money, and records the sale, in that order."}
{"type":"step","id":"s1.b1","stop":"s1","lines":[11,24],"hash":"41c07d92aa10b6e3","title":"Hold the stock, then charge","text":"Nothing is charged until the items are set aside, and nothing stays set aside if the charge fails."}
{"type":"step","id":"s1.b2","stop":"s1","parent":"s1.b1","lines":[11,16],"hash":"7be1a0c4d95f2233","title":"Check the stock, then hold it","text":"check_stock only looks; reserve_stock takes the units out of the count, so a second buyer cannot get them while this payment runs."}
{"type":"stop","id":"s2","path":"app/inventory.py","qualname":"check_stock","kind":"function","lines":[12,15],"hash":"0b7e55c1d2f4a960","from":"s1","line":13,"text":"check_stock answers one question: are there at least `quantity` units of this SKU? An unknown SKU counts as zero. It reads and never changes the stock."}
{"type":"halt"}
{"type":"halted"}
{"type":"resume"}
{"type":"stop","id":"s4","path":"app/inventory.py","qualname":"check_stock","kind":"function","lines":[12,15],"hash":"0b7e55c1d2f4a960","from":"s3","line":21,"same_as":"s2","text":"Called again here: reserve_stock does not trust its caller to have checked."}
{"type":"end","at":"2026-11-23T18:47:31Z"}
{"type":"pin","commit":"f4d6e15c0a9be1d2f7a8a1b0c3d4e5f60718293a","at":"2026-11-23T18:47:31Z","where":{"s1":["app/checkout.py",11,30],"s1.b1":["app/checkout.py",11,24],"s1.b2":["app/checkout.py",11,16],"s2":["app/inventory.py",12,15],"s4":["app/inventory.py",12,15]}}
{"type":"ids","nodes":{"s2":"3M1X9C7B5N2V8K4J6H0G1F3D5S"}}
```

Shortened: `s3`'s own `stop` and `step` lines and `s1.b3` are left out. As printed it replays to three stops (`s1`, `s2`, `s4`),
because `s3` was planned, never written, and left the route at `end` ([`03`](03-core.md) §1); TR1's test adds `s3`'s line and gets
four.

| `type` | Written by | Keys | Means |
|---|---|---|---|
| **`tour`** | `start`, `import` | `schema` · `id` · `title` · `question`? · `author`? · `at` · `detail` (`quick` \| `normal` \| `detailed`) · `next`? (only after an `import --replace`: the highest numbers ever used, §5, as `{"stop": 12, "blocks": {"s3": 4, "s7": 2}}`; a stop with no entry has used none) | the first line, and only the first. `schema` is the one version gate of the file: a number this V-NOC does not know makes the tour *cannot be read* |
| **`meta`** | `edit` | any of `title` · `question` · `detail` | a later value replaces the first line's. The id does not change with the title |
| **`plan`** | `start --plan`, `plan`, `start --route` | `stops`: a list of `{ id, path, qualname, occurrence?, from?, line?, same_as? }` | stops that are on the route and **not written yet** (`◌`) |
| **`stop`** | `add`, `import` | `id` · `path` · `qualname` · `occurrence`? · `kind` · `node`? · `lines`? · `hash`? · `from`? · `line`? · `same_as`? · `text` | a written stop: its node (§6), where its lines were, what they hashed to (§7), how the tour got here, and the intro. An id that is already on the route (planned, or written before) **replaces** that stop where it stands and clears its blocks; a new id goes to the end |
| **`step`** | `add`, `import` | `id` · `stop` · `parent`? · `lines` · `hash` · `title` · `text` | one **block** of a stop. `parent` is the block it is a detail of; absent at the top of the stop. An id the stop already has replaces that block **in place**; its details stay under it. A `step` whose stop is not written (planned, dropped, or never there) is a problem line |
| **`drop`** | `drop` | `id` | a stop (with its blocks and the stops nested in it) or a block (with its details) is off the tour |
| **`ids`** | `ids` | `nodes`: stop id → node ID | these stops' nodes now have an ID (§6). It becomes the stop's `node` |
| **`rebase`** | V-NOC, when it edits a file itself (an ID line: `vnoc tour ids`, attach, write-ID, repair) | `steps`: stop or block id → `{ lines, hash }` | these stops and blocks were fresh just before V-NOC's own edit; this is where they are now and what they hash to (§7). Replay replaces their `lines` and `hash`, **and keeps the hash it replaced as a second one the check accepts**: until the file is saved (the editor applied the edit to a buffer that was already dirty) and after an undo, the code on disk is the code before the edit, and that is fresh too. It does not undo a pin: the pin's `where` still names the code as it was. `hash` is absent for a file stop, which has none |
| **`pin`** | `finish`, `pin` | `commit` · `at` · `where`: stop or block id → `[path, first, last]` | the tour is pinned (§8): every stop's lines are in this commit, at these places. It holds until a later `stop`, `step` or `drop` |
| **`halt`** | the core, for the Stop button | — | the reader asked the agent to stop |
| **`halted`** | the core, when it refuses an agent's write because of a `halt` | — | the agent has been told. *stopping* becomes *stopped* |
| **`resume`** | `resume` | — | the agent may add again. After an `end` it changes nothing: the tour stays finished |
| **`end`** | `finish`, `import` | `at` | the author is done. Planned stops still not written leave the route. A written stop nested in one of them moves up to that stop's own `from` (or the top), and its `line` is dropped: it was a line of the stop that left |

The keys of a stop, one by one:

| Key | Value |
|---|---|
| `path`, `qualname`, `occurrence` | the node's name (§6). `qualname` is `""` for a file or a folder; `occurrence` is written only when it is not 1 |
| `kind` | `function` · `class` · `file` · `folder`, as `node.toml` writes it. For the icon and the name when the stop is *gone* |
| `node` | the node's ID, **only when it had one** when the stop was written (an `ids` record adds it later) |
| `lines` | `[first, last]`, 1-based, in the file, as `vnoc code` printed them at that moment. Absent for a folder |
| `hash` | §7. Absent for a folder, and for a file |
| `from`, `line` | the stop this one is nested in, and (for a call) the line in **that** stop's node where the call is |
| `same_as` | the stop that already explained this node |
| `text` | the intro: one paragraph ([`02`](02-commands.md) §4) |

A block's `lines` are also file lines at the moment it was written. The reader turns them into "lines 3 to 8 of the node" by the stop's
own first line, so a function that moved down the file as a whole keeps its blocks ([`03`](03-core.md) §2).

**Not in the tour's file, on purpose:** where a reader is and what they have seen, and whether a step is out of date. Both are true of
one machine and one working tree, not of the tour. They are in the state file (§9). Also not there: the size in minutes (computed), and
a time on every record (the file's own changed-time says when the agent last wrote, which is all "no news for 5 minutes" needs).

**The state of a tour** comes from its records, in order:

| State | When |
|---|---|
| **writing** | no `end`, and no `halt` since the last `resume` |
| **stopping** | a `halt` with no `halted` after it |
| **stopped** | a `halted` with no `resume` after it |
| **finished** | an `end` (a `stop` or `step` after it is an edit of a finished tour; it stays finished) |
| *no news* | **writing** or **stopping**, and the file was not changed for 5 minutes. Not a state of the file: the reader of it says so (design 05 §1) |
| *pinned* | beside the state, not one of them: the last `pin` is later than every `stop`, `step` and `drop` |

## 5. Ids

| Id | Form | Made | Rule |
|---|---|---|---|
| a tour | `checkout-order-7k2q` | by `start` or `import`: the slug of the title (the doc rules of `04-storage/01` §3, but **ASCII only**: lower-case `a-z`, `0-9` and single `-`; a run of spaces, `_` or other ASCII punctuation becomes one `-` (`checkout_order` → `checkout-order`), a character that is not ASCII is dropped (`Über café` → `ber-caf`), `tour` if nothing is left; at most 40 characters; it is typed by agents and is a file name on three systems), `-`, four characters from `0-9 a-z` without `i l o u` | it is the file's name, and the name of its state file. The random part keeps two branches from making the same file. It never changes: not when the tour is shared, not when its title is edited |
| a stop | `s1`, `s2`, … | by `plan`, `add` or `import`: one more than the highest stop number ever used in the file, dropped ones too | never reused. Not the number on screen |
| a block | `s3.b1`, `s3.b2`, … | by `add` or `import`: one more than the highest `b` ever used under that stop, at any depth | never reused. A detail is `s3.b2` with `parent: "s3.b1"`: the id does not say the depth. The stop's own id (`s3`) is its intro, as in the design's `StepId` |

A command takes a tour's id, or the start of one when only one tour starts so. An id that is wrong gets "did you mean", as a node does
(21.02 §4). Ids are made under the lock (D10), after reading the file, so two commands at once get two different numbers.

`import --replace` keeps the ids its document gives and makes new ones, above the highest ever used, for what has none (the highest
is kept in the new file's first line as `next`, so a number dropped before the import is still never reused).

## 6. How a stop names its node (D6)

Simple on purpose. A module that finds nodes again (after a rename, a move, an edit) is later work for all of V-NOC; a tour uses two
direct checks until then.

| When the stop is written, the node | The stop keeps | Found again by |
|---|---|---|
| has an ID | its name **and** `node` | **the ID:** the def that carries it, wherever it is and whatever it is called now |
| has no ID | its name | **the exact name:** the same path, qualname and occurrence |
| neither is found | | the stop is *gone*: the row struck through, the text kept (design 05 §4) |

- **The name is not an identity.** `location.rs` forbids saving a location as a reference that must hold. This one may break, and
  the tour shows it; nothing else in V-NOC depends on it.
- **`add` writes no ID.** No `ID:` line, no `node.toml`, no node folder.
- **`vnoc tour ids <tour>` writes them when you ask** ([`02`](02-commands.md) §9): for every stop whose node has none, the ID, the
  marker in the code and `node.toml`, by the attach flow without its last step (no doc). Then one `ids` record. From then on the
  tour follows those nodes through renames and moves, as their docs would. It honours `vnoc.toml`'s `[ids] default = "none"`: in such a
  project it is refused with that reason.
- **Whether that happens by itself is a setting** (§12): `[tours] ids = "always"` makes `finish` do it; `"ask"` (the start) makes
  `finish` say it could be done; `"never"` says nothing.
- All of the finding is one function, `tours::find(index, stop) -> Option<node>`. The later module replaces its body; the stop
  already keeps what a smarter search would want (the name, the ID, the kind, the hash of the lines).

## 7. The hashes

`hash` is the first 16 hexadecimal characters of the BLAKE3 hash of some lines, joined with `\n`.

| Hash of | The lines |
|---|---|
| a stop on a function or a class | the node's lines, `range.start.line ..= range.end.line` as the driver gives them (decorators in, the blank lines after it out: `source.rs`) |
| a block | the block's own lines |
| a stop on a file | none. A file's intro is about the file as a whole; one changed byte somewhere in it does not make "this file holds the stock" untrue. Its blocks are hashed |
| a stop on a folder | none |

Left out of every hash, so that V-NOC's own writing never makes a tour stale:

| Left out | Because |
|---|---|
| the line end (`\r\n` or `\n`) | a checkout on Windows must not change every hash. Lines are split as `source.rs` splits them |
| white space at the end of a line | a formatter's or an editor's doing, never a change of meaning |
| **a line that holds an ID marker**: a line containing the ID text of the def itself or of a def inside its lines (`Def.id`), or the file's `FileID` | `vnoc tour ids`, or attaching a doc, adds an `ID:` line inside the function. Without this rule, giving a tour IDs would mark that tour as changed. **[check]** TR0: that "contains the ID text" finds exactly the marker line in Python and in Bend. **Answered (README §7, TR0 check 4): yes, one line each, in both; the nested rule shown on Python only** |

**V-NOC's own edits never make a fresh stop stale** (TR5's finding). Leaving the marker line out is not always enough: attaching to a
Python function whose docstring is one line (`"""Charge once."""`) rewrites it into three (the text, a blank line, `ID: …`), so the
first line itself changes. A rule about docstrings in the hash would be one language's rule in a language-free module. Instead, every
V-NOC writer of a marker knows the file before and after its edit, and calls `tours::rebase(path, before, after)`: for each tour with a
stop in that file, each stop and block **that was fresh against `before`** gets its lines and hash against `after`, in one `rebase`
record. A stop that was already changed stays changed: a rebase never hides an edit someone else made. The writers hold the project's
write lock and then take the tours lock, never the other way round.

Everything else counts: a comment, a renamed variable, a changed indent. A tour is text **about** these lines; it is honest to say they
are not the same lines any more, and to let the reader judge (or, when the tour is pinned, to show the lines it was about: §8).

The hash is the whole rule, in one function (`tours/hash.rs`).

## 8. The pin (D19)

**Pinned** means one thing: *there is a commit in which every stop of this tour is exactly the code the tour was written about.* That
commit is in the tour's `pin` record. A tour that is not pinned has no commit in it at all.

**How a tour gets pinned.** `finish` tries; `vnoc tour pin` tries again later ([`02`](02-commands.md) §8). Three questions, in order:

```
1. is every written stop fresh against the files as they are now?            (the check of 03 §2: fresh, or moved)
      no  → not pinned: "s3 changed since it was written". Replace it, then pin.
2. are all the tour's files committed, and in HEAD?
      git status --porcelain -z -- <the files of its stops>     must say nothing      (modified, staged, untracked: each says so)
      git ls-tree --name-only -z HEAD -- <the same files>       must name every one   (an ignored file, or one git never had, is not there)
      no  → not pinned: "2 files have changes that are not committed: app/inventory.py, app/payments.py"
                        "app/generated.py is ignored by git: this tour cannot be pinned"   (git check-ignore -q, only to word it)
3. both yes → what is in HEAD is what is on disk is what the tour explains.
      write  pin { commit: HEAD, where: each stop's and block's path and lines now }
```

No old file is read to make a pin: step 2 is what makes HEAD equal to the files step 1 just checked. **Why two calls in step 2**
(TR0, check 6): `status` with paths says **nothing** for a clean file, an ignored file and a path that does not exist alike. `ls-tree`
says which of them HEAD holds. Step 1 already turned a deleted file into a *gone* stop, so a pin is never tried on one; `ls-tree` is
the guard that does not depend on that. Each call is 3 to 5 ms (TR0).

**When files are not committed.** The tour is finished, playable, and flagged *not pinned*, with the list. What happens next is the
setting `[tours] commit` (§12): `"always"` makes `finish` commit them and pin; `"ask"` (the start) makes it print them, and the agent
asks you; `"never"` only flags. Whatever the setting, these three are always there:

| | |
|---|---|
| you commit, as you would anyway, then `vnoc tour pin` (or the row's **Pin**) | the usual case: the agent explained work in progress, you commit it later |
| **`vnoc tour pin --commit`** (on screen: **Commit and Pin…**, which first shows the files and the message) | commits **exactly the tour's files that are not committed**, the whole of each, and nothing else; then pins. The message is `Code for the tour "<title>"` unless you give one. A file git ignores cannot be committed: the tour stays not pinned and says which file |
| leave it | a tour that is not pinned works. It only cannot show old code |

With `"ask"`, the agent never commits by itself: `finish` prints the files and the command, and the skill tells the agent to **ask
you** ([`04`](04-skill.md)). With `"always"`, you have already answered, once, for the project.

**What a pin gives: always playable.**

| A stop of a pinned tour | The stage shows |
|---|---|
| fresh | the code as it is now, live, as a card does |
| changed | **the code as written**: the lines of the pinned commit (`git show <commit>:<path>`), with the tour's lit lines exactly where its text says. A line under the text: *"The code changed since `f4d6e15`. This is how it was."* and a switch, **Now** |
| gone | the same: the code as written. *"This is no longer in the code. This is how it was at `f4d6e15`."* No **Now** |

So the text and the lit lines of a pinned tour always match, next week and after a refactor. A tour that is not pinned falls back to
the design's *changed* mark (the code as it is now, the lit lines dashed, *"the lines may not match"*).

**Read, not checked out** (decided 2026-11-23). What a tour must capture is the commit's id; that is in the `pin` record and never
changes. How the commit's code is shown is a view, and can be changed at any time without touching a tour. The view built now is
`git show`: "playing at that commit" means reading that commit's files, not moving your working tree to it:

| Checking out the commit would | Reading its files |
|---|---|
| fail, or need a stash, when you have edits | does not care what your tree holds |
| change the files your running program, your tests and your editor are on | changes nothing |
| rebuild the index for the old tree, and again on the way back | reads a few files once, kept in memory |
| leave you on a detached commit if anything goes wrong in between | has no "in between" |

*Later:* the whole project at the pinned commit, with V-NOC browsing it, is `git worktree add` into a folder of its own, opened in a
second window. The `pin` record is all it needs.

**What can break a pin, and what happens.**

| Case | Result |
|---|---|
| the tour is edited after the pin (a stop replaced, a block added) | not pinned until pinned again (§4). `add --replace` on a pinned tour says so |
| the commit is not in this repository: a shallow clone, a teammate who has not fetched it | the row says `pinned at f4d6e15 (not in this repository)`. Changed stops fall back to the *changed* mark. Nothing is lost: after a fetch it works |
| history was rewritten: the branch was rebased or squashed when it was merged, and the pinned commit is on no branch | the same as above for teammates; on your machine it works until git cleans up. **Pin a project tour after its code is merged:** `vnoc tour pin` on a tour that is fresh and committed writes a new `pin` with the commit you are on. `vnoc tour check` says `can be pinned again` when the pinned commit is not reachable from HEAD and the tour is fresh |
| a file of the tour is ignored by git, or the project is not a git repository | it can never be pinned. Flagged, with the reason. (An ignored file is found by step 2's `ls-tree`: `status` alone would read it as clean, TR0) |
| a submodule's file | not pinned first: its commit is another repository's. Said in the refusal |
| the tour's IDs (`ids`, or `finish` with `ids = "always"`) wrote `node.toml` files that are not committed | `--commit` commits them too (decided by the user, 2026-11-24): the ID lines in the code and the node files they name go in one commit. Only the `node.toml` of a stop's own ID, never a doc file in that folder; listed under the code files. The pin does not wait for them: it is about code |
| a hook changes a file while `--commit` commits it (a formatter, `lint-staged`) | the commit holds code the check never read: not pinned, `a hook changed <file> while committing it`, and `pin` again checks the new code (TR9b). Found by comparing the files after the commit with the bytes the check read. git leaves the real index at the file before the hook (`status` says `MM`), so those files get `git reset -q --` (`pin::settle_index`): the next `pin` sees them clean |

**Why not a copy of the code in the tour instead.** It would make every tour playable with no git. It also doubles the file and puts
source text into `.vnoc/`, in every review of a shared tour. The pin gives the same for everything that is committed, and for what is
not, committing it is one command. The room stays: a `code` key on a `stop` would be an addition.

## 9. The state file

`.vnoc/local/tour-state/<tour id>.json`: one small JSON object per tour, **rewritten** (a new file renamed over the old one, under the
lock). It is state, not history; if it is lost, the reader starts at the cover and the next look checks the tour again.

```json
{ "schema": 1,
  "place": { "at": "s3.b2", "seen": ["s1", "s2"], "back": ["s1.b1", "cover"], "moved": "2026-11-23T18:52:10Z", "by": "reader", "open": true },
  "check": { "tour_len": 4182, "at": "2026-11-23T18:52:11Z",
             "files": { "app/checkout.py": [1204, 1763923201123456789], "app/inventory.py": [1893, 1763923190000000000] },
             "stops": { "s3": "changed" }, "blocks": { "s3.b2": "changed" }, "moved": { "s3.b1": [22, 23] } } }
```

| Part | Holds | Written by |
|---|---|---|
| **`place`** (D17) | `at`: `"cover"`, `"end"`, or a step's id · `seen`: the stops read to their last step · `back`: where the reader was before each jump, 20 at most · `moved` and `by` (`reader` or `command`) · `open`: a window is showing this tour (set and cleared by the core; `where` reads it together with `moved`, so a crash that left it `true` reads as "open, last moved two days ago") | the core, when the reader moves on the canvas; `vnoc tour go` |
| **`check`** (D8) | the tour file's length and each file's `[size, changed-time in ns]` when the check ran · what was not fresh: the changed stops and blocks, the blocks found at other lines · `now`: for a stop found under another name or path, its name, path and lines now (added in TR6, so `check` can say `read_order (was parse_order)` without an index) · A stop not named is fresh; a file recorded as `null` was not there | the core, when it checks a tour for the sidebar or for an open tour; `vnoc tour check` |

- **As built (TR10):** `back` holds `"cover"` and `"end"` as well as step ids. A `place` that does not parse reads as none and keeps
  the `check` beside it. `check` gained **`displaced`**: for a fresh stop found at other lines (shifted or moved), its lines now, so a
  valid recorded check can say which lines to light with no index; a fresh block's lines now are its written lines shifted by its
  stop's displacement (`compare` calls a block `moved` only when it moved inside its stop). Missing reads as empty.
- **A recorded check is valid** while the tour's file has that length and every file in `files` has that size and time. Then it is the
  answer, and no source file is read. Otherwise the tour is checked again and the record rewritten ([`03`](03-core.md) §3).
- **One place per tour on this machine** (ask 3). The tour's id is the "session id": `vnoc tour where <tour>` has one answer.
  `vnoc tour where` with no id answers for the tour whose place moved last.
- The file follows the tour's id, so sharing a tour (a move between folders) keeps your place and its check.
- Deleting a tour moves its state file to the trash with it.

## 10. Rules

Everything in `.vnoc/tours/` arrives by `git pull` and is untrusted input (`04-storage/01` §5 rule 2).

| Rule | Why |
|---|---|
| a file name that is not `<valid id>.jsonl` is ignored and reported once | folder names under `nodes/` have the same rule |
| the first line is not a `tour` record, or its `schema` is higher than this version knows → the tour is listed as *cannot be read*, with only Delete | design 07 §8 |
| a blank line (only white space) → skipped, no problem | a hand edit, a merge |
| a second `tour` record → a problem line | the first line is the only one |
| a later line that is not JSON, or lacks what its type needs → **that line** is one problem (the file and the line number) and is skipped. The rest of the tour loads | rule 3: a broken part breaks itself only |
| a `type` this version does not know → skipped, silently | a newer V-NOC, or 27 |
| the last line has no `\n` → not read, no problem | it is being written this moment, or a crash tore it. The next append starts with `\n`, which turns the torn piece into one broken line |
| `path` must be a valid `RepoPath` (no `..`, not absolute), in a stop and in a `pin` | a pulled tour cannot make V-NOC read a file outside the project |
| `commit` must be 40 hexadecimal characters, and `path` a valid `RepoPath`, before git sees either. `git show` gets `<commit>:<path>` as its one argument, **with no `--` before it** (after `--` git reads it as a path and prints nothing, with exit 0: TR0). A 40-hex commit cannot start with `-`, so it cannot be read as an option. The paths of `status`, `ls-tree`, `add`, `commit` go after `--` | a pulled tour cannot hand git an option |
| a line ends at `\n`; a `\r` before it is dropped | git may check a committed tour out with `\r\n`. V-NOC writes `\n` |
| a `text` or `title` is shown as text. Only `` `code` `` and `*emphasis*` are drawn, by a small renderer of their own, never through the doc panel's Markdown and never as HTML | the text comes from a model, or from a pull |
| at most **40** stops on the route, **12** blocks in a stop, **1 MB** a file. A write refuses past them; a reader stops reading at 1 MB and says so | 40 is the design's limit (the segments, the route). A tour that wants more is two tours |
| a block whose `parent` is missing, or is in another stop, is shown at the top of its stop | a hand-edited or half-merged file still plays |
| a state file that does not parse is ignored and replaced at the next write | it is this machine's, and it can be made again |

**Editing a tour's file by hand** works for a `text` or a `title` (a pull-request suggestion on a shared tour). Changing `lines` by hand
leaves the old `hash`, so that block shows as changed: use `export` and `import --replace` for anything but words.

Two branches that each added lines to the same shared tour conflict at the end of the file, as any two appends do. Keeping both sides is
right when the lines are different stops; two stops that got the same id on two branches is the one case a person has to look at (the
second replaces the first). Rare: a shared tour is finished.

## 11. What it adds to the frozen format

All of it is addition (`04-storage/04-growth-path.md` §2 and §3).

| Addition | An older V-NOC does |
|---|---|
| `.vnoc/tours/` | does not read it (it reads `vnoc.toml`, `tags.toml`, `nodes/`). **Checked in TR0** with the released 0.3.1 command line: nothing listed, nothing changed, and `status`, `tree`, `check` print the same, with the same exit code, as without the new folders (README §7, check 1). The VS Code half is by hand |
| `.vnoc/local/tours/`, `.vnoc/local/tour-state/`, `.vnoc/local/tours.lock` | nothing: `local/` is ignored by git and unknown names in it are not read |
| `[tours]` in `vnoc.toml` | keeps it when it rewrites the file (`toml_edit`), reads nothing from it |
| a node folder with `node.toml` and no doc (after `vnoc tour ids`) | **[check]** TR0. It is what a node looks like after its last doc file was deleted by hand, so it should already be a state every version reads. **Answered (README §7, TR0 check 5): a quiet state in 0.3.1; it is reported only if its code is deleted (Missing, `0 docs`) or its `ID:` line is removed (ID lost)** |

No `format = 2`. The newest fixture of `04-growth-path.md` §5 gets one project tour and one private tour with its state file, so
"opening it changes no byte" keeps being checked.

**Frozen (TR14d, `6194253`).** Schema 1 is what `crates/vnoc/tests/fixtures/tours/v1/` holds: a shared tour with every record type
of §4, a private tour with one unknown key and one unknown `type` (hand-made, as its README says), two state files, one export
document. `tests/cli.rs` `tour_frozen_schema_1_…` reads them to committed goldens and checks that list, show, where and check change no
`.jsonl` byte and no place. These files are never edited: a later format is a new folder beside them, and v1 still reads.

## 12. The settings (D21)

```toml
# .vnoc/vnoc.toml
format = 1

[tours]                # optional, as every table but `format`. Absent = both "ask"
ids    = "ask"         # write IDs for a tour's stops:               "always" | "ask" | "never"
commit = "ask"         # commit a tour's files so it can be pinned:  "always" | "ask" | "never"
```

| Value | At `finish` (and `import`) | On screen |
|---|---|---|
| **`"always"`** | it is done, and the output says what was written or committed | nothing to click: the cover says `pinned at …` |
| **`"ask"`** | it is not done. The output says what could be, and with which command; the agent asks you ([`04`](04-skill.md) §3) | the cover offers it: **Write IDs**, **Commit and Pin…** |
| **`"never"`** | it is not done and not offered. `ids`: silence. `commit`: the tour still says `not pinned`, with the files, because that is a fact about the tour | the menu entries stay; the cover does not offer |

- **Order at `finish`:** the IDs first, then the pin. Writing IDs changes source files, so with `ids = "always"` those files are not
  committed when the pin is tried: `commit = "always"` commits them with the rest; `commit = "ask"` asks once.
- **Pinning itself is not a setting.** When everything is committed, a pin is one record and touches nothing: `finish` always tries.
- **For one call:** `vnoc tour finish <tour> --ids` / `--no-ids` / `--commit [-m "…"]` / `--no-commit` win over the setting.
- **At any time, whatever the setting:** `vnoc tour ids <tour>` and `vnoc tour pin <tour> --commit`. A setting says what happens
  **by itself**; it never forbids you a command. (The one thing that does forbid is the older `[ids] default = "none"`: a project
  that wants no markers at all gets none from a tour either, and `ids = "always"` there is reported as a problem, once.)
- **Changing it:** edit the file; or `vnoc tour settings --ids always --commit ask` ([`02`](02-commands.md) §9), which changes those
  keys and keeps everything else in the file; or the check box in the **Commit and Pin…** dialog and in the *IDs written* message:
  *"Always do this in this project"*.
- It is the **project's** setting, committed, the same for everyone on it: whether a team's tours carry IDs and commits is a team's
  habit. A value V-NOC does not know is read as `"ask"` and reported once.

Next: [`02-commands.md`](02-commands.md)
