# 22.03 One run, step by step

```
running
├── 1. the steps
├── 2. plan                 everything decided, nothing touched
├── 3. the command          words, no shell
├── 4. place                the copy
├── 5. the lock file        and the sweep that removes what a crash left
├── 6. run                  the process, its output, the limits
├── 7. stop · timeout       the whole group
├── 8. clean                always
├── 9. the index stays quiet
└── 10. what a run can end as
```

## 1. The steps

```
run(node, slug)
 1. gate      is running allowed here?                                   05 §2        → refused
 2. plan      read play.toml + the script; fill defaults; replace         §2           → "the recipe is broken: …"
              placeholders; find the tools; check every rule of 02 §7
 3. lock      create and lock .vnoc/local/play/runs/<name>.lock           §5           → "already running"
 4. place     create dir/file with the script's bytes, never overwriting  §4           → "a file with that name is there"
 5. start     the command, in dir                                         §3, §6       → "could not start: python3 not found"
 6. stream    stdout and stderr, as they arrive                           §6
 7. end       the process exits, or Stop, or the timeout                  §7
 8. clean     remove the copy, the folders step 4 made, the lock file     §8           (always, whatever happened after 3)
 9. report    how it ended, the exit code, the time                       §10
```

Steps 1 and 2 touch nothing. Everything a run changes on disk is between 3 and 8, and 8 runs on every path out.

## 2. Plan

```rust
pub struct RunPlan {
    pub dir: PathBuf,              // absolute; inside the root, symlinks followed
    pub copy: PathBuf,             // absolute: dir + the resolved `file`
    pub name: String,              // {name}
    pub command: Vec<String>,      // the words; [0] is the program
    pub env: Vec<(String, String)>,
    pub timeout: Option<Duration>,
    pub script: PathBuf,           // the file in .vnoc/ to copy
}
pub fn plan(project, userdata, node, slug, tools) -> Result<RunPlan, PlanError>
```

A plan is the whole truth about a run. The panel draws its `dir` and `command` beside the Run button; `vnoc play show` prints it; a test
compares it to a table. `PlanError` is one sentence for a person, naming the key and the file (`02` §7's refusals, a placeholder nobody knows,
a tool that was not found, a preset that does not exist).

## 3. The command: words, no shell

`run` is split into words:

- spaces separate words; `"…"` and `'…'` make one word with spaces in it; inside `"…"`, `\"` and `\\` are a quote and a backslash;
- **placeholders are replaced after the split**, each inside its word, so a project path with a space in it stays one word;
- nothing else has a meaning: no `$VAR`, no `~`, no `|`, no `&&`, no `>`.

`{python} -X dev {file}` → `["/Users/me/shop/.venv/bin/python", "-X", "dev", "vnoc_play_parse_a_bad_order.py"]`.

The first word is the program: an absolute path, or a name looked up on `PATH`. **[check]** PG0: on Windows, a name that is a `.cmd` file
(`npx`) with no shell; the preset can carry a different word for Windows if it must (04 §1 `run_windows`).

Someone who needs a shell says so, and owns the portability: `run = "sh -c 'source .env && python {file}'"`.

## 4. Place

1. If the copy's parent folder does not exist (`examples/` in a crate that has none), create it, and remember that this run made it.
2. Create the copy with `create_new` (`userdata/atomic.rs`): it fails if anything is at that path. **A run never overwrites and never
   reuses a file it finds.** The message says which file and that V-NOC did not make it (or that a sweep will: §5).
3. Its bytes are the script's bytes, unchanged.

Before step 1 of the first run in a project, once: if `<root>/.git` is a folder and `.git/info/exclude` has no `vnoc_play_*` line, append
one (ask 1 of the README). Failing to is not an error.

## 5. The lock file, and the sweep

