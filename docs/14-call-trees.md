# 14. Release 0.2: call trees

Written 2026-11-12, while the 0.1.0 publish run waits on GitHub. It is the build plan for 0.2, on the branch `0.2-call-trees`. The driver side
builds on [`06-drivers.md`](06-drivers.md) §4 and §7; the UI side on [`10-ui-rework/03-focus-and-contexts.md`](10-ui-rework/03-focus-and-contexts.md).
Where this file is more exact, it wins.

```
call trees
├── 1. what 0.2 ships
├── 2. decisions
├── 3. who does what
├── 4. pylspt: what the wire needs
├── 5. driver: vnoc/callTree
├── 6. core: the host, the merge, the method
├── 7. extension and canvas
├── 8. order of work
└── 9. what is not part of this
```

## 1. What 0.2 ships

Pick a function, run "Show Call Tree", and see what it calls, as a tree of nodes on the canvas. Each node is one function or class; a node the
project knows (it is in the STRUCTURE tree) opens, shows its docs and jumps to its code like any other node.

| In | Out (later) |
|---|---|
| the tree of one entry function, merged | the context graph (`call_graph`): not used anywhere |
| jump to a call site | editing on the canvas (0.3) |
| cancel a running request | a cache between requests, unless step 7.6 measures a need |
| expand a node that was cut | "who calls this" (callers) |

## 2. Decisions

