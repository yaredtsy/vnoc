# 22.04 Languages

```
languages
├── 1. a language is a preset      the fields; the built-in table
├── 2. Python
├── 3. TypeScript and JavaScript
├── 4. Rust                        pub only · the private step, designed
├── 5. editing a script            what language help works where
├── 6. adding a language           a table in vnoc.toml; Go as the example
└── 7. a preset is not a driver
```

## 1. A language is a preset

A preset is a row of data. The runner (03) never asks "is this Rust?"; it reads the row.

| Field | Is | Used |
|---|---|---|
| `ext` | the script's extension | naming the script of a new playground (`script.py`): the editor colours it by this |
| `file` | the copy's path, relative to `dir` | 03 §4. Its last part must start with `vnoc_play_`, so in practice with `{name}` |
| `run` | the command | 03 §3 |
| `run_windows` | the command on Windows, when it differs | optional; only if check 6 of the README asks for it |
| `anchor` | file names that mark a good `dir` | *New Playground* proposes the nearest folder at or above the owner node that holds one; none found → the project root |
| `tools` | the placeholders this command needs found, and how | 02 §6 |
| `starter` | the text of a new script | optional. `{hint}` in it becomes the owner's `path::qualname` |
| `timeout` | seconds | optional; 60 |
| `note` | one line the form shows under the command | "reaches `pub` items only" |

The built-in table (`crates/vnoc/src/play/presets.rs`):

| `language` | `ext` | `file` | `run` | `anchor` | `tools` |
|---|---|---|---|---|---|
| `python` | `py` | `{name}.py` | `{python} {file}` | — (the root) | `python` |
| `typescript` | `ts` | `{name}.ts` | `npx --no-install tsx {file}` **[check]** | `package.json` | — |
| `javascript` | `js` | `{name}.js` | `node {file}` | `package.json` | — |
| `rust` | `rs` | `examples/{name}.rs` | `cargo run --quiet --example {name}` | `Cargo.toml` | — |
| `custom` | *(asked)* | *(required in `play.toml`)* | *(required)* | — | — |

A project's `vnoc.toml` can change any field of a row, or add a row (02 §5, §6 below).

## 2. Python

```
dir = "."     file = "{name}.py"     run = "{python} {file}"
```

- **Imports.** Python puts the script's own folder first on `sys.path`. With the copy at the project root, `from src.orders import …` works
  as it does for the project's own scripts. A project whose packages start one level down (`src/` layout, a monorepo service) sets
  `dir = "src"` or `dir = "services/api"`, once, as the project's default.
