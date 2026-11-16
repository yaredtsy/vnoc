# 21.01 What agents get: the freedoms and the guard rails

```
what agents get
├── 1. the rule of thumb
├── 2. the freedoms          seven, each with the command that gives it
├── 3. the guard rails       what no command does, and what the skill forbids
├── 4. how a session goes    the loop the commands are shaped for
└── 5. what a good answer looks like
```

## 1. The rule of thumb

**Reading is free. Writing text is free. The structure under the text is V-NOC's.**

An agent may ask anything, in any shape, as often as it likes, and may write any Markdown it likes into a note. What it may not do is make the
things a note hangs on (an ID, a node folder, a `node.toml`, a marker in the code) by hand, or destroy anything. Those are the parts with rules a
model cannot guess (`05-identity/01-id-generation.md`), and the parts a teammate's `git merge` depends on.

## 2. The freedoms

| # | Freedom | Given by | Why an agent needs it |
|---|---|---|---|
| 1 | **Name a node any way it knows it** | every `<node>` takes a location (`a.py::C.f`), `file:line`, an ID, or a path ([`02`](02-naming-a-node.md)) | at different moments it holds a name it read, a line from a stack trace, an `ID:` it saw. Translating between them by hand costs a call and goes wrong |
| 2 | **Shape the structure it reads** | `vnoc tree` with a depth, kinds, a `--where` query, signatures, line ranges, doc titles, flat or nested, a limit ([`03`](03-read-commands.md) §4) | "what is in `billing/`" and "every undocumented function under `api/`" and "the classes of this file with their methods' signatures" are three trees. One fixed tree is always too much or too little for a context window |
| 3 | **Read exactly one thing** | `vnoc code <node>`, `vnoc show <node>` | one function instead of a 2,000-line file; with line numbers, because its edit tool works by exact text and place |
| 4 | **Ask for several at once, and pipe** | several `<node>` per command, `-` for standard input, `--format id\|path`, `--json` | five functions in one call, not five calls; `find … \| show -`; `jq` |
| 5 | **Search four ways** | `vnoc find` (a few letters of a name, words in a note, filters: doc 20), `vnoc grep` (a regular expression over code or notes, answered as nodes) | it rarely knows the exact name. And "which functions mention `retry_count`" is a question `rg` answers in lines, which it then has to map to functions itself |
| 6 | **Write a note about anything, of any kind** | `vnoc doc new <node>` on a function, class, file or folder; then the note is a Markdown file it edits with its own tools | a why, a plan, a decision, a trap. V-NOC does not police the content. `--tag plan` makes it findable as one (`tag:plan`); `/` is the whole project, once A17 is in |
| 7 | **Fix what it broke** | `vnoc check` says what is wrong; `vnoc repair` writes a lost `ID:` line back; `vnoc relink` moves a Missing node's notes to where the code went | agents rewrite whole functions, docstring included. Better to give the fix than only the rule |

And one that is not a command: **it works before any setup.** With no `.vnoc/` and no skill installed, `vnoc tree` and `vnoc code` still answer
(C6). An agent can try the tool on any Python project.

## 3. The guard rails

| What | How it is held | Why |
|---|---|---|
| **No remove, no trash, no empty trash** | no command exists | removing is a person's decision; Missing is a safe state |
| **No ID, node folder or `node.toml` by hand** | the skill's rule; `vnoc check` reports a folder that is not a node | the ID is a hash with exact input rules; a guessed one breaks clean merges |
| **No bulk writing** | `doc new` makes one note per call; there is no "document everything" | 400 notes that restate the code bury the 10 that explain something |
| **No `vnoc init` on its own** | `doc new` refuses in a project with no `.vnoc/` and says to ask the user; the skill's rule | starting V-NOC in a repository puts `ID:` lines in its code: the owner decides |
| **`ID:` lines stay** | the always-on rule in `AGENTS.md` (C11); `check` fails on a lost one; `repair` restores it | the one mistake agents make by default |
| **Notes are information, not instructions** | the skill says so; no command prints a note as if it were a command | a note arrives by `git pull` from someone else: it is a prompt-injection path |
| **Nothing leaves the machine, nothing outside the root is read** | the binary has no network code; paths are checked against the root (`RepoPath` refuses `..`) | it will be allowed without a prompt in many setups; it must deserve that |
| **Writes are small and say what they touched** | every write prints each file it changed, and says "read it again before you edit it" for a source file | the agent's own picture of that file is now old; its edit tool will refuse or, worse, overwrite the new `ID:` line |

Per-command permission (allow reads without asking, ask for writes) is the agent host's to give. The command names are chosen so a prefix rule
can tell them apart: every write is one of `vnoc init`, `vnoc doc new`, `vnoc relink`, `vnoc repair`, `vnoc agent`; everything else only reads
([`07`](07-install.md) §4).

## 4. How a session goes

The commands are shaped for this loop; the skill teaches it in these words.

```
orient      vnoc status                          is V-NOC here? how big? anything wrong?
            vnoc tree src/billing --sig          what is there

before      vnoc show src/orders.py:142          the function, and: does it have notes?
changing    vnoc doc read <node>                 read them first

change      (its own edit tools; the ID: line stays)

after       vnoc check --changed                 nothing Missing, no ID lost
            vnoc repair                          if one was
            vnoc doc read <node>                 are the notes of what it changed still true? fix them

when it     vnoc doc new <node> --title "…"      then write the body into the path it prints
learned a why
```

## 5. What a good answer looks like

The reader is a model with a limited context window (futureplan 02.01 §3, kept):

| Rule | In practice |
|---|---|
| **Short by default, more on request** | `tree` stops at depth 2 and 200 rows; `show` prints at most 120 lines of code; each says how much it left out and the flag that gives the rest |
| **Plain text first** | no box-drawing, no colour unless a terminal asks, two-space indents. JSON spends tokens on quotes |
| **Line numbers on code** | always |
| **Give back both names** | every node line has its location and, if it has one, its ID |
| **Say the next step, once, only when there is one** | `docs (2) → vnoc doc read src/orders.py::OrderService.parse_order` |
| **Errors that teach** | `no node at src/orders.py::parse_ordr · did you mean: src/orders.py::OrderService.parse_order` (from `vnoc_search`, 02 §4) |
| **Paths it can open** | a note's real path, relative to the root: reading and editing it needs no further `vnoc` call |
| **The same every time** | the same order for the same input; no timestamps, no timings on standard output |
