# 15. The Python driver leaves pylspt

Written 2026-11-12 from reading the code of `vnoc/drivers/python`, `pylspt` (`c395dde`) and `language_driver_rs` (`4834812` plus its uncommitted
work). **Nothing here was built or run.** Every `[check]` is a claim to verify before depending on it.

```
leave pylspt
├── 1. where things stand
├── 2. the order
├── 3. stage A: language_driver_rs (not this repo)
├── 4. stage B: the driver switches (this repo, one commit)
├── 5. stage C: clean up
├── 6. disk rules
└── 7. what is not part of this
```

## 1. Where things stand

The driver builds pylspt from git (`rev = "c395dde15b"`) and names four `ruff_*` crates. The same analyzer now lives in
`language_driver_rs/languages/python` as `python-syntax` and `python-analysis`. ruff and ty are therefore compiled once per repo.

| Driver file | Uses from pylspt | Replacement in `language_driver_rs` | Ready? |
|---|---|---|---|
| `outline.rs` | `defs::scan_defs`, the ruff parser | `python_syntax::outline` | yes (tag `python-v0.1.0`) |
| `plan_ids.rs` | `edit::plan_ids`, `edit::apply` | `python_syntax::plan_marker`, `analyzer_core::edit::apply` | yes |
| `file_id.rs` | `ruff_text_size` only | `python_syntax::header_marker`, `plan_header_marker` | yes |
| `positions.rs` | `ruff_text_size` only | plain `u32` byte offsets | yes |
| `call_tree.rs` | `db::open_project`, `interp::call_wire_at`, `FrameJson` with `target_file`, `target_qualname`, `target_nth`, `target_kind` | `python_analysis::{Project, call_wire, Frame}` | **no**: see §3 |

pylspt has one commit after the port (`c395dde`: the four `target_*` fields and `call_wire_at`). It was not ported. `language_driver_rs` builds
the same information its own way in steps E1–E5 of its `docs/05-python-migration/05-entries-and-targets.md`.

## 2. The order

```
A  language_driver_rs: E1–E5 and EW, then push   ← blocks B
B  vnoc: the driver switches, one commit         ← pylspt is no longer built after this
C  clean up: build folders, cargo's git cache, docs
```

Do not do B in two halves. A driver that uses `python-syntax` and still uses pylspt compiles two copies of ruff.

## 3. Stage A: `language_driver_rs`

Its own plan is complete; this section only says what B waits for.

| Step | What B needs from it | State on 2026-11-12 |
|---|---|---|
| E1 | `DefInfo` has `qualname` and `occurrence` | done: `8fafd0f` |
| E2 | `Frame` has `target_file`, `target_qualname`, `target_occurrence`, `target_kind`, `call_site_file` | done: `acc1b2a` |
| E3 | `Entry` and `Root` in core | done: `9120198` |
| E4 | `call_wire(&project, file, &Entry { qualname, occurrence }, &opts, merged)` finds a def the way the outline names it | done: `3236d54` |
| E5 | a class as the entry | done: `58748f3` |
| EW | `Project::open` strips the `\\?\` prefix `canonicalize` adds on Windows, so `target_file` and `call_site_file` stay relative there. It replaces the driver's `plain` (and fixes its UNC bug) | done: `a5625af` (added 2026-11-12) |

Then the owner pushes `main`. B needs the pushed commit's hash (called `<rev>` below), **and that repo's CI green on it, the Windows
`analysis` job for `python` included**: it is the only proof of EW. The push carries everything on `main`, not only E1–EW (19 commits
were unpushed on 2026-11-12), so the green run is also the proof that `<rev>` builds as a whole. `python-cli` (PY7) is **not** needed for B.

**Accepted exception to the green-CI rule (2026-11-12, by the owner).** `<rev>` is `4aae76a79f2498f950c4d7d1e2623e7e3f2f8090`. CI run #11 on it is green for
`core` and for Python on all three systems, the Windows Python job included (the proof of EW). `analysis (typescript, windows-latest)` is red, and was red
before this work. B goes ahead because the driver compiles no TypeScript crate: it names `python-syntax`, `python-analysis` and `analyzer-core`, and
nothing of TypeScript is in their dependency tree. Whoever bumps `<rev>` later checks again; this exception is for this `<rev>` only.

Confirmed 2026-11-12: `https://github.com/yaredtsy/language_driver_rs` is public (it answers without a login), so the driver's CI can
fetch it.

## 4. Stage B: the driver switches

One commit on `0.2-call-trees`. Read `python-syntax/src/lib.rs` and `python-analysis/src/lib.rs` at `<rev>` first: they are the whole
API, and where they differ from this section, they win (say so in the report).

### 4.1 Before the first build

1. `cd drivers/python && cargo clean`. The toolchain changes, so nothing in `target/` can be reused, and keeping it would hold two builds
   (§6).