| # | Decision | Why |
|---|---|---|
| **T1** | The driver returns the **wire tree** (`pylspt::interp::call_wire`): one node per call site. The graph is not requested, stored or sent. | The tree is what is drawn. The graph has no user yet. |
| **T2** | **The core merges** siblings. Not the driver, not the extension. | It is V-NOC policy, not Python knowledge (06 §7's rule). One function serves every driver, and the CLI / MCP paths get the same tree. pylspt already assumes it: `call_count` is "always 0 from pylspt". |
| **T3** | A merged node keeps **every call site**, as `call_sites: [Range]`. | `merge_for_output` keeps only the first; the canvas needs all of them to jump. |
| **T4** | The merge key is the target's **ID when it has one, else `(file, qualname, occurrence)`**. Never the qualname alone. | Two functions with one qualname in two files must stay two nodes. |
| **T5** | `require_ids` is **off**. | In V-NOC only nodes with a doc carry an ID; with the filter on, the tree would be almost empty. |
| **T6** | The driver opens the project **per request** at first. No cache. | Simplest. Step 7.6 measures it; a cache is added only if the numbers ask. |

## 3. Who does what

```
webview ◄─ extension ◄── core (vnoc) ◄────────── drivers/python ◄──────── pylspt
draws      command,      merges siblings (T2)    1-based → 0-based         call_wire
the tree   relays        maps targets to nodes   module path → repo path   + file, qualname,
                         many requests, cancel   strips `FunctionSchema/`    occurrence (§4)
```

## 4. pylspt: what the wire needs

`FrameJson` today (rev `670ae79a15`) names a target by `target_qname` (module-qualified, as `sem.qname` gives it) and `target_id`
(`FunctionSchema/<id>`, empty with no ID). The core cannot find a node from that: it looks nodes up by **file + `__qualname__` + occurrence**
(`LocationKey`), the same three values the outline gives.

Add to `FrameJson`, filled in `solver.rs` `frame_json` and `render.rs` `render_root`:

| Field | Value |
|---|---|
| `target_file: Option<String>` | the def's file, as an absolute path. `None` when there is no file on disk (a builtin). |
| `target_qualname: String` | the def's `__qualname__` with `<locals>`, exactly as the outline function returns it |
| `target_nth: u32` | 1-based, among definitions with that qualname in that file, in source order: the outline's occurrence |
| `target_kind` | `function` or `class` (the solver already knows: `sem.is_class`) |

Keep `target_qname`, `target_id`, `call_count`, `contexts`: the parity differ and `pylspt-dev` use them. `merge_for_output` stays as it is.

**Check before coding, and write the answers into this section:**

- [x] **`entry: &str`** (read in `Sem::entry`, `src/sem.rs`; run on the fixture below). It splits on `.` and walks down through class and function
      bodies by name, so `Service.run` and `Service.go.inner` work. It cannot name every def: `outer.<locals>.inner` finds nothing (the
      outline's qualname has `<locals>`); a def under an `if` / `try` is not seen (only the statements of the body itself are searched); two defs
      with one qualname give no way to pick (the first one wins for the last name; the last one wins for a name in the middle); a class is never an entry (the last name must be a `def`).
      → **Added** `Sem::entry_at(file, qualname, nth)` and `interp::call_wire_at(db, file, qualname, nth, opts)`: `qualname` is the outline's
      `__qualname__`, `nth` is the outline's occurrence (1-based, source order). `call_wire` and `--entry` stay as they were.
      `pylspt-dev calls … --entry <qualname> --nth <n>` reaches it. A class is still not an entry of `call_wire_at`: the driver builds a class's tree from its `__init__`'s calls (§5).
- [x] **Code outside the project** is **dropped**, not in the wire. `sites_union` skips `Target::External` and `Unresolved`, and `shown()` skips any
      def whose file is not in the project. On the fixture, `Store.save` calls `os.getcwd()` and has `children: []`. So the driver's
      `target.file: null` (§5) cannot come from pylspt today: it can only happen for a project that includes files outside the root. The driver's
      test states "library calls are not in the tree". `target_file` is `None` only for a def with no system path (a typeshed stub, a builtin).
- [x] **`call_site` is in the caller's file.** Confirmed on a two-file fixture (`experience/12-call-tree/05-output/python/wire_target/`):
      `Store().save()` in `caller.py` line 8 gives the node `Store` (a class, in `callee.py`) with `call_site` line 8, columns 9–16 of
      `caller.py`, and the test cuts `Store()` out of that line of `caller.py` with those columns. Line and column are **1-based**
      (`call_site.start = [line, column]`, a pair, not an object): the driver converts to 0-based and UTF-16.

Things found while checking, for 7.2:

- A constructor call `Store()` is **one node of kind `class`** (its children are what `__init__` calls, if the class has one; the fixture's class has none,
  so this was not run). `Store().save()` is two calls at two sites, so the fixture's children are `Store`, `Store.save`, `twin`.
- `contexts[].func.Path.nth` (the existing `stable_def`) is **0-based** and keyed by the dotted path without `<locals>`. The new `target_nth` is
  **1-based** and keyed by the `__qualname__` with `<locals>`. They are different fields on purpose; the driver reads only the new one.
- With two `def twin` (one under an `if`), a call to `twin()` resolves to the **second**: `target_nth: 2`.

Then: commit, push, and put the new hash in the four `rev =` lines of `drivers/python/Cargo.toml`.

## 5. Driver: `vnoc/callTree`

Capability `callTree` becomes `true`. `vnoc/expand` is **not** a separate implementation: it is `callTree` with the callee as the entry, so 0.2
only adds `callTree`.

**Request**

```json
{ "root": "/abs/project", "file": "src/app.py", "entry": { "qualname": "Service.run", "occurrence": 1 }, "depth": 6 }
```

`depth` is optional and shortens the output only (`Limits::render_depth`).

**Response**

```json
{ "root": {
    "target": { "file": "src/app.py", "qualname": "Service.run", "occurrence": 1, "kind": "function", "id": null },
    "call_site": null,
    "marks": [],
    "children": [
      { "target": { "file": "src/db.py", "qualname": "Db.save", "occurrence": 1, "kind": "function", "id": "abc123" },
        "call_site": { "start": { "line": 11, "character": 8 }, "end": { "line": 11, "character": 25 } },
        "marks": ["recursive"],
        "children": [] } ] } }
```

| Field | Rule |
|---|---|
| `target.file` | relative to `root`, `/` separators. **`null` when the def is outside `root`** (library code): the core shows it, and never looks it up. |
| `target.id` | the ID text as written in the docstring, without the `FunctionSchema/` / `ClassSchema/` prefix. `null` when empty. The core classifies it. |
| `call_site` | in the caller's file (the parent node's `target.file`). 0-based line, UTF-16 `character`: convert from pylspt's 1-based line and character-counted column with the existing `positions.rs` helpers, reading the caller's text. `null` on the root. |
| `marks` | `recursive`, `cut`, `incomplete`, as pylspt's `Mark` serialises them |
| not sent | `call_count` (the core counts), `contexts`, `target_qname` |

**Errors:** no function and no class with that qualname and occurrence in the file → `InvalidParams` with the message `no function <qualname> in <file>`. A file that cannot be read →
`InternalError`, as `outline` does.

**A class as the entry.** pylspt's `call_wire_at` takes functions only (7.1 check), so the driver builds this one itself. It finds the def of
`(qualname, occurrence)` in the file's outline; when it is a class, the response is:

```
Service                  the class: kind "class", call_site null
└── …                    what Service.__init__ calls (no node for `__init__` itself: the class stands for it)
```

| Rule | |
|---|---|
| which `__init__` | the last `def __init__` in the class's own body (the one Python runs; earlier ones are `@overload`s or replaced). It is run as an ordinary `call_wire_at(<qualname>.__init__, nth)`; its root is dropped and its children become the class's children, with their `call_site`s as pylspt gave them. The root's marks (`cut`, `incomplete`) are copied to the class root. |
| no `__init__` in the class body | the root with **no children**, not an error. An inherited `__init__` is not followed in 0.2. |
| `depth` | applies to the `__init__` tree as it does for a function; the class root is not counted. |
| `vnoc/expand` | a class node expanded is the same request: its entry is that class. |

**Cancel.** The driver already runs each request on its own thread with a cancel flag, and drops the answer of a cancelled request. That is
enough for 0.2. Also set `Limits::deadline` to 20 seconds so a runaway solve ends by itself; the tree then carries `incomplete` marks.

**Files**

| File | Change |
|---|---|
| `crates/vnoc-driver-protocol/src/lib.rs` | `CallTreeParams`, `CallEntry`, `CallTreeResult`, `CallNode`, `CallTarget`, `CallMark`. `Serialize` + `Deserialize` on all. |
| `drivers/python/src/call_tree.rs` | new: open the project (`pylspt::db::open_project`), `call_wire` with `Mode::Summary`, `require_ids: false`; convert `FrameJson` → `CallNode` |
| `drivers/python/src/main.rs` | the `method::CALL_TREE` arm; `("callTree", true)` |
| `drivers/python/tests/call_tree.rs` | new, with a fixture project of three files |
| `drivers/python/src/positions.rs` | `Lines::position_1based`: pylspt's 1-based line and character column → the wire's position |
| `drivers/python/Cargo.toml` | `ruff_db` (same `rev`), only to turn a path into the `SystemPath` that `open_project` takes: the fourth `ruff_*` line the facade (06 §7) will remove |
| `drivers/python/tests/fixtures/call_tree/` | the fixture project: `pkg/calc.py`, `pkg/db.py`, `pkg/app.py` |
| `drivers/python/tests/outline.rs` | its handshake test no longer says `callTree: false` (the new test checks `true`) |

**Tests** (through the real binary, as `tests/outline.rs` does)

| Case | Expect |
|---|---|
| `a()` calls `b()` twice and `c()` once | three children, in source order: the driver does **not** merge |
| caller and callee in two files | `target.file` of each is its own file; `call_site` is a range in the caller's file |
| a non-ASCII character before the call on the same line | `character` counts UTF-16 units |
| a method and a nested def as the entry | both resolve |
| direct recursion | the inner node has the `recursive` mark and no children |
| a def with a docstring `ID:` | `target.id` is that text, no prefix |
| a class with `__init__` as the entry | root is the class; its children are what `__init__` calls, in source order; no node named `__init__` |
| a class with no `__init__` | the root, no children |
| a class with two `__init__` defs | the children are those of the second |
| an unknown entry | `InvalidParams` |
| `initialize` | `callTree: true` |

## 6. Core

### 6.1 The host: many requests at once

Today `DriverClient` holds one `tokio::sync::Mutex<Io>` and every request waits for the one before it, with a 10 second timeout. A call tree
would block every outline behind it. Build the reader-task pattern of [`06-drivers.md`](06-drivers.md) §4 "0.2: many requests at once":

| Piece | Rule |
|---|---|
| writer | stdin behind a mutex held only while one frame is written |
| reader | one task reads every frame and wakes the request with that `id` through a `oneshot` |
| timeout | per method: 10 s for `outline`, `planIds`, `planFileId`; 30 s for `callTree` |
| cancel | when the caller's future is dropped or times out, send `$/cancelRequest { id }` and remove the waiting entry |
| a late or unknown `id` | ignored |
| the driver dies | every waiting request gets `HostError`; the next request starts a fresh driver (the `forget` path of today) |

Tests: with a fake driver script, two requests answered in reverse order each get their own answer; a timeout sends `$/cancelRequest`; a dead
driver fails all waiting requests.

Built in `crates/vnoc/src/driver_host.rs`. What the table leaves open, as built:

- The fake driver is **in the test process**, over `tokio::io::duplex` pipes (`DriverClient::connect` takes any pair of pipes), not a script: no
  Python or shell is needed, so the tests are the same on all four CI systems. The tests are in that file.
- A request is dropped from `waiting` and cancelled by one rule: when its future ends with the entry still there. That covers a timeout and a
  caller that went away. Each frame is written by its own task, so a caller that goes away halfway never leaves half a frame in the pipe.
- A timeout **still drops the driver**, as before (`forget`): the plan does not say otherwise. With call trees in flight, one timed-out
  `outline` then fails them too. If that hurts in 7.4, make `Timeout` keep the driver (the cancel has been sent) and only drop on `Closed`,
  `Io` and `Protocol`.
- `forget` drops a driver only if it is still the current one: a late failure of an old driver must not drop its replacement.
- `DriverHost::call_tree` exists (capability `callTree`, 30 s) so the 30 s limit has a caller; the core's `vnoc/callTree` that uses it is 7.4.
- The handshake now has the 10 s limit too. New error `HostError::Closed` for a driver that is gone.

### 6.2 The merge

`crates/vnoc/src/calls.rs`, a pure function over the driver's tree: no I/O, no index.

| Rule | |
|---|---|
| key | `id` when present, else `(file, qualname, occurrence)` (T4). Two targets with `file: null` merge on `(qualname, occurrence)`. |
| siblings with one key | one node: `call_count` = how many there were, `call_sites` = every site in source order, `marks` = the union |
| their children | concatenated, then merged the same way, recursively |
| order | the order of each key's first appearance |
| the root | `call_count: 1`, `call_sites: []` |

Tests: the table above, one test per row, plus: same qualname in two files stays two nodes; a merge three levels deep.

### 6.3 The method the extension calls

`vnoc/callTree` on the core's own protocol (`crates/vnoc/src/protocol.rs`).

```
request   { "key": "src/app.py::Service.run" }           the node's `id` from vnoc/structure, as vnoc/attach takes it
response  { "revision": 41, "root": CallTreeNode }

CallTreeNode {
  "key":        "src/db.py::Db.save" | null,    the structure id, when the target is in the index; null for library code
  "name":       "save",
  "qualname":   "Db.save",
  "path":       "src/db.py" | null,             relative to the opened folder, as in vnoc/structure
  "kind":       "function" | "class",
  "call_count": 2,
  "call_sites": [Range],                        in the PARENT node's file
  "marks":      ["recursive"],
  "children":   [CallTreeNode]
}
```

- The core resolves `key` to `(file, qualname, occurrence)`, asks the driver, merges, then maps each target with `Index::node_at`.
- `revision` is the index revision the mapping was made against; the extension drops an answer older than the structure it shows.
- A key that is not a function or class → `INVALID_PARAMS`. A driver without the `callTree` capability → an error that says so.

## 7. Extension and canvas

Kept small on purpose: the first version draws the tree. The EXPLORE contexts of `10-ui-rework/03` come after it works.

| Piece | Change |
|---|---|
| command | `vnoc.showCallTree` "Show Call Tree", on function and class nodes (tree row menu, card menu). Enabled only when the driver has `callTree`. |
| extension | sends `vnoc/callTree`, shows progress with a Cancel button (cancel drops the request: the core then cancels the driver), relays the result to the canvas |
| canvas | the entry's card gets a "Calls" section: the merged tree as indented rows, `name ×N` when `call_count > 1`, a mark badge (`↻ recursive`, `✂ cut`, `… incomplete`) |
| a row | click selects that node when `key` is set · a row with `key: null` is grey and does nothing · the row's menu lists its call sites, each opens the parent's file at that range |
| stale | when `vnoc/indexChanged` arrives, the section shows "the code changed · Refresh" and does not re-run by itself |

## 8. Order of work

Each row is one commit on `0.2-call-trees` and leaves the branch green. `main` takes 0.1 fixes only; merge `main` into the branch after each.

| # | Build | Proof |
|---|---|---|
| **7.1** | pylspt: the three checks and the four fields of §4 · push · the new `rev` in the driver | `pylspt-dev calls … --wire` shows the new fields on a two-file fixture · the driver still builds and its tests pass |
| **7.2** | the protocol types and the driver's `vnoc/callTree` (§5) | `drivers/python/tests/call_tree.rs` passes |
| **7.3** | the host: reader task, per-method timeout, cancel (§6.1) | the three host tests · the existing lifecycle test still passes |
| **7.4** | the merge and the core's `vnoc/callTree` (§6.2, §6.3) | the merge tests · a lifecycle test: the method on a fixture project returns a merged, mapped tree |
| **7.5** | the command and the "Calls" section (§7) | in the window suite: the command on a fixture function shows the rows · by hand on this repo's Python fixtures |
| **7.6** | measure on pylspt's own tests folder and on httpx: time of the first and of a second request on one entry | the numbers written here; decide T6 (cache or not) and whether `expand` needs anything more than a new `callTree` |

7.1 is in the pylspt repo and comes first: 7.2 cannot be tested without it.

## 9. What is not part of this

- The graph, in any form.
- A cache across requests, unsaved-buffer text for call trees, SQLite: decided by 7.6.
- EXPLORE contexts and "Explore in New Context" (`10-ui-rework/03`): after 7.5, in its own plan.
- Callers ("who calls this").
