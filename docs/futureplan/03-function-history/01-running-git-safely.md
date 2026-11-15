# Future 03.01 Running git safely

```
running git
├── 1. CLI or library
├── 2. is spawning git from an extension acceptable?
├── 3. the real risk: a repo's own config can run programs
├── 4. the trust gate
├── 5. the hardened runner
└── 6. finding the git program
```

## 1. CLI or library

| | `git` CLI | `gix` (pure Rust) | `git2` (libgit2) |
|---|---|---|---|
| Blame | complete: `-L`, `-w -M -C`, `--ignore-revs-file`, `--contents -` | `gix-blame` exists, young **[check range support]** | `blame_file` with a line range; slow, fewer options |
| Log filters | all of them: `--author --since --until --grep -S -G -- path` | walk and filter yourself | walk and filter yourself |
| Reading many blobs | `cat-file --batch`, about 1 ms each | fastest | fast |
| Same behaviour as the user's git (config, attributes, ignore-revs) | yes | mostly | partly |
| Packaging | nothing | pure Rust, easy | a C library in every per-platform `.vsix` |

**Pick the CLI**, as graph diff already does ([`../01-graph-diff/02-how-to-compute.md`](../01-graph-diff/02-how-to-compute.md) §2). Parse only the
machine formats (`--porcelain`, `--format=` with `%x00` separators, `--raw --no-abbrev`), which are stable across versions. Keep every call inside
`git.rs`, so blob reads can move to `gix` later if profiling asks for it. `git2` is ruled out by the C dependency alone.

## 2. Is spawning git from an extension acceptable?

Yes. It is normal and expected, not a policy problem:

- VS Code's own Git extension runs the `git` program for everything it does. GitLens, the Python extension, rust-analyzer and V-NOC itself
  (`driver_host.rs` spawns the driver) all run child processes.
- The Marketplace has no rule against child processes **[check the current policy text on the day]**. What it cares about is honesty: do what the
  extension says it does.
- The one place it cannot work is a **virtual workspace** (vscode.dev, github.dev): no processes at all. V-NOC already needs a local core, so nothing
  changes. Remote SSH / WSL / containers are fine: the extension host, the core and git all run on the remote side.

So the question is not *whether* to run git but *when it is safe to*.

## 3. The real risk: a repo's own config can run programs

Git reads `.git/config` from the repo, and several keys there name a program that git will run:

| Key | Runs during | Relevant to read-only commands? |
|---|---|---|
| `core.fsmonitor` | anything that refreshes the index (status, diff against the working tree) | **yes** |
| `diff.<driver>.textconv` (+ a `.gitattributes` line) | `log -p`, `diff`, blame **[check: blame uses textconv by default]** | **yes** |
| `diff.external` | `git diff` (and `log` with `--ext-diff`) | yes for `diff` |
| `core.pager` | anything with a terminal | no, there is no terminal; pass `--no-pager` anyway |
| `core.sshCommand`, `credential.helper` | network commands | no, V-NOC never fetches |
| hooks | commit, checkout, merge… | no, V-NOC never runs those |

A normal `git clone` does **not** copy the remote's `.git/config`, so a cloned repo cannot do this by itself. The danger is a folder whose `.git/`
came from someone else: a zip with the `.git` folder in it, a shared drive, a repo nested inside a downloaded project. Git's own `safe.directory` check
(git 2.35.2+) exists for a close cousin of this problem.

This is exactly the case VS Code's **Restricted Mode** is for, and VS Code's own Git extension does not run git there **[check]**.

## 4. The trust gate

V-NOC already declares `"untrustedWorkspaces": { "supported": "limited" }` in `extension/package.json` and ignores workspace-set executable paths
(`restrictedConfigurations`). Extend the same rule:

```
Restricted Mode     → the core runs NO git command at all.
                      History, blame and diff show one line: "Trust this folder to see its history."
trusted             → git runs, through the hardened runner below.
trust granted later → vscode.workspace.onDidGrantWorkspaceTrust → tell the core → history appears.
```

The extension tells the core at start (one boolean in the initialize message, say `git: "allowed" | "restricted"`) and again when trust is granted.
The core enforces it in `git.rs`, so no other module can forget.

## 5. The hardened runner

Even when trusted, every call goes through one function in `git.rs`, and that function adds the same protections:

```
git --no-pager
    -c core.fsmonitor=false          no fsmonitor program, ever
    -C <git_root>
    <subcommand>  <V-NOC's own flags>
    --no-textconv  --no-ext-diff     where the subcommand accepts them
    --end-of-options  <revisions>    a revision from user input cannot become a flag
    --  <paths>                      a path cannot become a revision or a flag

environment   GIT_TERMINAL_PROMPT=0     never wait for a password
              GIT_OPTIONAL_LOCKS=0      never take the index lock; do not race the user's own git **[check: VS Code sets this too]**
process       Command::new(git).args(...)   no shell, so no quoting bugs and no injection
              kill_on_drop + a timeout      a slow blame on a huge file does not hang the core
```

Four rules that a test can check:

1. **Only read-only subcommands.** An allowlist inside `git.rs`: `rev-parse`, `log`, `blame`, `show`, `cat-file`, `diff`, `ls-tree`, `merge-base`.
   A test asserts that nothing else can be built.
2. **Paths are `RepoPath`.** It already refuses `..` and absolute paths.
3. **Revisions from a filter box** are checked once with `git rev-parse --verify --end-of-options <rev>` before they are used anywhere.
4. **Command-line `-c` wins over repo config.** That is git's precedence order, which is why `-c core.fsmonitor=false` is enough.

Why harden when trusted: "trusted" means the user trusts the code in the folder, not necessarily every key in a `.git/config` they never read. The cost
is a few flags.

## 6. Finding the git program

1. Ask VS Code's Git extension for the path it uses: the `vscode.git` extension's API (`getAPI(1).git.path`) **[check the API on the day]**. This
   respects the user's `git.path` setting and its Windows lookup.
2. If that extension is disabled, `git` on `PATH`.
3. If V-NOC ever adds its own `vnoc.git.path` setting, put it in `restrictedConfigurations` next to `vnoc.server.path` and `vnoc.driver.path`.

The extension passes the path to the core. The core never searches by itself, the same as for the driver.

**No git found, or a folder that is not a repo:** the history features hide; nothing else in V-NOC changes. `Project::open` already falls back to the
opened folder when there is no `.git`.

Next: [`02-blame-history-diff.md`](02-blame-history-diff.md)
