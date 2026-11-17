# 22.09 Build order

```
build order
├── 1. rules for every step
├── 2. phase 0: checks            PG0
├── 3. phase 1: the core          PG1 – PG3     stop point: a Python playground runs from a test
├── 4. phase 2: VS Code           PG4 – PG5     stop point: Python playgrounds, usable by a person
├── 5. phase 3: languages         PG6 – PG8     TypeScript/JavaScript · Rust · an owner in non-Python projects
├── 6. phase 4: agents            PG9 – PG10    after doc 21's A10
├── 7. phase 5: after             PG11 – PG12   optional
└── 8. what each step leaves working
```

On a branch `playground` from `main`. Each step is one commit and leaves the branch green. Phases 1 – 3 need nothing from search (20) or
the command line (21) beyond what is on `main`; phase 4 needs 21's A2, A7, A8 and A10.

## 1. Rules for every step

- **Stage files by path.** Never `git add .` or `git add docs`. Don't touch `docs/futureplan/`.
- **No logic in TypeScript** (P1). If a step finds itself choosing a default, joining a path or reading `play.toml` in the extension or a
  page, that belongs in `crates/vnoc/src/play/`.
- **No language name in `run.rs`, `sweep.rs` or `store.rs`.** `python`, `rust`, `cargo` appear in `presets.rs` and in tests only. A grep in
  the step's checks.
- **Checks per commit, light:** `cargo test -p vnoc` and `cargo clippy` when `crates/` changed; typecheck and the touched packages' unit
  tests when TypeScript changed; the window suite `VNOC_SUITE=play` (added in PG4) on PG4, PG5, PG8; full `pnpm -r test` on PG5 and PG8.
  The window suite needs `env -u ELECTRON_RUN_AS_NODE`.
- **A test that runs a real program skips, loudly, when the program is not there** (`python3`, `node`, `cargo`): it prints `skipped: no
  python3 on PATH`. CI has all three; a contributor's laptop may not.
- A step that settles a `[check]` or a `[measure]` writes the answer into the file that asked, in the same commit or a `docs:` commit after.
- Spec gaps go into these docs, not into silent choices.

## 2. Phase 0: checks

### PG0 Six checks (no product change)

Small programs under `spikes/play/`, and two things done by hand. Each answer is written into the README §6.

| # | Check | How |
|---|---|---|
| 1 | an older V-NOC and a `play/` folder | install the released 0.2 VSIX; a fixture whose node folder has `play/x/{play.toml, script.py}`: open, browse, Remove the node, Restore it. `git status` clean after open; the folder back after Restore |
| 2 | killing a tree | a Rust program that starts `sh -c "sleep 100 & wait"` (and on Windows `cmd /c` starting a child) in its own group, kills it, and proves both are gone. macOS and Linux here; Windows in CI or by hand. Decides the dependency (`libc`; `windows-sys` or `taskkill /T /F`) |
| 3 | `PATH` in a window started from the Dock | a throwaway command in the extension that logs the core's `PATH` and whether `python3`, `cargo`, `node`, `npx` resolve |
| 4 | Rust examples | in a copy of this repository: drop `crates/vnoc-search/examples/vnoc_play_x.rs`, `cargo run --quiet --example vnoc_play_x`; **[measure]** first run, second run, and a run after a one-line change of the script |
| 5 | TypeScript runners | a small package with and without `tsx` installed: `npx --no-install tsx f.ts`; `node f.ts` on the Node versions at hand. Decides the `typescript` row's `run` |
| 6 | no shell on Windows | starting `npx` and `python` by name from Rust's `Command`. Decides `run_windows` |

Commit: `play PG0: six checks, answers in docs/22-playground/README.md`.

## 3. Phase 1: the core

### PG1 The recipe (no disk, no process)

| Files | What |
|---|---|
| `crates/vnoc/src/play/mod.rs`, `lib.rs` | the module |
| `play/recipe.rs` | `Recipe` ⇄ `play.toml` with `toml_edit` (unknown keys and comments kept); `Change`; the lookup of 02 §4; `{name}` from a slug; placeholders; the word split (03 §3); every refusal of 02 §7 as a `PlanError` with its sentence |
| `play/presets.rs` | the `python` and `custom` rows; `[play.language.*]` read from a `vnoc.toml` document; the tool table with Python's search as a pure function over "does this path exist" and "the environment" passed in |

