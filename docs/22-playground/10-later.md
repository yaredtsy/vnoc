# 22.10 Later, and why the format has room

```
later
├── 1. edit the script where it runs      full language help
├── 2. the editor's interpreter           {python} from the Python extension
├── 3. other ways to place a file         Go's overlay · a scratch crate
├── 4. a playground that checks itself    expected output · CI · promote to a test
├── 5. inputs and more files              arguments · standard input · fixtures
├── 6. scratch playgrounds                not committed
├── 7. tools a project defines
├── 8. a terminal, a debugger, a container
├── 9. smaller things
└── 10. the room each one needs
```

None of this is scheduled. It is here so each idea is written once, and so the last section can show that none of them changes a file
a user has already committed.

## 1. Edit the script where it runs

The limit of 04 §5: the editor's language help follows where a file is, and the script is in `.vnoc/`. The fix: **Edit Script places the
copy and opens the copy.** While the tab is open the file exists at its run place (`examples/vnoc_play_x.rs`), so rust-analyzer, the
TypeScript server and Pylance treat it as part of the project. Saving the tab writes the text back to the script in `.vnoc/`; closing it
removes the copy. The lock file of 03 §5 already describes "a copy that is in the tree on purpose", so the sweep needs one new word in it
(`for = "edit"`) and a window that is still alive to hold it.

Costs to settle then: the copy is in the tree for minutes, not seconds (the watcher of a dev server will see it); a window that crashes
leaves it until the next start; two windows.

## 2. The editor's interpreter

`{python}` found by rule (04 §2) is right when the project has a `.venv` or the user set `[tools]`. The Python extension knows which
interpreter the user picked. The extension can read it (`ms-python.python`'s API **[check]**) and pass it in `vnoc/playRun`'s `tools`,
which the lookup of 02 §6 already places second. No format change; one optional dependency on another extension's API, so it waits.

## 3. Other ways to place a file

The copy (G5) is the mechanism that works everywhere. Some toolchains have a better one, and a preset can name it without the recipe
changing shape:

| Way | For | Gain | Needs |
|---|---|---|---|
| **overlay** | Go: `go test -overlay <json>` treats a file as if it were at a path, without writing it | nothing appears in the tree at all | a preset field `place = "overlay"`, and the runner writing the JSON under `.vnoc/local/play/` |
| **scratch crate** | Rust: a small crate under `.vnoc/local/play/<name>/` with a path dependency on the user's crate | nothing in `examples/`; could share `target/` | a preset field `place = "crate"`, a manifest template; **[measure]** whether the shared `target/` rebuilds the world |
| **the `mod` line** | Rust's private items | 04 §4 | the `attach` key |

`place` absent means "copy", which is every playground written before the field exists.

## 4. A playground that checks itself

A playground is an example. An example that stopped working is a finding ("this function's behaviour changed").

- **Expected output.** `expected.txt` beside the script: the stdout of a run the owner accepted. *Run* then also says "same as expected" or
  shows the difference.
- **`vnoc play check [--all]`.** Runs every playground that has an `expected.txt`, compares, exits `1` on a difference. A CI job, next to
  `vnoc check`.
- **Promote to a test.** A playground that asserts something belongs in the test suite. A command that writes its script into the
  project's test folder with the language's test frame around it is a per-language template, so it is preset data again.
- **Run history.** The old V-NOC's doc promised "each run is stored"; its code never did. If wanted: the last N results under
  `.vnoc/local/play/history/`, never committed.

## 5. Inputs and more files

| Want | Shape |
|---|---|
| arguments | `args = ["--verbose"]` in `play.toml`, appended to the command's words |
| standard input | `stdin = "input.txt"`, a file in the playground's folder |
| a fixture next to the script | `copy = ["input.json"]`: files placed beside the copy under `vnoc_play_`-prefixed names, removed with it |
| several named inputs for one script | `[[case]]` tables: a title and `args`/`stdin` each; Run offers the cases |

The folder per playground (G2) is what makes these additions and not a new layout.

## 6. Scratch playgrounds

Ask 4. A playground under `.vnoc/local/nodes/<shard>/<id>/play/` is not committed: an agent's trial that nobody else needs. The layout is
the one `01-vnoc-folder.md` §2 reserved for private docs (`local/nodes/`); the loader reads a second root; the row shows a "local" mark and
a *Keep* action that moves the folder to the committed side. `vnoc play new --scratch`. Teams-and-privacy was dropped as a release
(2026-11-16), so this would be the first user of that space; that is the reason it is not in the first version.

## 7. Tools a project defines

Today a tool (`{python}`) is a row in the core with its search rule. A project may want `{node}` (a version manager's path) or `{dotnet}`.
The shape: `[play.tools.node] find = ["$VOLTA_HOME/bin/node", "node"]` in `vnoc.toml`, the same search steps the Python row uses, expressed
as data. Until someone needs it, an absolute path in `.vnoc/local/play.toml` `[tools]` already answers "which program on this machine"
for the tools that exist.

## 8. A terminal, a debugger, a container

- **Run in the terminal.** Place the copy, send the command to VS Code's integrated terminal (the user's shell, their activated
  environment, colour, input), remove the copy when the terminal's command ends **[check]** shell integration reports that. An editor act
  around the core's `plan`; the output is then not V-NOC's to show.
- **Debug.** Place the copy, start a debug session on it (`vscode.debug.startDebugging` with a configuration the preset names), remove it
  when the session ends. Breakpoints in the real code work, because the real code is what runs.
- **A container.** `run = "docker run --rm -v {root}:/w -w /w python:3.12 python {file}"` works today, as a command. A real sandbox
  (no network, a read-only tree) is a product of its own.

## 9. Smaller things

- A path in a traceback that names a project file is a link to that line (and a line in `vnoc_play_….py` opens the script).
- `vnoc play try --lang python --dir . -` : run a script from standard input once, placed and cleaned, kept nowhere. The agent's
  scratch file without the file.
- A CodeLens "▶ 2 playgrounds" above a function, when hover and CodeLens arrive as LSP features (`02-mvp-and-releases.md`, 0.7+).
- The doc panel renders a link `[example](play:parse-a-bad-order)` in a doc as a Run button: a doc that shows and proves.
- A playground's `tags`, as docs have (21 C16), and the same filters.

## 10. The room each one needs

| Later thing | Adds | Touches a committed file that exists? |
|---|---|---|
| edit where it runs | a word in the lock file (`local/`) | no |
| the editor's interpreter | nothing | no |
| overlay · scratch crate | an optional preset field `place` | no |
| the `mod` line | an optional key `attach` in `play.toml` | no |
| expected output | a file `expected.txt` in the playground's folder | no |
| arguments · stdin · fixtures · cases | optional keys in `play.toml`; files in the folder | no |
| scratch | the reserved `local/nodes/` | no |
| project tools | an optional table in `vnoc.toml` | no |
| terminal · debug | an optional preset field | no |

Every row is an optional key, an extra file, or something under `local/`. An older V-NOC that meets one keeps it (`toml_edit`), ignores it,
and runs the playground the way it knows, or says which key it cannot honour (04 §4's rule for `attach`). That is the same promise
`04-storage/04-growth-path.md` makes for docs, and the reason a playground is a folder with a small TOML file and not a row in a database.
