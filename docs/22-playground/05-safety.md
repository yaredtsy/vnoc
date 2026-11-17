# 22.05 Safety

```
safety
├── 1. what a run is           the user's own code, as the user
├── 2. the trust gate          Restricted Mode: list and read, never run
├── 3. the command is on screen
├── 4. why no approval dialog
├── 5. what V-NOC itself must never do    the rules a recipe passes
├── 6. agents
└── 7. secrets
```

## 1. What a run is

A run starts a program from the project, with the script from the project, as the user: their files, their network, their credentials. It is
exactly as safe as `python scratch.py` in their own terminal, and V-NOC says so in the same words, in the form and in the README. The old
V-NOC's doc called its runner a "sandbox executor" and then said it restricts nothing. This one is not called a sandbox anywhere.

So the question is not "is the script safe" (V-NOC cannot know) but **"did this person decide to run this project's code?"**, and the rules
V-NOC's own part must keep: it copies one file and deletes that one file.

## 2. The trust gate

VS Code already asks the question: **Workspace Trust**. A folder the user has not trusted is in Restricted Mode, and `extension/package.json`
already declares `untrustedWorkspaces: limited` (the core's and the driver's paths are ignored there).

| | Restricted Mode | Trusted |
|---|---|---|
| list a node's playgrounds, read a script, read the recipe | yes | yes |
| New Playground, edit | yes (they are files) | yes |
| **Run** | **no**: the button is disabled, with "Trust this folder to run playgrounds" and VS Code's *Manage Workspace Trust* | yes |

- **The gate is in the core, not only in the button.** The extension passes `trusted` in `initializationOptions`; `vnoc/playRun` answers
  `refused` when it is false. A page cannot run something by sending the request itself.
- Trust granted later: the extension tells the core (`onDidGrantWorkspaceTrust` → a notification, or a restart of the core) **[check]** PG5,
  whichever the code makes simpler.
- An editor with no such idea (a later port) passes `trusted: true` and owns that decision.

## 3. The command is on screen

The plan (03 §2) is drawn under every playground's title, always, not behind a click:

```
in services/api   $ .venv/bin/python vnoc_play_parse_a_bad_order.py
```

A playground named "format a date" whose command is `sh -c "curl … | sh"` shows that line beside its Run button. Nothing more clever than
that, and it costs nothing.

## 4. Why no approval dialog

The first idea (and my first answer on 2026-11-17) was: ask before a playground's first run on this machine, and again when its command
changes. Reasons it is not in:

- **It approves the wrong half.** `python {file}` is harmless and identical in a hundred playgrounds; what a run does is in the script. An
  approval that covers the script must be asked again on every edit, including the user's own, which teaches everyone to click Yes.
- **It adds nothing to what trust already allowed.** In a trusted folder, `git pull` can already change a test the user's next test run
  executes, a task, a build script.
- It needs a store of approved hashes per machine, and a story for agents (which have a shell anyway).

If a team asks for it, it is an addition: a hash of the plan in `.vnoc/local/`, asked in the extension. Nothing in the format waits for it.
(Ask 2 of the README.)

## 5. What V-NOC itself must never do

These are about V-NOC's own hands, and they hold even for a recipe written to break them (02 §7 lists the refusals):

| Never | Kept by |
|---|---|
| write outside the project root | `dir` is a `RepoPath` or `.`; it is resolved with symlinks followed and must still be under the root; `file` has no `..` |
| overwrite or delete a file it did not create | `create_new` for the copy; clean deletes the path this run created; the sweep deletes only a path written in a lock file under `.vnoc/local/` (which git does not carry: a pulled repository cannot plant one) |
| create a file with a name that means something to a tool | the copy's last part starts with `vnoc_play_` |
| read a script from outside its playground folder | `script` is a plain file name |
| run with a shell | words only (03 §3) |
| run on its own | every run is one click or one command. Opening a project, listing, searching, the walkthrough: none of them runs anything |
| leave the copy behind | clean on every path; the sweep for a crash |

## 6. Agents

`vnoc play run` has no gate. An agent that can run `vnoc` can run `python`, so a gate there protects nothing and only teaches agents to go
around the tool. What the skill tells an agent (08 §4):

- before running a playground **you did not write in this session**, read it: `vnoc play show`;
- never commit a `vnoc_play_*` file, and never write one yourself: if you see one, a run is in progress or crashed (`vnoc play run` cleans
  up at its next start);
- no secrets in a script.

The agent's own harness (its permission prompt for shell commands) is the gate a person set for it, and `vnoc play run` passes through that
like any command.

## 7. Secrets

A script is committed. The form's one standing line: "Committed with the project. No passwords or keys." A script reads what it needs from
the environment, which a run inherits; `[env]` in `play.toml` is for flags (`ORDERS_DEBUG = "1"`), and is committed too.

Next: [`06-core.md`](06-core.md)