Tests (tables): a recipe round-trips byte for byte with an unknown key and a comment; each optional key absent → the project's value → the
preset's; `{name}` for `parse-a-bad-order`, `über-test`, `---`, a 60-character slug; the split: quotes, an escaped quote, a placeholder
whose value has a space; `{{`; an unknown placeholder; each row of 02 §7; `custom` with no `run`; an unknown language is listed and not
planned; Python's tool order, one row per rung.

Commit: `play PG1: play.toml, presets, placeholders, the plan's rules`.

### PG2 Playgrounds in the node folder

| Files | What |
|---|---|
| `identity/attach.rs` | `ensure_node` split out of `attach` (06 §6); `attach` = `ensure_node` + `create_doc`. No behaviour change: the existing attach tests pass untouched |
| `userdata/layout.rs`, `userdata/read.rs` | `play_dir`, `plays_of`; `NodeRecord.plays` (in its equality); a `play/x/` with a bad or missing `play.toml` → one `LoadProblem` |
| `play/store.rs` | `list`, `create` (slug by the doc rules, `-2` if taken; three `create_new` writes), `update`, `is_play` |
| `index/tree.rs`, `index/mod.rs`, `protocol.rs` | `Node.plays`, `MissingNode.plays` |
| `protocol.rs`, `server.rs` | `vnoc/plays`, `vnoc/playPresets`, `vnoc/playSuggest`, `vnoc/playNew`, `vnoc/playUpdate` (06 §3); `playNew` under the `attaching` mutex |
| `crates/vnoc/tests/fixtures/` | the newest fixture gains a playground; the "opens unchanged" test still passes |

Tests (`crates/vnoc/tests/lifecycle.rs` style, with the real driver): `playNew` on a function with no docs → node folder, `ID:` line,
`play/<slug>/` with two files, `structure` says `plays: 1`, the revision moved; a second with the same title → `-2`; on a folder; `plays`
lists both with their plans; `playUpdate` of `dir` to a folder that does not exist → refused, file unchanged; of `run` to `null` → the key
is gone; editing the script file by hand → the revision does **not** move; rename the function in the source → the playground is still
listed on it; Remove and Restore → still there.

Commit: `play PG2: playgrounds live in the node folder; the five requests`.

### PG3 A run

| Files | What |
|---|---|
| `play/run.rs` | `plan` (reads, resolves, finds tools on the real disk); `run`: place, start, stream (UTF-8 across chunks, ANSI removed, the 1 MB limit), stop, the timeout, clean (03 §4 – §8); the process group by PG0's answer |
| `play/sweep.rs` | the lock file, the sweep; called at server start |
| `scan.rs`, `server.rs` (`filesChanged`) | names that start with `vnoc_play_` are skipped (G15) |
| `play/run.rs` | the `.git/info/exclude` line, once (ask 1; skipped if the answer was no) |
| `protocol.rs`, `server.rs` | `vnoc/playRun`, `playOutput`, `playEnded`, `playStop`, `playRuns`; `State.runs`; the 50 ms coalescing; stop everything at shutdown; `initializationOptions.trusted` and the refusal (05 §2) |

Tests (need `python3`; skip loudly without): a script that prints to both streams and exits 3 → the chunks in order, `how: exit`, `code:
3`, the copy gone, the lock file gone; the revision is the same before and after, and `structure` never listed the copy; a file already at
the copy's path → refused, that file untouched; two runs of one playground at once → the second `already running`; a script that sleeps →
Stop ends it within a second and a grandchild it started is gone too; `timeout = 1`; a script that prints 3 MB → `cut: true`, about 1 MB
kept, the process not blocked; a split multi-byte character; a leftover made by hand (a lock file nobody holds, and its copy) → the sweep
removes both and nothing else; `trusted: false` → refused, nothing placed; a program that does not exist → `not_started`, nothing left.
**[measure]** 100,000 printed lines: time, and that `vnoc/structure` answers meanwhile.

By hand, once: the same on Windows (the lock, the kill).

Commit: `play PG3: plan, place, run, clean; leftovers are swept; the index stays quiet`.

**Stop point.** The core can do everything for Python. Nothing on screen.

## 4. Phase 2: VS Code

### PG4 The tab, the list, New Playground

