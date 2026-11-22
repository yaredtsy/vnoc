# 23. The Bend driver

Bend is the second language. Written 2026-11-22, from the analyzer side's hand-off note and its branch `bend-plan` in `language_driver_rs`
(last fully reviewed commit `7f54b73`).

```
bend
├── 1. what has to change
├── 2. the pin
├── 3. how Bend maps onto the protocol (decided)
├── 4. steps B0 – B7
└── 5. to check on the way
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
- **First pin: `7f54b73`**, the last commit reviewed on the analyzer side. It must be on GitHub first: `bend-plan` was 41 commits ahead of its
  remote (B0).
- **Never build against the checkout by path.** A coding session edits that tree, with uncommitted changes between its commits. Read the
  analyzer's code at the pin with `git -C ../language_driver_rs show 7f54b73:<path>`, not from the working tree.
- At `7f54b73`, `analyzer_core::wire::WIRE_FORMAT_VERSION` is 2, and the toolchain is 1.99.1 (as `drivers/python`).
- When the analyzer side tags `bend-v0.1.0` (the qualname and marker rules freeze there), re-pin to the tag (B7).

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
| **B3** | the core runs several drivers (06 §5): one per extension from `files.extensions`; the scan collects every claimed extension; each request goes to the file's driver; `outline_text` is told the file; `initializationOptions.drivers` (a list; `driverPath` still read); a crashed driver takes only its language down. CLI: every `vnoc-driver-*` beside the binary, `--driver` given more than once. May split into B3a (core) and B3b (CLI) | `crates/vnoc` |
| **B4** | the extension and packaging: both drivers resolved (dev: `drivers/bend/target/debug`), the watcher and `vnoc.extensions` include `.bend`, the canvas shows `.bend` as plain text, the VSIX and the CLI archive carry `vnoc-driver-bend`, `install.sh`, `publish.yml`, `package.yml`, third-party notices | `extension`, `scripts`, `.github` |
| **B5** | continue a cut node from its context: an opaque `context` on `CallNode`, a `from` on `CallTreeParams` (both additive); the driver answers it with `Session::graph_from`; the core sends it when a cut node is expanded | protocol, `drivers/bend`, `crates/vnoc`, webview |
| **B6** | by-hand check on a real Bend project in VS Code and with the CLI; README (languages), CHANGELOG; the release bump list gains `drivers/bend/Cargo.toml` | |
| **B7** | re-pin to `bend-v0.1.0` once tagged; release | |

## 5. To check on the way

- **B3: core code that reads Python into a qualname.** A Bend name has dots that are not nesting: `Nat.double` with `parent ""` must be one node
  at the top of its file, not `double` under `Nat`. Grep the core and the webview for splitting a qualname on `.` and for `<locals>`.
- **B3: the location key** (`file::qualname#n`) with every character a Bend name may hold. Read the Bend name grammar at the pin.
- **B4: colours.** `.bend` is plain text in the canvas first; a Monarch grammar is a later, separate step.
- **Windows:** relative paths with `/`, `dunce` and not `std::fs::canonicalize`, test paths built piece by piece
  ([`19-ci-cd.md`](19-ci-cd.md)). Each step is pushed and CI runs on all three systems before the next one.