- **Which Python** is the question that fails most often (a system Python that has none of the project's packages). `{python}` is found per
  machine: `[tools] python` in `.vnoc/local/play.toml` → `$VIRTUAL_ENV` → `<dir>/.venv` → `<root>/.venv` → `python3` → `python`. A project
  that uses a runner says so for everybody: `[play.language.python] run = "uv run python {file}"` (or `poetry run python {file}`), and then
  no tool needs finding.
- **Private names** (`_helper`, a nested function's parent) are importable. Python has no limit here.
- **The starter**, for a function or class in `src/orders.py`:

  ```python
  # playground for {hint}
  from src.orders import OrderService   # best effort: the file's path from dir, dots for slashes; the top-level name
  ```

  The import line is a guess from the path. It is right for the common layouts and editable when it is not; it is never checked.

## 3. TypeScript and JavaScript

```
dir = <nearest package.json>     file = "{name}.ts"     run = "npx --no-install tsx {file}"
```

- **Imports are relative to the file**, and `node_modules` and `tsconfig.json` are found by walking up from it. That is why the copy goes into
  the package's folder: `import { parseOrder } from "./src/orders"` then means what it means in the package's own code.
- **The runner differs by project**, more than in any other language. The default needs `tsx` among the project's dev dependencies;
  `--no-install` makes a missing one an error at once and not a download prompt on a closed standard input **[check]** PG0. The form's note
  lists the usual others: `node {file}` (recent Node versions run `.ts` by removing the types **[check]** which versions), `bun {file}`,
  `deno run -A {file}`. A project picks one in `vnoc.toml`.
- **Modules.** Whether `{name}.js` is an ES module or CommonJS is decided by the nearest `package.json` (`"type"`), as for the project's own
  files. A project that needs the other kind sets `file = "{name}.mjs"` or `"{name}.cjs"`.
- **Private** means "not exported". An unexported function cannot be reached; that is the language.
- **Other tools may see the copy while it exists**: a dev server that watches the folder, a test runner in watch mode. It lives for the
  length of the run. A project where that hurts sets `file = "scratch/{name}.ts"` (a folder its tools ignore).

## 4. Rust

```
dir = <nearest Cargo.toml>     file = "examples/{name}.rs"     run = "cargo run --quiet --example {name}"
```

Cargo treats every `.rs` file in a package's `examples/` folder as a small program that uses the package's library **[check]** PG0: found
with no line in `Cargo.toml`, unless the package says `autoexamples = false`. The script is an ordinary `fn main`:

```rust
use shop::orders::parse_order;

fn main() {
    println!("{:?}", parse_order("id=7;qty=x"));
}
```

What to know, and what the form's note says:

| | |
|---|---|
| **`pub` only** | an example is its own crate. It sees what any other crate would: the library's public items |
| **a package with no library** (only `src/main.rs`) | has nothing an example can import. The message says so; the fix is the usual one (move the logic into `src/lib.rs`), or the private step below |
| **a workspace root with no `[package]`** | is not a place to run. `anchor` proposes the nearest `Cargo.toml` above the node; the form checks it has `[package]` and offers the members if not **[check]** how cheaply: a `toml` read, no `cargo metadata` |
| **the first run is a build** | of the library (usually already built) and the example. **[measure]** PG0: first and second run on this repository |
| **dev-dependencies** | are available to an example. Useful: the playground can use what the tests use |
| **what stays** | `target/debug/examples/vnoc_play_*`: Cargo's, in a folder the project already ignores |

### The private step (designed; not built first: ask 5)

Privacy in Rust follows the module tree, not folders. To reach a private function, the script must be a **module of the crate**, declared by
a line in the module that owns the function. A child module sees its parent's private items.

```
recipe:   language = "rust-private"   file = "src/orders/{name}.rs"   attach = "src/orders.rs"
          run = "cargo test --quiet {name} -- --nocapture"

on a run, between "place" and "start":
          append to src/orders.rs:      #[cfg(test)] mod vnoc_play_parse_a_bad_order; // vnoc-play
on clean: remove that line
```

The script is then a test module (`#[test] fn play() { … }`), compiled only for `cargo test`.

It is the same idea as the copy, applied to one line, and it costs more:

- it **edits a tracked file**. `git status` shows `src/orders.rs` as modified during the run; no exclude line can hide that;
- it is refused when that file has unsaved changes in an editor (the extension knows: `dirtyFiles.ts`), and the write goes through the same
  path as an ID marker (the editor applies it, or 21's `EditSink::Disk`);
- the lock file records the line, and the sweep removes exactly a line that ends in `// vnoc-play` and names this module: nothing else;
- the recipe gains one key, `attach`. It is an addition; a V-NOC that does not know it cannot run that playground and says why.

Everything it needs from the runner is one new step between two existing ones, which is why the runner's steps are a list (03 §1) and not one
function.

## 5. Editing a script

The script lives in `.vnoc/nodes/…/play/<slug>/`, and runs somewhere else. The editor's language help looks at where it **lives**:

| Language | In the editor tab | Why |
|---|---|---|
| Python | colours, formatting; imports resolve when `dir` is the project root **[check]** PG4 with Pylance | the language server resolves from the workspace root, which is where the copy would be |
| TypeScript / JavaScript | colours, formatting, the standard library. **Relative imports show as errors**: `./src/orders` is relative to the run place, not to `.vnoc/…` | the file is outside any `tsconfig` and its imports point elsewhere |
| Rust | colours only | rust-analyzer does not analyse a file that belongs to no crate |

That is honest for a first version: a playground is ten lines, and Run tells the truth in a second. The real fix is to edit the script **at
its run place** (the copy exists while the tab is open, and saving writes back to `.vnoc/`): 10 §1. It needs nothing from the format.

## 6. Adding a language

No release, no code: a table in the project's `vnoc.toml` (02 §5).

```toml
[play.language.go]
ext     = "go"
file    = "{name}_test.go"            # a _test.go file in the package's folder sees the package's private names
run     = "go test -run VnocPlay -v ."
anchor  = ["go.mod"]
note    = "put the copy in the package's folder (Run in), not at the module root"
```

| The language needs | Then |
|---|---|
| a file run by a program | `run = "<program> {file}"` |
| the file in a special folder | put the folder in `file` (`examples/{name}.rs`) |
| a name, not a path | `{name}` |
| a file name pattern its tools pick up | put it around `{name}` (`{name}_test.go`); the last part must still start with `vnoc_play_` |
| a different program per machine | a tool (`{python}`); new tools need a row in the core's table today, and a `[tools]` line then finds them. Project-defined tools: 10 §7 |

When a table has proved itself in a few projects, it becomes a built-in row. That is how the list in §1 grows.

## 7. A preset is not a driver

| | A driver (`06-drivers.md`) | A preset |
|---|---|---|
| is | a program that reads source code | a row of strings |
| gives | the tree: files, classes, functions; markers; call trees | how to run a script |
| a language needs it to | **have nodes** | **have playgrounds** |

They are independent, on purpose: a Python function can own a shell-script playground, and Rust got a preset before it has a driver. But a
playground needs an owner, and today only Python files (and the folders above them) are nodes: the README's ask 3.

Next: [`05-safety.md`](05-safety.md)