| Files | What |
|---|---|
| `shared/messages.ts`, `extension/src/relay.ts` | the request types; the relay forwards them |
| `extension/src/newPlayground.ts` (new), `addDoc.ts` | the three questions (07 §3); the first-attach question shared with `addDoc.ts`; `playNew`; open the script beside |
| `extension/src/commands.ts`, `package.json` | `vnoc.newPlayground`, `vnoc.showPlaygrounds`; the menu group `6_play`; `menus.test.ts` rows |
| `webview/src/docs/DocPanel.tsx`, `webview/src/play/PlayList.tsx` (new), `playRows.ts` (new, pure) | the two tabs; the rows; the plan line; Edit Script; ⋯ (Rename, Copy Command, Reveal Folder, Remove: 07 §5) |
| `webview/src/node-card/footerChips.ts`, the sidebar row | the `plays` chip |
| `webview/src/state/view.ts` | which tab, which row is open, in `Saved` |
| `extension/src/attentionView.ts` | "2 docs, 1 playground" |

Tests: `playRows.ts` table (what a row shows for each `PlayInfo`: a plan, a refusal); `footerChips` rows; `menus.test.ts`; the window suite
`play` (new, registered as `edit` and `search` are): New Playground on a fixture function → the tab shows it, the script is open in a tab,
the card's chip says 1; Remove → gone, and the folder is in the system trash **[check]** how a test sees that; the palette command.

