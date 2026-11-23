# 23. The Bend driver

Bend is the second language. Written 2026-11-22, from the analyzer side's hand-off note and its branch `bend-plan` in `language_driver_rs`
(last fully reviewed commit `7f54b73`).

```
bend
├── 1. what has to change
├── 2. the pin
├── 3. how Bend maps onto the protocol (decided)
├── 4. steps B0 – B7
├── 5. adding a language: the registry
└── 6. to check on the way
```

## 1. What has to change

Two parts:

1. **The driver**, `drivers/bend`, built like `drivers/python`: its own Cargo workspace, a thin adapter between the wire and the analyzer
   (`bend-syntax`, `bend-analysis`). The Python driver is the template (~1,150 lines: outline, the two ID planners, the call tree, the worker).
   Bend is easier: no engine, no salsa lock to match, builds in seconds.
2. **The core runs more than one driver.** [`06-drivers.md`](06-drivers.md) §3a and §5 describe this, but it was never built because there was
   no second driver. Today it is Python in five places:

| Where | What is fixed to Python |
|---|---|
| `crates/vnoc/src/scan.rs` | `python_files`: only `.py` files are scanned |
| `crates/vnoc/src/server.rs`, `driver_host.rs` | one `driverPath`, one `DriverHost` |
| `crates/vnoc/src/cli/project.rs` | looks for `vnoc-driver-python` beside the binary |
| `extension/src/core.ts`, `extension.ts` | one driver path; the watcher is `*.py`; `vnoc.extensions` is `.py`, `.pyi` |
| `scripts/package.mjs`, `install.sh`, workflows | one driver in the VSIX, in the CLI archive, in CI |

The driver can be built and tested on its own first (B1, B2): its tests talk to the binary over stdio, as the Python driver's do. The core
work (B3, B4) comes after.

## 2. The pin

- The Bend crates come from one commit of `language_driver_rs`, by `git` + `rev`, as Python's do. All of them at the same rev
  (two revs build `analyzer-core` twice and the types do not match).
- **Pin: the tag `bend-v0.1.0` (`cdc4f42`)**, 10 commits after the first pin `7f54b73` (the last commit reviewed on the analyzer side, pushed
  first, B0). Those 10 are cost work only: the graphs are byte-identical and the API the driver uses is unchanged.
- **Never build against the checkout by path.** A coding session edits that tree, with uncommitted changes between its commits. Read the
  analyzer's code at the pin with `git -C ../language_driver_rs show cdc4f42:<path>`, not from the working tree.
- At `cdc4f42`, `analyzer_core::wire::WIRE_FORMAT_VERSION` is 2, and the toolchain is 1.99.1 (as `drivers/python`).
- The analyzer side tagged `bend-v0.1.0` (the qualname and marker rules freeze there); the driver is pinned to the tag (B7, done).

The public API the driver uses (outline, markers, the session, the call graph, the wire) has not changed shape since the analyzer's layer 1.
The value layer added three things: `Precision`, `Options::with_precision`, and `Session::graph_from`. The work still running there changes
timings, and which node some calls sit under; not the shape of the tree or of the wire.

## 3. How Bend maps onto the protocol (decided)

