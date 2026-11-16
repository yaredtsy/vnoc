# 21.05 How a command runs

```
how a command runs
├── 1. one binary            serve, or a command
├── 2. the root
├── 3. one shot              exact, measured, and the cache that is ready if the number is bad
├── 4. standard output
├── 5. exit codes
└── 6. new dependencies
```

## 1. One binary

```
main.rs   parse arguments (clap, derive)  ─┬─ none, or `serve`  → today's LSP main loop, untouched
                                            └─ a command          → cli::run(command) → exit code
cli/      args.rs (the clap types) · out.rs (text, --format, --json) · one file per command group
          each handler: open the project, build what it needs, call the library, print
library   Project · DriverHost · index::refresh · Index · UserData · identity::{attach, relink, write_id}
          vnoc_search · search::corpus_of · node_ref (doc 20) · index::structure, calls::tree_for (A1)
```

- `cli/` lives in the library crate (`crates/vnoc/src/cli/`), so the integration tests can call `cli::run` as well as run the binary.
- The extension keeps starting the core with no arguments. A later release passes `serve`; both work from A2 on.
- The driver runs as the server runs it (`DriverHost`), and exits with the command (`DriverClient`'s `Drop`).

## 2. The root (C9)

1. `--root <dir>`, if given;
2. else the nearest folder at or above the current one that holds `.vnoc/`;
3. else the nearest that holds `.git`;
4. else the current folder.

The command opens that folder (`Project::open`), so the whole repository is in scope wherever in it the agent's shell happens to be. `.vnoc/`
is at the git root (`UserData::new(project.git_root())`), so 2 and 3 name the same folder in every ordinary repository.

## 3. One shot (C4)

Each run: open the project → start the driver → `refresh(…, Touched::All)` (walk, outline every file, read `.vnoc/`, resolve) → answer → exit.

| Command | Needs the driver | Needs the whole index |
|---|---|---|
| `doc list/read/path` by **ID**, `skill`, `guide`, `agent …`, `init` | no | no |
| `show`, `code`, `doc …` by location or line, `tree`, `find`, `grep`, `missing`, `check`, `changed`, `status`, every write | yes | **yes** |
| `calls` | yes | yes, and the driver's call analysis on top |

**Why the whole index even for one function.** futureplan 02.02 §2 drew a lazy design: outline one file for `show file:line`. It is not exact.
Which record a def carries is decided by `identity::resolve`, marker first: a record whose anchor says `a.py::f` but whose ID is written in
`b.py` belongs to `b.py`. A build that read only `a.py` would hand `f` a doc that is not its own, and `doc new` would then add to the wrong
node. Exactness is the product; speed is measured.

**The budget, measured in A0** before any command exists (a small example program that does the one-shot build and prints the times):

| Project | Budget for `vnoc status`, release build, warm disk |
|---|---|
| httpx (the project the pictures use) | 300 ms |
| 1,000 generated files | 1 s |
| `calls` on httpx, depth 2 | 5 s |

**[measure]**, written here. Inside the budget: nothing more is built. Outside it: step **A16**, designed now so it is not a new plan then:

```
the outline cache      .vnoc/local/cache/outline-1.json      (local/ is git-ignored; V-NOC's own caches already belong there: 04-storage)
  per file:   path · size · modified time · the driver's outline (defs, file ID)
  header:     the driver's version, the cache's format number
  a run:      walk → stat every file → same size and time: take the outline from the cache; else ask the driver → write the cache back (atomic)
  the driver: started only if at least one file has to be read
  wrong or unreadable cache, another driver version: ignored and rewritten. Never an error.
  no .vnoc/ in the project: no cache (nothing is written into a project that has not started V-NOC)
```

It is exact (the same index, from the same outlines), it serves every command, and the extension's core can read it at start-up too (the
"SQLite if measured" row of `04-storage/04-growth-path.md`, as one file). A process that stays warm is **not** built for the CLI: finding it,
its lifetime and two versions meeting are the costs futureplan 02.02 §2 lists, and they are what MCP gets for free ([`08`](08-mcp-and-plugins.md)).

## 4. Standard output

`main.rs` has `#![deny(clippy::print_stdout)]` because in `serve` standard output **is** the protocol. That attribute covers the binary crate
only; `lib.rs` has none. From A2:

- `#![deny(clippy::print_stdout, clippy::print_stderr)]` on the library too;
- `cli/out.rs` is the **one** module with `#[allow]`: it writes through a locked `stdout`, and every command prints through it;
- logs go to standard error with `VNOC_LOG`, as now; a command prints nothing else there unless it fails;
- a closed pipe (`vnoc tree | head`) ends the command quietly with `0`, not with a panic message.

## 5. Exit codes (C6)

| Code | Means | Examples |
|---|---|---|
| `0` | done | also: nothing found; `check` with no problems; a write that attached by name with a notice |
| `1` | `check` found problems | only `check` |
| `2` | the request cannot be met | unknown flag; no such node (02 §4); `doc new` with no `.vnoc/`; a refused relink; a title the node already has |
| `3` | the environment | no driver found, the driver failed to start, `vnoc.toml` has a newer `format`, the lock could not be taken, `git` missing for `changed` |

One function maps a library error to a code (`cli::exit_code`), with a table test, so a new error cannot pick a code by accident.

## 6. New dependencies

| Crate | For | Note |
|---|---|---|
| `clap` (derive) | arguments, `--help` | the help text is part of the interface: each command's `about` is written for an agent reading it cold |
| `regex` | `grep` | already in the tree through `ignore` → `globset` **[check]** `cargo tree -i regex-automata`: the cost is then small |

Each new crate needs `node scripts/notices.mjs` run again and `extension/THIRD_PARTY_NOTICES.md` committed (`scripts/package.mjs` refuses to
package without it). The step that adds the crate does it.