By hand: check 8 of the README (Pylance in the script's tab).

Commit: `play PG4: the Playgrounds tab, New Playground, the card's count`.

### PG5 Run on screen

| Files | What |
|---|---|
| `shared/session.ts`, `extension/src/session.ts`, `extension/src/core.ts` | the `plays.runs` slice, fed by `playOutput` and `playEnded`, filled once from `playRuns` |
| `extension/src/runPlayground.ts` (new) | save the script's tab if dirty, `playRun`, `playStop`; `vnoc.runPlayground` (the project-wide QuickPick), `vnoc.stopPlaygrounds` |
| `extension/src/extension.ts`, `core.ts` | `trusted` in `initializationOptions`; what happens when trust is granted (check 9) |
| `webview/src/play/PlayRow.tsx`, `playState.ts` (pure), `PlayOutput.tsx` | Run / Stop; the header; the output box; every row of 07 §7 |
| `webview/src/play/RecipeForm.tsx`, `recipeForm.ts` (pure) | the ⚙ form (07 §4); `playUpdate`; "use as the project's default" |
| `README.md`, `CHANGELOG.md` | the feature, and 05 §1's sentence |

Tests: `playState.ts`: one row per state of 07 §7, from a `PlayInfo` and a run; `recipeForm.ts`: a field equal to its default sends
`null`; the window suite: Run on the fixture's playground → the output text and `exit 0` in the session; Stop on a sleeping one; a dirty
script tab is saved before the run; reload the webview mid-run → the row is `running` again with the output so far; an untrusted window
**[check]** whether the test host can open one, else by hand.

Commit: `play PG5: Run, Stop, the output, the recipe form`.

**Stop point.** Python playgrounds work for a person in VS Code. This can ship.

## 5. Phase 3: languages

### PG6 TypeScript and JavaScript; a project's own languages

| Files | What |
|---|---|
| `play/presets.rs` | the `typescript` and `javascript` rows (PG0's answer for `run`); `anchor` used by `suggest`; `run_windows` if check 6 asked |
| `crates/vnoc/tests/play-langs/ts/` | a three-file package with `tsx` as a dev dependency (installed in CI) |
| `docs/22-playground/04-languages.md` | §3 with what PG0 and this step found |

Tests: `suggest` on a node two folders below a `package.json` → that folder; a run of a `.ts` script that imports the package's code
(skips without `node`); a `[play.language.go]` table in the fixture's `vnoc.toml` appears in `playPresets` and plans (no Go needed to plan);
a table that changes one key of `python` changes only that key.

Commit: `play PG6: TypeScript and JavaScript presets; languages from vnoc.toml`.

### PG7 Rust

| Files | What |
|---|---|
| `play/presets.rs` | the `rust` row; its `note` |
| `play/store.rs` (`suggest`) | a `Cargo.toml` anchor counts only with a `[package]` table; else the nearest member that has one |
| `crates/vnoc/tests/play-langs/rust/` | a two-crate workspace: a library with one `pub` and one private function, a binary-only crate |

Tests (skip without `cargo`): a run prints the `pub` function's result; `examples/` did not exist → it is created and removed; it existed
→ it stays; the copy is gone and `git status` of the fixture is clean (with the exclude line); a script that names the private function →
`exit 101`-style failure with rustc's message passed through (the test checks only non-zero and that the message is there); `suggest`
from the workspace root → the library crate.

Commit: `play PG7: the Rust preset`.

### PG8 An owner in projects with no Python (after ask 3 is answered)

Two possible bodies; the answer to ask 3 picks one or both.

- **(a) the root.** Needs 21's A17 (`.vnoc/root/`). `play/` under it by the same rules; `playNew` with the key `/`; the panel on the root.
- **(b) files of preset languages as file nodes.** `scan.rs` also collects files whose extension is a preset's `ext` and no driver's; the
  index holds them with no defs; the tree shows them as files; a doc or a playground attaches **by name** (`id_in_code = "no"`: there is no
  driver to plan a `FileID` line). The canvas shows a file card with no children and "V-NOC does not read `.rs` files yet" in place of the
  code's outline.

(b) changes what the tree means ("files V-NOC can read" becomes "files V-NOC knows a language for"), so it gets its own short design note
in this folder before it is built.

Commit: `play PG8: playgrounds on the project root` / `… on files V-NOC does not read yet`.

## 6. Phase 4: agents (after doc 21's A10)

### PG9 `vnoc play list`, `show`, `path`, `run`

| Files | What |
|---|---|
| `crates/vnoc/src/cli/play.rs` (in 21's layout) | the four handlers: parse, call `play::{list, plan, run, sweep}`, print (08 §2, §3); `--json` |
| `cli/show.rs`, `cli/tree.rs` | the playground count beside the doc count |

Tests (21's CLI test style, on the sample project): each command's text and JSON; `run`'s four exit codes; stdout and stderr kept apart;
Ctrl-C (a signal to the command) stops the script and removes the copy **[check]** how the command hears it: `tokio::signal`.

Commit: `play PG9: vnoc play list, show, path, run`.

### PG10 `vnoc play new`, the guide

| Files | What |
|---|---|
| `cli/play.rs` | `new`: `play::create` with `EditSink::Disk`, under the write lock (21.04 §7); the guard rails of 08 §5 |
| the guide's sources (21 A10) | the `play` topic (08 §4); the one bullet in `SKILL.md` |
| `crates/vnoc/tests/sample/` | one playground |

Tests: `new` on a function with no docs (the `ID:` line, the files, the printed lines); a taken title → `2`; `--again`; no `.vnoc/` →
`2` with the `init` sentence; a 300 KB script → `2`; the guide test covers the topic. Then 21's A14 way of trying it: two agents, three
tasks ("why does `parse_order` fail on `qty=x`? show me"), and the guide is changed from what they did.

Commit: `play PG10: vnoc play new; the guide's play topic`.

## 7. Phase 5: after (optional)

### PG11 Find them

`has:play` and `no:play` in the query language (20, 01 §2; one parser, so the box, `find` and `tree --where` get it at once);
`vnoc play list --where`. Commit: `play PG11: has:play`.

### PG12 Rust's private functions (only if ask 5 becomes yes)

The step of 04 §4: the `attach` key, the line added and removed through the edit seam, the lock file's `line`, the sweep's rule, the
refusal on a dirty file. Its own tests: the line is gone after a run, after a Stop, after a kill and a sweep; a file with unsaved changes is
refused. Commit: `play PG12: rust-private`.

## 8. What each step leaves working

| After | A person can | An agent can |
|---|---|---|
| PG3 | nothing new on screen | — |
| PG4 | create, list, edit, rename and remove playgrounds on any node | — |
| **PG5** | **run them (Python), stop them, read the output, change where and how** | read the folders under `.vnoc/` |
| PG6 | TypeScript and JavaScript; any language through `vnoc.toml` | — |
| PG7 | Rust (`pub`) | — |
| PG8 | own a playground in a project with no Python | — |
| PG9 | — | list, read and run what exists |
| **PG10** | — | **create its own; the skill tells it when** |
| PG11 | search for nodes that have one | `find 'has:play'` |

Next: [`10-later.md`](10-later.md)