2. `rust-toolchain.toml`: `1.98.0` → `1.99.1`.
3. `Cargo.toml`: remove the `pylspt` line and the four `ruff_*` lines. Add, all at one `<rev>`:

   ```toml
   python-syntax   = { git = "https://github.com/yaredtsy/language_driver_rs", rev = "<rev>" }
   python-analysis = { git = "https://github.com/yaredtsy/language_driver_rs", rev = "<rev>" }
   analyzer-core   = { git = "https://github.com/yaredtsy/language_driver_rs", rev = "<rev>" }
   ```

   The three must name the same `<rev>`, or Cargo builds `analyzer-core` twice and the types do not match. Fix the `description` and the
   comments that name pylspt and its toolchain.
4. `Cargo.lock`: copy `language_driver_rs/languages/python/Cargo.lock` at `<rev>` over it, then let the build add the driver's own crates.
   Do not run `cargo update`: ruff only compiles against the versions that lockfile holds (this is how the current lockfile was made, from
   pylspt's). After the first build, `git diff Cargo.lock`: if the version of `salsa` or of any `ruff_*` / `ty_*` crate moved, stop and
   report it `[check]`.

### 4.2 The files

Two constants, used everywhere: `MarkerStyle::docstring("ID")` for defs, and the same with the label `"FileID"` for the header (`[check]`
that the header functions ignore `placement`).

| File | After |
|---|---|
| `positions.rs` | `Lines::position(u32)`, `Lines::range(Span)`. `position_1based` is unchanged. No `ruff_text_size` |
| `outline.rs` | `python_syntax::outline(text, STYLE)` and a conversion to the protocol's `OutlineResult` (table below). `parse`, `Collector`, `qualname`, `ids_by_range` and `Outlined` are deleted. Keep one function that returns the `python_syntax::Outline` itself: `plan_ids.rs` needs its defs |
| `plan_ids.rs` | for each wanted ID: the def by `(qualname, occurrence)` in the outline, then `python_syntax::plan_marker(text, def, &want.id, STYLE)`. `Ok(edit)` is planned; `Err(skip)` goes through the reasons table below. `apply` is `analyzer_core::edit::apply` (it returns `Option`; `None` is an internal error, not a panic). `flatten` and `DefNode` are deleted |
| `file_id.rs` | `read` is `python_syntax::header_marker`; `plan` is `python_syntax::plan_header_marker`, with the same `Written` and `text_hash` as today. The six unit tests are the proof; they stay as they were except one token (changed 2026-11-12): `lines.position(TextSize::try_from(o).unwrap())` is `lines.position(u32::try_from(o).unwrap())`, because `Lines::position` takes a `u32` now and the driver no longer names `ruff_text_size`. The asserts are untouched |
| `call_tree.rs` | §4.3 |
| `main.rs` | the comment on line 107 says the analyzer, not pylspt. Nothing else |

**Outline conversion** (`python_syntax::Def` → `vnoc_driver_protocol::Def`):

| Protocol field | From |
|---|---|
| `qualname`, `name`, `params`, `syntax_error` | the same fields |
| `kind` | `Class` → `Class`; `Function`, `Method` and anything else → `Function` (the core's enum is `#[non_exhaustive]`: the `match` needs a `_` arm) |
| `parent`, `parent_kind` | `def.parent` is an index into the same list: the parent's `qualname` and its converted kind; `""` and `None` at module level |
| `range` | `Lines::range(def.range)` |
| `id` | `def.marker` |
| `file_id` | `python_syntax::header_marker(text, FILE_STYLE)` (the outline's own `file_marker` is not filled for Python) |
| `syntax_errors` | `range` through `Lines`, `message` as is |

**Occurrence.** The protocol is **1-based**, the analyzer is **0-based**. Convert at the edge, both ways, in one small function each. A
request with occurrence 0 is `InvalidParams`.

**Skip reasons** (`python_syntax::Skip` → `Skipped.reason`; the driver's own strings stay, a test pins one of them):

| `Skip` | Reason |
|---|---|
| `SyntaxError` | `syntax error inside this def` |
| `AlreadyMarked` | `replacing an ID is not supported` when `replace_existing`, else `already has an ID` |
| `UnsafeDocComment` | `no place for an ID` |
| `InvalidText` | `this ID cannot be written into a docstring` (new) |
| `DefNotFound` | not a skip: the ID goes to `unmatched` |
| anything else (`_`) | the `Skip`'s own `Display` text |

### 4.3 `call_tree.rs`

```
read params.root/params.file              cannot read → InternalError (as today)
Project::open(&params.root)               Err → InternalError
call_wire(&project, &params.file,
          &Entry { qualname, occurrence − 1 }, &opts, false)
                                          None → InvalidParams "no function <qualname> in <file>"
Frame → CallNode
```

`opts`: `Options { limits: Limits { deadline: Some(20 s), render_depth: params.depth, ..Limits::default() }, ..Options::default() }`.
The defaults already are what T5 and §5 of `14-call-trees.md` ask for: no ID filter, no prefix on `target_id`, summary mode.

| `CallNode` field | From `Frame` |
|---|---|
| `target.file` | `target_file`, as is: already relative to the root with `/`. An absolute path (a def outside the root) → `null` `[check]` that it cannot happen with the default options |
| `target.qualname` | `target_qualname` |
| `target.occurrence` | `target_occurrence + 1` |
| `target.kind` | `target_kind`: class → `Class`, else `Function` |
| `target.id` | `target_id`, `null` when empty. No prefix to strip |
| `call_site` | `call_site` (1-based line, character column) through `Lines::position_1based`, on the text of **`call_site_file`**, read from `root/<call_site_file>`. Not the parent's `target.file` any more |
| `marks` | the strings `recursive`, `cut`, `incomplete`; any other string is left out |
| `children` | the same, in order |

Deleted, because the analyzer does it now: `nth_def`, the `__init__` picking and `class_root` (a class entry's root frame already is the
class), `plain` and `system_path` (the analyzer canonicalizes and strips the Windows prefix itself: stage A, step EW; do not delete them
before `<rev>` has EW), the root stripping in `target`, the `FunctionSchema/` split, and
the read of the entry file's outline.

### 4.4 What changes for the user

| Behaviour | Before | After |
|---|---|---|
| a class whose `__init__` is inherited | root with no children | the inherited `__init__`'s calls |
| a property read or written | no node | a node for the getter or setter |
| siblings | evaluation order (`on_ok(fetch())` gives `fetch` first) | source order (`on_ok` first) |
| a def under `if` / `try`, a twin def, `outer.<locals>.inner` as the entry | worked (`call_wire_at`) | works (E4) |

Edit `14-call-trees.md` in the same commit: §3's picture and T1 name the new crates; §4 gets one line saying it is replaced by this
file; §5's row "An inherited `__init__` is not followed in 0.2" and its "which `__init__`" row are replaced by "the analyzer's
constructor lookup"; step 7.6 measures the new analyzer.

### 4.5 CI

`.github/workflows/ci.yml`: the comment on line 42 says `1.99.1`. The long-paths step stays (the new repo vendors ruff too) with its
comment reworded. Nothing else `[check]`.

### 4.6 Tests and proof

- Every existing test of `drivers/python` passes **unchanged**: `tests/outline.rs` (it checks the golden IDs and the reason
  `already has an ID`), `tests/call_tree.rs`, the unit tests of `file_id.rs`. **If a test has to change, stop and report which one and
  why.** Do not edit it to pass. The likely candidates: `a_class_with_two_inits_shows_the_second` (the analyzer asks ty for `__init__`
  `[check]` that ty gives the last one), and the parameter list of a def (`*args`, `**kwargs`, keyword-only).
- One new test in `tests/call_tree.rs`, with two new fixture files: a class in `pkg/child.py` whose base, with an `__init__` that calls
  a function, is in `pkg/base.py`. Entry: the class. Expect one child, and its `call_site` is the range of that call **in `base.py`**
  (cut the text out of `base.py` with the range, as the two-file test does).
- `cargo clippy -- -D warnings` in `drivers/python`; `grep -rn "pylspt\|ruff_" drivers/python/src drivers/python/Cargo.toml` finds
  nothing; the conformance and lifecycle tests of the root workspace pass.
- Measure and write here: build time and binary size of the driver, before and after.

Commit: `driver: python-syntax and python-analysis replace pylspt`.

## 5. Stage C: clean up

| What | When |
|---|---|
| `~/.cargo/git/checkouts/rust_python_parser-*` and its `db` entry (100 MB) | after B is merged |
| the `pylspt` folder | keep the source until `python-cli` exists (PY7): `pylspt-dev calls` and `run-jarvis.sh` have no replacement yet. Its `target/` is already gone. Before deleting it, save what is not in git there: the modified `run-jarvis.sh`, `cli-wire.json` and `fable_plan/` |
| `06-drivers.md` §7 and the other docs that name pylspt | reword when next edited; not a step |

## 6. Disk rules

On 2026-11-12 the disk was full. One build of ruff and ty is about 5 GB.

| Rule | Why |
|---|---|
| `cargo clean` in `drivers/python` **before** the toolchain changes | artifacts of two toolchains do not replace each other: 5 GB becomes 10 |
| never build in `pylspt` again | `cargo clean` was run there on 2026-11-12 (5.4 GB). If an editor has that folder open, rust-analyzer rebuilds it: close it |
| after A is pushed, `language_driver_rs/languages/python/target` (8.6 GB) may be cleaned when space is short | it is only needed while working in that repo |

## 7. What is not part of this

| Thing | Where it goes |
|---|---|
| `call_site_file` in the protocol's `CallNode`. Today the core opens a call site in the parent's `target.file`; for a call made inside an inherited constructor that is the wrong file. After B the driver sends the right **position**, and the core still opens the parent's file | its own step after B: a protocol field, the core's merge, the extension's jump |
| a cancel flag inside the analyzer | `language_driver_rs`, after TS5 |
| a bigger stack for the request thread | its own driver fix |
| a project kept open between requests | step 7.6 decides |