| Question | Answer | Why |
|---|---|---|
| `name`, `files.extensions` | `"bend"`, `["bend"]` | |
| `def f(…)`, `@unsafe def f`, a template `def twice(~f, x)` | `function` | |
| `law f:` | `function` | the wire has two kinds; a law is not built |
| `type List<a>:` | `class` | the analyzer gives it the class prefix (its D45): it is what gets built. Constructors are not defs |
| qualname | the declared name, as written, dots included (`Nat.double`); `parent` is always `""` | Bend has no nesting; the dot is a character of the name (analyzer `docs/qualnames.md`) |
| a law and its proof | the same qualname, occurrence 1 and 2 | the analyzer's D15; the marker tells them apart for good |
| a def's marker | the line `# ID: <id>` **directly above** the declaration (above `@unsafe`), below any doc comment: `MarkerStyle::above("ID")` | Bend has no docstring (analyzer `docs/markers.md`) |
| the file's marker | `# FileID: <id>` at the top, before the first `import`: read by `header_marker`, planned by `plan_header_marker`, style `above("FileID")` | as Python, the header has its own label |
| environment inputs | none: capability `environmentInputs` is false | Bend has no manifest or lock file |
| a call tree's entry | a def. Bend files have no code of their own: the analyzer answers `Entry::File` with nothing | |
| precision | `Precision::Precise` (the analyzer's default) | |
| sessions | one `Session` per (root, entry file), kept on a worker thread; `refresh()` before each request | `Session::open` takes the entry file; refresh rebuilds when a file it read changed. Only saved text is read, as for Python |
| a `cut` node, expanded | B2: the core asks for that def as a new entry (what it does today). Correct, less precise below that node. B5: continue from the node's context with `Session::graph_from` | |

## 4. Steps

One prompt at a time; the next only after the last one reports.

| Step | What | Where |
|---|---|---|
| **B0** | the user pushes `bend-plan` of `language_driver_rs`, so `7f54b73` can be fetched | the user |
| **B1** | protocol: `files.extensions` in the `initialize` result (additive; the Python driver says `py`, `pyi`). `drivers/bend` tiers 1 and 2: handshake, `vnoc/outline`, `vnoc/planIds`, `vnoc/planFileId`. Its stdio tests. CI runs it | `crates/vnoc-driver-protocol`, `drivers/python/src/main.rs`, `drivers/bend`, `ci.yml` |
| **B2** | `drivers/bend` tier 3: `vnoc/callTree` on a worker thread (sessions per root and file, refresh, cancel, deadline, a panic answers an error), `depth` → `Budget::max_depth` | `drivers/bend` |
| **B3a** | the core's driver registry (§5): discovery, `Drivers` by extension, the scan and every request routed by the file's extension, capabilities per file, `languages` in the core's `initialize` result, a crashed driver takes only its language down | `crates/vnoc` |
| **B3b** | the CLI on the registry: `--driver` repeatable, `VNOC_DRIVERS`, every `vnoc-driver-*` beside the binary; `vnoc status` lists the languages | `crates/vnoc/src/cli` |
| **B4a** | the CLI fixes from B3b; the extension with no language named (§5): it finds every driver (`bin/`, dev `drivers/*/target/debug`), sends `drivers`, builds the watcher, `vnoc.extensions` and the call-tree menu from `vnoc/languages`; the canvas's ID-line reader reads comment markers; short names from the node, not the key's last dot; the window suite opens a `.bend` file | `crates/vnoc/src/cli`, `extension`, `shared`, `webview` |
| **B4b** | packaging and CI with no language named: `package.mjs`, `install.sh`, the CLI archive, `publish.yml`, `package.yml`, `ci.yml`, dependabot loop over `drivers/*/`; third-party notices; the texts that still say one driver | `scripts`, `.github`, docs |
| **B4c** | the conformance test on every driver (06 §6), each driver's `conformance/`; the byte-order-mark fix in both drivers' `Lines` | `crates/vnoc/tests`, `drivers/*` |
| **B5** | continue a cut node from its context: an opaque `context` on `CallNode`, a `from` on `CallTreeParams` (both additive); the driver answers it with `Session::graph_from`; the core sends it when a cut node is expanded | protocol, `drivers/bend`, `crates/vnoc`, webview |
| **B6** | by-hand check on a real Bend project in VS Code and with the CLI; README (languages), CHANGELOG; the release bump list gains `drivers/bend/Cargo.toml` | README and CHANGELOG done in 0.3.1 (`release: 0.3.1`); the by-hand check is not part of this commit |
| **B7** | re-pin to `bend-v0.1.0` once tagged; release | done: pinned to `bend-v0.1.0` (`cdc4f42`) |

### As built in B1 (dc38b01 – 355ac66, reviewed 2026-11-22)

- `initialize` answers `files: { extensions: [...] }`; Python `py`, `pyi`; Bend `bend`. `callTree` and `environmentInputs` are announced
  `false` (B2 turns `callTree` on; `environmentInputs` stays false).
- `positions.rs` is a copy of Python's (no shared crate: two copies, rule of two). Occurrences are `u32` in the Bend analyzer.
- `planFileId`: `written.text` and `span` are the comment and its own line break; the blank line Bend may add after it is in the edit only.
  (Python's `written.text` is the whole inserted text.) The core reads only `span.start`, so the two agree where it matters.
- A request before `initialized` ends the driver without an answer (`lsp-server`'s `initialize_finish`), as Python's does.
- Cargo prints `error: unclosed table` on every build of the pin: a broken-manifest fixture in the analyzer repo that Cargo skips while it
  searches the repo. The exit code is 0. Harmless; it is in CI logs too.
- Left for B4: a `drivers/bend` entry in `.github/dependabot.yml`.

### As built in B2 (69787d1, reviewed 2026-11-22)

- `vnoc/callTree` on a worker thread as Python's: one `Session` per (root, entry file), `refresh()` first, opens/reopens/solves counted,
  `catch_unwind` (a panic forgets the root's sessions), test hooks `stats`, `hold`, `panic`, `deadline`; deadline 20 s; 8 MiB worker stack
  (copied from Python; a 1,500-level chain passes).
- **`depth` is the solver's budget** (`Budget::new(depth, None)`), not a render depth as for Python: a context past it is not walked at all
  and is drawn `cut`. Cheaper, and what `Session::graph` documents. The core asks with `DEPTH_CAP` 32, so the analyzer's own 1,000-level
  guard (`incomplete`) is not reached through the core.
- A session whose entry file is gone answers an internal error naming the file; it comes back when the file does.
- The analyzer's `passed` nodes (a def handed over, not called) are ordinary children: the protocol has no word for them. To revisit with B5
  if continuation-passing code reads wrong in the by-hand check.
- Call-site columns skip a leading byte-order mark (the analyzer's do; the editor never shows one). The outline's ranges do not: see §6.

### As built in B3a (d7125f0 – e31aeed, reviewed 2026-11-22)

- `DriverHost` is a list of slots; `start_all()` starts them in parallel, sorts the claims by driver name, the first claim of an extension
  wins, a later one is a warning on that slot. Every index build runs `start_all()` again: a driver that never answered its handshake is
  tried again then; one that answered and crashed later keeps its extensions and restarts on the next request for one of its files.
- **A driver that never answered its handshake claims nothing**, so its files are not scanned (its extensions are unknown). Its reason is in
  `vnoc/languages`. (§5 said "its files are shown without definitions": that holds only for a driver that crashed after its handshake.)
- A driver that crashes mid-build fails only the files it was asked for, each with the reason; before, any such error failed the build.
- Extensions compare exactly (`.PY` is not `.py`), as before. One line in `extension_of` if that is ever wanted.
- `vnoc/languages` answers `[{ name, version, path, extensions, capabilities, state: "running" | "failed", error?, warnings }]`;
  `warnings` (an extension another driver won) is kept.
- `discover`: a file named `vnoc-driver-<a-z0-9_->` plus the OS suffix, executable on Unix (so `.d`, `.pdb` in `target/debug` are not drivers).
- Start-up, server path, both drivers, two-language project: 53 ms cold, then 8 ms (Python alone 8–10 ms, Bend alone 7 ms). No cache needed.
- Short names: call-tree nodes take `Def::name` from the index; a target the index lacks falls back to the last dot. `suggest.rs` and
  `vnoc-search` `nearest.rs` cut at the last dot on purpose (both sides of a fuzzy match). The location key round-trips Bend names
  (letters, digits, `_`, dots; never `/ ? - # ::`). Left for B4: `extension/src/saveDraft.ts:81`, `addDoc.ts:42` cut a key at its last dot.
- **`.pyi`: reverted in B3b.** B1 made the Python driver announce `py` and `pyi`, so B3a's scan began reading stub files, which 0.3 never
  showed. The Python driver announces `py` only until stubs are a decision of their own.

### As built in B3b (906ec04 – 75a2c15, reviewed 2026-11-22)

- `--driver` repeatable (short help shortened to fit the 90-column test; the order is in `long_help`); then `VNOC_DRIVERS`, `VNOC_DRIVER`,
  then every `vnoc-driver-*` beside vnoc's real path. An env list replaces discovery, as a flag does.
- **A `--driver` path that does not exist is exit 3 at once**, even among two: a typed path that is wrong is the user's mistake to see.
  A path from `VNOC_DRIVERS` / `VNOC_DRIVER` is not checked first: it shows as a failed driver. A path that exists and is no driver: failed,
  the other languages work.
- Discovery follows symlinks (a linked driver beside a linked binary).
- `vnoc status`: one `driver` line per driver; JSON `drivers` rows gain `extensions`, `state`, `warnings`, `error` (failure only). Schema
  stays 1 (docs/21 C5: fields are only added); `tests/golden/status.json` changed, one-driver text did not.
- "is it Python?" in two error messages (calls.rs, attach.rs) → "does a driver read it?".
- Start-up, CLI, two drivers beside a copied binary: 1.25 s the first time (macOS checks new files), then 0.01 s.
- **Fixed in B4a:** (1) a failed driver printed a `WARN … is not used` line, twice (open's `start_all` and the build's), and the driver's
  own stderr, on a command that exits 0: against docs/21 05 §4. (2) The empty-project line `driver   not started (no Python files to read)`
  is wrong now: every driver starts, and the driver may not be Python's.

### As built in B4a (45abe69 – dce3605, reviewed 2026-11-22)

- CLI: a failed driver is started once per command, logged at `info`, its stderr piped and added to its failure reason; stderr is empty on
  exit 0. `main` records whether `VNOC_LOG` was set before it sets `warn` (`cli::project::LOG_ASKED_FOR`); when it was, drivers' stderr is
  shown. The server warns once per failed driver per `start_all`. An empty project: each driver's line says `no .<ext> files`.
- Extension: `vnoc.drivers` (array) is the setting; `vnoc.driver.path` is read as a list of one (deprecated). Found by itself: `bin/vnoc-driver-*`
  installed, `drivers/*/target/debug/vnoc-driver-*` from source (`chooseDrivers`, shared/binaryChoice.ts). `initializationOptions.drivers`;
  the CLI command line gets `VNOC_DRIVERS`.
- `vnoc/languages` after ready and after each restart → `vnoc.extensions`, the watcher glob (`watchGlob`, made again on change) and
  `vnoc.callTreeExtensions`. The call-tree menus: `resourceExtname in vnoc.callTreeExtensions` (editor), `fileExtname in …` (webview node
  menus: `nodeContext` sends the node's extension). One output line per language.
- The canvas's ID reader drops a leading `#`, `//`, `///`, `/*`, `*` from the label word. Short names in saveDraft / addDoc come from the node.
- Window suite: `extension/src/test/fixtures/bend/` is copied into the workspace as `bend/`; suite `bend` (6 tests). The skeleton suite's
  counts changed for it. Known flake seen once: the edit suite's "Save says gone" timed out in one full run.
- Left for B4b: the extension's description and `python` keyword; the first-attach prompt says "docstring" (addDoc.ts ~30), which Bend has not.

### As built in B4b (2f2b6a0 – 188f43d, reviewed 2026-11-22)

- `scripts/drivers.mjs` finds the drivers: every `drivers/<x>/` whose Cargo.toml package is `vnoc-driver-<x>`; any other folder there fails
  the script. `package.mjs` (build, `extension/bin/`, the CLI archive) and `notices.mjs` use it. `THIRD_PARTY_NOTICES.md` regenerated
  with the Bend driver's crates.
- `install.sh` installs `vnoc` and every `vnoc-driver-*` the archive holds (listed with `tar -tzf`), each by `.new.$$` + rename; a driver
  an earlier install left is kept and named (`left vnoc-driver-x from an earlier install`): it may be the user's own.
- CI: the setup action, `ci.yml`, `package.yml` loop over `drivers/*/`; one rust-cache step for every driver workspace (a computed
  list). Cost: a change to one driver's Cargo.lock invalidates the others' cache too; accepted (the locks move only on a re-pin).
  Dependabot: `directories: ["/drivers/*"]`.
- `crates/vnoc/tests/release_version.rs` (new): every `drivers/*/Cargo.toml` and `extension/package.json` carry the core's version.
- Texts: the extension's description names Python and Bend; the first attach says "an ID line in {name}'s code"; build and layout texts
  (CONTRIBUTING, README build, docs 03, 06, 19, 21/07) say "every driver". **Left for B6:** README's product claims ("Python today", 8
  places): they change when Bend is announced as supported.

### As built in B4c (eebc304 – a08d8c2, reviewed 2026-11-22)

- Each driver has `src/source.rs` (`read_text`: the core's rule, a copy per workspace) for outlines from disk and for call-site texts.
  Bend's call-site columns are exact on a BOM file. Python's analyzer reads files itself and counts the mark: a call on line 0 of a BOM
  file is one column late (pinned by a test in `drivers/python/tests/file_read.rs`, to change when the analyzer is fixed).
- A non-UTF-8 file is now read (bad bytes as U+FFFD). Three core tests that needed "a file the driver cannot read" use a file with no read
  permission instead: Unix only, skipped on Windows (the reporting code is the same on every system). If Windows coverage is wanted: a
  `VNOC_TEST_HOOKS` switch in the drivers that refuses a marked file.
- `crates/vnoc/tests/driver_conformance.rs`: one test over every driver folder, every failing check of every driver listed at the end;
  each driver has `conformance/{input.<ext>, expected-outline.json}` (checked by hand, UTF-16 columns included).

## 5. Adding a language: the registry (B3, B4)

TypeScript and Rust come after Bend (their analyzers are in `language_driver_rs` already). B3 and B4 are built so that a language is added
in one place.

**The rule: a language is a folder and a binary, and the driver says the rest about itself.** `drivers/<lang>/` builds `vnoc-driver-<lang>`;
its `initialize` answer gives `name`, `files.extensions`, `capabilities`. No other file in the repo lists languages or extensions. The one
exception is cosmetic and optional: the canvas colour table (a missing entry is plain text, never a bug).
The other exception is `activationEvents` in `extension/package.json`: one `workspaceContains:**/*.<ext>` line per claimed extension, the one place a language is listed by hand, because VS Code reads activation events before any code runs and so cannot ask the core; `driver_conformance.rs` fails with the line to add when one is missing.

| Part | How |
|---|---|
| **Where drivers are** (discovery) | An explicit list replaces discovery: `--driver <path>` (repeatable, CLI), `VNOC_DRIVERS` (paths, the OS's list separator), `initializationOptions.drivers` (extension). `driverPath` / `VNOC_DRIVER` still read, as a list of one. Otherwise: every file named `vnoc-driver-*` (plus `.exe` on Windows) in the folder of vnoc's real path: the VSIX's `bin/` and the CLI install folder both are that. The extension's development window: every `drivers/*/target/debug/vnoc-driver-*` that exists |
| **The registry** | The core starts every driver found, in parallel, and reads its handshake. `extension → driver`. A driver whose handshake has no `files` is not used (warning). Two drivers claiming one extension: the first by name wins, a warning in the log and in `vnoc status`. A driver that fails to start or crashes: its files are shown without definitions and with the reason; the other languages work; it is started again on the next request for one of its files (as today) |
| **Using it** | The scan collects every claimed extension. Every per-file request (outline, IDs, call tree) goes to the file's driver. A capability is asked per file: "call tree" on a `.bend` file is Bend's `callTree` |
| **The extension learns it from the core** | A request `vnoc/languages` answers `[{ name, version, path, extensions, capabilities, state }]` once the drivers have answered their handshakes. The watcher glob, `vnoc.extensions` (menus) and "not available for this language" come from it. The extension has no list of its own |
| **Packaging and CI** | `package.mjs`, `install.sh`, the CLI archive, `publish.yml`, `package.yml`, `ci.yml` (lint, test) and dependabot loop over `drivers/*/`. The release version test reads every `drivers/*/Cargo.toml` |
| **Conformance** | `crates/vnoc/tests/driver_conformance.rs` (06 §6) runs against every driver; each driver folder has `conformance/` (an input file with an emoji, the expected outline). A new driver passes it before it is merged |

**Start-up cost.** All drivers start when the core starts, for the handshake only. Rule for every driver: `initialize` answers without loading
the analyzer (Python and Bend already do). B3a measures the cost for a CLI command (macOS checks a new binary on its first launch, ~1 s once).
If it hurts, cache the handshake keyed by the driver's path, size and modified time; not before.

**Adding TypeScript after B4:**
1. `drivers/typescript/` from `drivers/bend` as the template; it announces `ts`, `tsx`, … and its capabilities.
2. `drivers/typescript/conformance/`.
3. Optional: a line in the canvas colour table.

CI, packaging, the core, the CLI and the extension pick it up with no change.

## 6. To check on the way

- **B3: core code that reads Python into a qualname.** A Bend name has dots that are not nesting: `Nat.double` with `parent ""` must be one node
  at the top of its file, not `double` under `Nat`. Grep the core and the webview for splitting a qualname on `.` and for `<locals>`.
- **B3: the location key** (`file::qualname#n`) with every character a Bend name may hold. Read the Bend name grammar at the pin.
- **B3/B4: per-language capabilities.** The structure answer has one `capabilities` map (the running driver's) and the extension sets
  `vnoc.callTree` from it (`extension.ts`). B3a keeps that map as the union of all drivers' for old readers; B4 decides per node from
  `vnoc/languages` and the node's file.
- **B4: the canvas's ID-line reader** (`shared/idLine.ts`) is a port of Python's docstring reader (quotes stripped around the words). A
  Bend `# ID: x` line reads as `ID` + `x` because `#` is its own word; `#ID: x` (legal for the analyzer: blanks are optional) does not.
  The reader must come from the language: B4 decides how (the driver reports each def's marker span in the outline, or a per-language rule).
- **BOM and non-UTF-8 (B4c).** The core reads a file with a leading byte-order mark dropped and bad bytes as U+FFFD (`source.rs`), and
  strips the mark before it sends text for an ID plan (`identity/edits.rs`). A driver reading `root` + `file` for an outline used
  `read_to_string`: a BOM file's line 0 was off by one column, and a non-UTF-8 file failed. Fix: each driver reads a file as the core does
  (lossy, the mark dropped), checked by the conformance test.
- **B4: colours.** `.bend` is plain text in the canvas first; a Monarch grammar is a later, separate step.
- **Windows:** relative paths with `/`, `dunce` and not `std::fs::canonicalize`, test paths built piece by piece
  ([`19-ci-cd.md`](19-ci-cd.md)). Each step is pushed and CI runs on all three systems before the next one.