`.vnoc/local/play/runs/<name>.lock` is created and locked (`std::fs::File::lock`, the same call 21's write lock uses) before the copy is
placed, and holds what this run will put into the tree:

```toml
copy    = "/Users/me/shop/vnoc_play_parse_a_bad_order.py"
made    = ["/Users/me/shop/examples"]          # folders this run created; usually empty
started = 2026-11-17T10:22:33Z
```

- **Another run of the same name** (a second click, an agent and a person at once, two windows) cannot take the lock: `already running`.
  Different playgrounds have different names and run side by side.
- The operating system drops the lock when the process that holds it ends, however it ends. So a lock file **that can be locked** belongs to
  a run that died: **the sweep** takes it, deletes the `copy` if it is still there, removes each `made` folder if it is empty, and deletes the
  lock file.
- The sweep runs when the core starts, at the start of every `vnoc play run`, and before a run that found its name taken (then it tries once
  more).
- Two playgrounds on different nodes can have the same slug, so the same `{name}`. They cannot run **at the same time**; the second is told
  which one holds the name. (A name with part of the node's ID in it would end this, and make every traceback and every `--example` longer.
  Not worth it until someone meets it.)

No process ids, no "is it still alive" guess: the lock is the liveness test, and it is the same on three systems. **[check]** on Windows in
PG3, by hand.

## 6. Run

| | |
|---|---|
| working folder | `dir` |
| standard input | closed. A script that reads it gets end-of-file at once |
| environment | the core's own, plus `[env]`, plus `VNOC_PLAY=1` (a script, or the code it calls, can tell) |
| process group | its own (`process_group(0)` on Unix; a job object or the like on Windows: check 2 of the README) |
| output | stdout and stderr are read as they arrive and passed on in chunks, each marked `out` or `err`, in arrival order |
| text | bytes are decoded as UTF-8; a character split between two chunks waits for its other half; bytes that are not UTF-8 become `�` |
| colour | ANSI escape sequences are removed in the core. The result is plain text for any editor and for an agent |
| limit | the first 1 MB is passed on. After that the output is still read (so the process does not block on a full pipe) and dropped, and the end says `cut: true` |

The environment in VS Code is the extension host's. When the window was started from the Dock, VS Code has usually read the login shell's
`PATH` **[check]** (check 3 of the README). If a program is not found, the message names the word and both fixes: an absolute path in
`.vnoc/local/play.toml` `[tools]`, or the command in the form.

## 7. Stop and the timeout

Both do the same: kill the **group**, wait for the pipes to close, go to step 8. `cargo run` starts the example; `npx` starts `node`; killing
only the first would leave the script running with nobody reading it.

- The timeout is `timeout` from the plan: 60 s when nothing says otherwise, none when `0`.
- The core going away (the window closes, the core is killed) ends the group too **[check]** PG0: on Unix a group does not die with its
  parent by itself; the core kills the runs it holds when it shuts down, and a run of a core that was killed outright keeps going until its
  timeout. The sweep removes its copy only once it has ended (its lock is then free). That is the one case where a copy outlives its window,
  for at most the timeout.

## 8. Clean

Always, in this order, each one best-effort and never failing the run:

1. delete the copy;
2. remove each folder this run made, **if it is empty** (the script may have written into it: then it stays, and the end says so);
3. unlock and delete the lock file.

What a run leaves on purpose: whatever the command itself produced (`target/debug/examples/vnoc_play_*` from Cargo, `__pycache__`). Those are
the build tool's, in folders the project already ignores.

## 9. The index stays quiet

A file appearing in the tree is, to V-NOC, a change of the project: the watcher tells the core, the core re-reads, the revision moves, every
call tree on screen is marked stale. A run must not do that.

- `scan.rs`: a file or folder whose name starts with `vnoc_play_` is skipped. (With the exclude line of §4 the walker already skips it:
  `git_exclude(true)`. The explicit rule is for a worktree, a project with no git, and a person who removed the line.)
- The server's `filesChanged` handler drops paths whose last part starts with `vnoc_play_` before it decides what to refresh.
- Test (PG3): a run on the fixture; the index's revision is the same before and after, and `vnoc/structure` never lists the copy.

## 10. What a run can end as

| `how` | When | Exit code | Shown as |
|---|---|---|---|
| `exit` | the process ended by itself | its own | `exit 0 · 0.8 s`, red when not 0 |
| `stopped` | Stop | — | `stopped · 3.1 s` |
| `timeout` | the limit passed | — | `timed out after 60 s` |
| `not_started` | step 5 failed: the program was not found, or cannot be run | — | the reason, and the two fixes of §6 |
| *(an error, not an end)* | steps 1 – 4 | — | the sentence: refused · broken recipe · already running · name taken |

With `exit`, `stopped` and `timeout` comes `ms`, `cut`, and `left` (a `made` folder that was not empty).

Next: [`04-languages.md`](04-languages.md)
