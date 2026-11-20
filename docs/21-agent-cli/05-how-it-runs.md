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
main.rs   parse arguments (clap, derive)  ─┬─ none, `--stdio`, `serve`  → today's LSP main loop, untouched
                                            └─ a command          → cli::run(command) → exit code
cli/      args.rs (the clap types) · out.rs (text, --format, --json) · one file per command group
          each handler: open the project, build what it needs, call the library, print
library   Project · DriverHost · index::refresh · Index · UserData · identity::{attach, relink, write_id}
          vnoc_search · search::{prepare, answer} · node_ref (doc 20, built) · index::structure, calls::tree_for (A1)
```

- `cli/` lives in the library crate (`crates/vnoc/src/cli/`), so the integration tests can call `cli::run` as well as run the binary.
- **The extension starts the core as `vnoc --stdio`**, not with no arguments: `vscode-languageclient` appends `--stdio` for an executable
  started with `TransportKind.stdio` (`extension/src/core.ts`; checked 2026-11-18 in `vscode-languageclient` 9.0.1, `lib/node/main.js`).
  So `main.rs` decides before clap: the arguments are empty, `[--stdio]`, `[serve]` or `[serve, --stdio]` → the main loop.
  *(Changed 2026-11-20, A11c: the user ran bare `vnoc` in a terminal, it waited silently, and Enter crashed it with `Invalid header: "\n"`.)*
  **Empty arguments start the server only when standard input is not a terminal** (the extension always pipes it). In a terminal, bare
  `vnoc` prints the short help and exits `0`. `--stdio` and `serve` start the server whatever stdin is.
  *(Narrowed 2026-11-20, A11d, after A11c found `vnoc < /dev/null` panics with "stdin is a pipe".)* On Unix the server's stdin must be a
  **pipe or a socket** (`async_lsp::stdio::PipeStdin` takes both; *as built in A11e*: Node's `child_process.spawn` with `stdio: 'pipe'`,
  which is how the extension starts the core, gives the child **sockets**, not FIFOs, so "pipe" here means either). So: empty arguments
  start the server only when stdin **and stdout** are pipes or sockets; a terminal, a regular file or `/dev/null` → the help, exit `0`. `--stdio` / `serve` with a stdin or stdout that is not a pipe → exit `2` with
  `vnoc --stdio is for an editor, which talks to it through pipes; for a command, see vnoc -h`, never a panic. Windows keeps "not a terminal"
  (its server reads stdin through tokio, which takes any handle). Anything else
  goes to clap. `--stdio` is not a clap flag and does not show in `--help`. A later release of the extension passes `serve`; all four work
  from A2 on, and an old extension with a new core keeps working. `lifecycle.rs` starts the core with no arguments, the window suite with
  `--stdio`: A2 tests both.
- The driver runs as the server runs it (`DriverHost`), and exits with the command (`DriverClient`'s `Drop`).

## 2. The root (C9)

1. `--root <dir>`, if given;
2. else the nearest folder at or above the current one that holds `.git` (a folder, or the file a worktree or submodule has);
3. else the nearest that holds `.vnoc/`;
4. else the current folder.

The command opens that folder (`Project::open`), so the whole repository is in scope wherever in it the agent's shell happens to be.

*(Order changed 2026-11-18, before A2: the first version put `.vnoc/` before `.git`.)* `.vnoc/` is read at the **git root**
(`UserData::new(project.git_root())`), and `Project::open` walks up to `.git` by itself. So in an ordinary repository both rules name the
same folder, and where they differ the old order was wrong: a `.vnoc/` below the git root (a test fixture, a copied folder, the sample in
`crates/vnoc/tests/sample/`) would have been chosen as the root while the core read the repository's own `.vnoc/`. With `.git` first, the
root and the folder the core reads `.vnoc/` from are always the same. Rule 3 is for a project with no git at all. `--root` below a git root
is allowed: it is what the extension does when a subfolder is opened (the scope is that folder, `.vnoc/` is the repository's).

The sample project cannot be used where it lies (its git root is this repository): tests, the guide's examples and by-hand checks run it
as a **copy outside the repository**.

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

Since doc 20, `refresh` also reads **the text of every doc** (`DocTexts`: 17 – 32 ms for 1,000 docs of 2 KB, files in the system's cache;
20, 03 §7), and a one-shot has no earlier stamps to skip by, so it reads them all on every run, for `show` and `tree` as much as for `find`.
A0 measures with docs in the project (the generated one gets 1,000). If that part alone is over a tenth of the budget, `refresh` takes a
flag and the commands that read no doc text (`show`, `code`, `tree` without `--where`, `calls`, `missing`, `check`, `status`) skip it; not
before the number says so.

### Measured in A0 (2026-11-17)

`crates/vnoc/examples/oneshot.rs` (commit `29d9538` on `agent-cli`; `oneshot measure`). Apple M1, release builds of the example and the driver,
warm disk, a load average of about 3 (a browser was open). Each cell is the **median (min – max) in ms over 9 runs**, after one run that is
thrown away; every run starts from a new `Project`, `DriverHost`, `Index` and two empty search slots, as a new process would.

| Phase | httpx, no `.vnoc/` | httpx + 50 docs | generated: 1,000 files, 1,000 docs |
|---|---|---|---|
| walk (`scan::python_files`) | 0.7 (0.7 – 0.8) | 0.8 (0.7 – 2.4) | 1.5 (1.4 – 3.4) |
| **`refresh(All)` in all** (≈ `vnoc status`) | **28.6** (28.0 – 31.1) | **31.7** (31.4 – 32.9) | **237.4** (235.5 – 264.5) |
| · `.vnoc/` read (`load_all`) | 0.0 | 2.0 (2.0 – 2.2) | 42.6 (42.5 – 43.8) |
| · doc texts, from empty (`refresh_doc_texts`) | 0.0 | 0.7 (0.7 – 0.8) | 17.0 (16.9 – 17.5) |
| · outlines (a **difference**: total − walk − the two above) | 27.9 (27.3 – 30.3) | 28.2 (26.6 – 29.1) | 175.3 (174.3 – 203.6) |
| search: `prepare`, both slots empty | 1.4 (1.3 – 1.4) | 2.6 (2.6 – 2.8) | 27.0 (26.8 – 27.7) |
| · `prepare`, corpus alone (doc index kept) | 1.4 (1.3 – 1.4) | 1.5 (1.4 – 1.5) | 12.7 (11.6 – 13.4) |
| · of which `Index::tree` alone | 1.0 (1.0 – 1.1) | 1.1 (1.0 – 1.1) | 8.8 (8.5 – 9.5) |
| search: `run`, no limits | 0.2 | 0.2 | 14.2 (14.1 – 14.4) |
| `calls` depth 2, driver already running | 166.1 (165.4 – 169.1) | 166.7 (165.5 – 176.1) | 50.3 (42.8 – 52.4) |
| `calls` depth 2, cold driver (start included) | 172.0 (166.3 – 214.0) | 168.9 (167.6 – 177.3) | 47.4 (43.3 – 105.9) |
| driver start + one empty outline (a probe) | 3.1 (2.9 – 3.7) | 3.1 (2.9 – 5.4) | 2.9 (2.8 – 27.0) |

What was measured: **httpx** is `encode/httpx` at `7cff5f79bc78` (a shallow clone, dated 2026-11-16), the whole repository: `httpx/`, `tests/`,
`scripts/`, 60 Python files, 13 folders, 107 classes and 1,134 functions and methods. **+ 50 docs** is a copy of it with 50 docs on defs spread
evenly through the index (`oneshot docs`). **Generated** is `oneshot generate` (a fixed seed): 1,000 files in 20 folders, 10 defs each (1,000
classes, 9,000 functions and methods), 1,000 docs of about 2.1 KB, one on each file, with no `ID:` line in the code. `calls` is
`httpx/_client.py::Client.send` (a 24-node tree) and, on the generated one, a function of one generated file (4 nodes: that number says what the
driver costs to start, not what a call analysis costs). The search word is `request` (110 nodes, 1 doc) on httpx and `parse` (737
nodes, 735 docs) on the generated project. The log line `refresh` writes itself agrees with these (httpx 27 – 31 ms, generated 235 – 264 ms).

**Verdicts, by the rules above:**

- **`status` ≈ the refresh total: inside the budget.** httpx 28.6 ms (31.7 with 50 docs) against 300 ms; generated 237 ms against 1 s. `status`
  itself is not built; it adds `Index::tree` (1.0 / 8.8 ms) and the process's own start and print. **A16 is not scheduled** after A6; it stays
  designed, below, for a bigger project or a slower machine.
- **Doc texts alone: under a tenth of the budget, so `refresh` gets no flag.** 0.7 ms against 30 ms on httpx with 50 docs; 17.0 ms against 100 ms on
  the generated one (20, 03 §7 measured 17 – 32 ms). **A surprise next to it:** reading `.vnoc/` itself (`load_all`: 1,000 `node.toml` files and
  the listing of their docs) is 42.6 ms, two and a half times the doc texts, and with them 25 % of the refresh. Neither is over its budget; it is
  where the next ms would come from if a bigger project needed them.
- **`calls` depth 2 on httpx: 166 ms (172 cold) against 5 s.** The driver's start is 3 ms: the cold and the warm call differ by noise.
- **Search, next to doc 20's.** The doc index is 14.3 ms on 1,000 docs (`prepare` with both slots empty, 27.0, minus corpus alone, 12.7; doc 20:
  about 16 ms). The corpus is 12.7 ms on 10,000 names, **twice doc 20's "about 6 ms"**, and the reason is what `prepare` does that doc 20's number
  did not: 8.8 ms of it is `Index::tree`, the tree made from the index, so the corpus itself is about 4 ms. A one-shot makes the tree anyway;
  a command that already has one (`status`, `tree`) makes it once. `run` with no limits is 14.2 ms for a word with 737 node hits and 735 doc hits;
  doc 20's 1.3 – 1.6 ms was a run with the default limits (200 nodes, 50 docs), which `find` also has.
- **One more number, outside the budget:** the first run made right after the driver binary was built took 1,163 ms on the generated project
  (`index: … 1163 ms in all`), and the thrown-away first run of the final series 502 ms (httpx 62 ms, with 50 docs 59 ms). The 1,163 ms did not
  come back. It was not looked into:
  probably the first launch of a new binary. A user's first `vnoc` after an install may see it.
  **Measured again in A13 (2026-11-20), on the installed copy.** `install.sh` into a temp folder from the release archive, then three times: both programs
  (`vnoc`, `vnoc-driver-python`) copied into a new folder, `vnoc status` on a one-file Python project, and again at once. The first run took **0.57, 0.69
  and 0.64 s**; the second **0.00, 0.01 and 0.00 s** (the index, in the log: 473 ms in all, then 3 ms). So a **fresh copy costs about 0.6 s on its
  first run only**, whatever the project: it is not the index and not the driver's work (the second run does both in 3 ms). The reading: macOS checks
  an executable it has not seen before (the 29 MB of the two programs) at the first launch and remembers the answer; the files are the same bytes, so
  nothing of V-NOC's is slow. Not fixed, and not fixable from here; it goes into the README's troubleshooting ("the first `vnoc` after an install takes
  a second"). Linux and Windows were not measured.

Inside the budget: nothing more is built. Outside it, it would have been step **A16**, designed now so it is not a new plan then:

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
- logs go to standard error with `VNOC_LOG`, as now; a command prints nothing else there unless it fails. *(As built in A2: a command's default
  filter is `warn`, and when `VNOC_LOG` is not set `main` sets it to `warn` for the process before any thread exists, because the driver it starts
  reads the same variable and its own default is `info`: without that, every command printed the driver's "analyzer wire format" line.)*;
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
| `anstream`, `anstyle` | the palette (03 §11) | both are already built for `clap` (1.0.0 and 1.0.14): direct dependencies at the same versions add two edges to `Cargo.lock` and no package; the release binary grew 79,776 bytes (A6d) |
| `regex` | `grep` | `regex-automata` is already in the tree (through `ignore` → `globset`, and `tracing-subscriber` → `matchers`); `regex` itself is **not** (checked in A0, below) |
| `dialoguer` (A12b) | the agents menu of `vnoc agent install` in a terminal | added 2026-11-20 because the typed line was not recognised as a choice; default features off where they are not needed; measure the binary's size before and after |

**Checked in A0 (2026-11-17):** `cargo tree -p vnoc -i regex-automata` shows `regex-automata` 0.4.18; `-i regex` finds no such package. `regex-syntax`
0.8.11, `aho-corasick` 1.1.5 and `memchr` 2.8.3 are there too, and those four are all that `regex` 1.13.1 depends on. In a scratch copy of the
workspace with `regex = "1"` added, the list of crates in `cargo tree -p vnoc` grew by **one line, `regex` itself**. What grows is the compile
time of crates already there (`regex-automata` gets its `meta` and `nfa-pikevm` parts, `aho-corasick` its literal search), not the count. The step
that adds the dependency (A5) measures the build time and the release size.

**Measured in A5 (2026-11-19), the step that added it:** the release binary went from **7,771,808 to 8,705,808 bytes** (+934,000, +12 %). A clean
`cargo build --release -p vnoc` took 53.9 s before and 49.6 s after (CPU time 267 s and 260 s: the difference is the load of the machine, a load
average of 12 – 15 during both), so `regex` costs **no build time anyone can measure** and about 0.9 MB. `Cargo.lock` gained one package, `regex`
1.13.1, as the scratch copy said. `THIRD_PARTY_NOTICES.md` did **not** change: `regex` was already in it, through the driver's workspace, which
`scripts/notices.mjs` covers too.

Each new crate needs `node scripts/notices.mjs` run again and `extension/THIRD_PARTY_NOTICES.md` committed (`scripts/package.mjs` refuses to
package without it). The step that adds the crate does it.
