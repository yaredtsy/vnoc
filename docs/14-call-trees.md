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
├── 9. what is not part of this
├── 10. a call site names its file (step 7.7)
└── 11. measurements (step 7.6)
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
| **T1** | The driver returns the **wire tree** (`python_analysis::call_wire`, since [`15-leave-pylspt.md`](15-leave-pylspt.md); it was `pylspt::interp::call_wire`): one node per call site. The graph is not requested, stored or sent. | The tree is what is drawn. The graph has no user yet. |
| **T2** | **The core merges** siblings. Not the driver, not the extension. | It is V-NOC policy, not Python knowledge (06 §7's rule). One function serves every driver, and the CLI / MCP paths get the same tree. pylspt already assumes it: `call_count` is "always 0 from pylspt". |
| **T3** | A merged node keeps **every call site**, as `call_sites: [Range]`. | `merge_for_output` keeps only the first; the canvas needs all of them to jump. |
| **T4** | The merge key is the target's **ID when it has one, else `(file, qualname, occurrence)`**. Never the qualname alone. | Two functions with one qualname in two files must stay two nodes. |
| **T5** | `require_ids` is **off**. | In V-NOC only nodes with a doc carry an ID; with the filter on, the tree would be almost empty. |
| **T6** | The driver opens the project **per request** at first. No cache. | Simplest. Step 7.6 measured it (§11): **kept for 0.2**, no cache. |

## 3. Who does what

```
webview ◄─ extension ◄── core (vnoc) ◄────────── drivers/python ◄──────── python-analysis
draws      command,      merges siblings (T2)    1-based → 0-based         call_wire, Frame:
the tree   relays        maps targets to nodes   occurrence 0-based → 1    file, qualname,
                         many requests, cancel   reads call_site_file      occurrence (D20)
                                                 (python-syntax: outline, markers)
```

## 4. pylspt: what the wire needs

> **Replaced by [`15-leave-pylspt.md`](15-leave-pylspt.md) §4** (2026-11-12): the driver no longer uses pylspt, and the fields this section asks of it
> are `Frame`'s own in `language_driver_rs` (E1–E5). What follows is the history of the request.

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
        "call_site_file": "src/app.py",
        "marks": ["recursive"],
        "children": [] } ] } }
```

| Field | Rule |
|---|---|
| `target.file` | relative to `root`, `/` separators. **`null` when the def is outside `root`** (library code): the core shows it, and never looks it up. |
| `target.id` | the ID text as written in the docstring, without the `FunctionSchema/` / `ClassSchema/` prefix. `null` when empty. The core classifies it. |
| `call_site` | where the call is written, in `call_site_file`. 0-based line, UTF-16 `character`: convert from pylspt's 1-based line and character-counted column with the existing `positions.rs` helpers, reading that file's text. `null` on the root, and for a call written in a file outside `root` (§10.3). |
| `call_site_file` | the file `call_site` is in: relative to `root`, `/` separators. The parent's `target.file` for an ordinary call, the base class's file for a call made in an inherited constructor. Absent on the root, and from a driver older than 7.7 (§10.3). |
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
| which `__init__` | the analyzer's constructor lookup: a class is an entry of `call_wire` (E5), its root frame is the class, and its children are what the constructor calls, inherited ones included (the call site is then in the base class's file: `call_site_file`, §10). The driver picks nothing. |
| no constructor in project code | the root with **no children**, not an error. |
| `depth` | applies to the `__init__` tree as it does for a function; the class root is not counted. |
| `vnoc/expand` | a class node expanded is the same request: its entry is that class. |

**Cancel.** The driver already runs each request on its own thread with a cancel flag, and drops the answer of a cancelled request. That is
enough for 0.2. Also set `Limits::deadline` to 20 seconds so a runaway solve ends by itself; the tree then carries `incomplete` marks.

**Files**

| File | Change |
|---|---|
| `crates/vnoc-driver-protocol/src/lib.rs` | `CallTreeParams`, `CallEntry`, `CallTreeResult`, `CallNode`, `CallTarget`, `CallMark`. `Serialize` + `Deserialize` on all. |
| `drivers/python/src/call_tree.rs` | new: open the project (`python_analysis::Project::open`), `call_wire` with the default options and a 20 s deadline; convert `Frame` → `CallNode` (15 §4.3; it was `pylspt::db::open_project` and `FrameJson`) |
| `drivers/python/src/main.rs` | the `method::CALL_TREE` arm; `("callTree", true)` |
| `drivers/python/tests/call_tree.rs` | new, with a fixture project of three files |
| `drivers/python/src/positions.rs` | `Lines::position_1based`: pylspt's 1-based line and character column → the wire's position |
| `drivers/python/Cargo.toml` | `python-syntax`, `python-analysis`, `analyzer-core`, all at one `rev` of `language_driver_rs` (15 §4.1). No `ruff_*` line: `Project::open` takes a plain path, so the `ruff_db` line this row once asked for is gone |
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
- A timeout **keeps the driver**: the cancel has been sent, and other requests (a call tree) may be running on it. Only `Closed`, `Io` and `Protocol`
  drop it. The price: a driver that hangs for good is not restarted by timeouts alone; each request just times out. Nothing hangs in 0.1's
  `outline`; if 7.6 finds a hang, add a restart after a few timeouts in a row.
- The limit covers writing the request as well as waiting for the answer. A request that cannot be written in time (the driver has stopped
  reading its input, the pipe is full) is `Closed`, not `Timeout`: it drops the driver, since nothing else would end one that is stuck.
- `forget` drops a driver only if it is still the current one: a late failure of an old driver must not drop its replacement.
- `DriverHost::call_tree` exists (capability `callTree`, 30 s) so the 30 s limit has a caller; the core's `vnoc/callTree` that uses it is 7.4.
- The handshake now has the 10 s limit too. New error `HostError::Closed` for a driver that is gone.

### 6.2 The merge

`crates/vnoc/src/calls.rs`, a pure function over the driver's tree: no I/O, no index.

| Rule | |
|---|---|
| key | `id` when present, else `(file, qualname, occurrence)` (T4). Two targets with `file: null` merge on `(qualname, occurrence)`. |
| siblings with one key | one node: `call_count` = how many there were, `call_sites` = every site (a file and a range, §10.4) by file and then in source order, `marks` = the union |
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
  "call_sites": [CallSite],                     each a Range plus the "path" it is written in (§10.5)
  "marks":      ["recursive"],
  "children":   [CallTreeNode]
}
```

- The core resolves `key` to `(file, qualname, occurrence)`, asks the driver, merges, then maps each target with `Index::node_at`.
- `revision` is the index revision the mapping was made against; the extension drops an answer older than the structure it shows.
- A key that is not a function or class → `INVALID_PARAMS`. A driver without the `callTree` capability → an error that says so.

Built in `crates/vnoc/src/calls.rs` (the merge), `protocol.rs` (`CallTree`, `CallTreeNode`) and `server.rs` (`call_tree`). What the text leaves open, as built:

- **The driver gets the opened folder as `root`**, not the git root, and the entry's path below it. That is the project the user sees and what its imports
  resolve against (`from lib import x` in `services/api/` works). Its paths come back relative to the folder, so `path` is the driver's `target.file`
  as is, and the key is the folder's prefix plus that path (the index's keys start at the git root).
- **`key` is set when `Index::def_at` finds the target**, not `node_at`: `node_at` answers only for a documented node, and every def in the tree has a key.
- A key without `::` (a file or a folder), one that does not parse, or one the index does not have → `INVALID_PARAMS`, as the driver's `no function …`
  is. A driver without `callTree` → `REQUEST_FAILED` with the host's message.
- **Merge:** a site that two merged parents both give (their children are the same calls of one body) is listed **once**, in source order (the key is the file and the range, §10.4); `call_count`
  still counts the calls (2). Without that the "who calls" menu would list one place twice. An `id` that is not an ID is no ID (T4 falls to
  the place). One ID in two places (a copied function) merges, as T4 says.
- A request the editor cancels is dropped by `ConcurrencyLayer`, which drops the driver request under it (7.3 sends the cancel). Not tested end to end
  here: it needs a slow driver.

## 7. Extension and canvas

> **Replaced by [`16-ui-redesign/05-calls-as-nodes.md`](16-ui-redesign/05-calls-as-nodes.md) (2026-11-12):** calls become cards and sidebar rows
> opened one level at a time, the host fetches what is open, and there is no notification and no Refresh. What follows is what 7.5 built.

Kept small on purpose: the first version draws the tree. The EXPLORE contexts of `10-ui-rework/03` come after it works.

| Piece | Change |
|---|---|
| command | `vnoc.showCallTree` "Show Call Tree", on function and class nodes (tree row menu, card menu). Enabled only when the driver has `callTree`. |
| extension | sends `vnoc/callTree`, shows progress with a Cancel button (cancel drops the request: the core then cancels the driver), relays the result to the canvas |
| canvas | the entry's card gets a "Calls" section: the merged tree as indented rows, `name ×N` when `call_count > 1`, a mark badge (`↻ recursive`, `✂ cut`, `… incomplete`) |
| a row | click selects that node when `key` is set · a row with `key: null` is grey and does nothing · the row's menu lists its call sites, each opens the file the site names, at that range |
| stale | when `vnoc/indexChanged` arrives, the section shows "the code changed · Refresh" and does not re-run by itself |

Built in `extension/src/callTree.ts`, `commands.ts`, `webview/src/node-card/NodeCalls.tsx`, `canvas/calls.ts`, `state/calls.ts`. What the table leaves open, as built:

- **"Enabled only when the driver has `callTree`"**: `vnoc/structure` now carries `capabilities` (the running driver's, from its handshake), and the
  extension sets the context key `vnoc.callTree` from each structure. The row and the card menu test it; the palette does not offer the command.
- **Where the trees live**: in the extension (`CallTrees`, the latest per entry, 50 at most), not in the session: they are large and mean nothing after
  a restart. A canvas that opens later asks for them (`callTrees`); an open one hears each new one (`callTree` event).
- **The card**: the section is under the body, before the footer, ten rows high at most (more scroll inside it); the layout makes the card that
  much taller (`callsHeight`). An entry that calls nothing in the project still gets the section, with one line saying so. The root's own marks
  are in the section's header.
- **A row's menu** is a "N sites" toggle on the row that lists the call sites as `line N` buttons, not VS Code's right-click menu: that menu is
  static (package.json), and the list is not. A site is labelled `line N` in the parent's file and `name.py:N` in another (§10.5); a site with no file is not listed, and a row with none has no toggle.
- **A row click is `select`**, as written: the node may not be drawn on the canvas; the doc panel and the sidebar follow the selection.
- **Stale** is computed in the page: the tree's `revision` is older than the structure's. Refresh runs the same request again without moving the canvas.
- Cancel is the progress notification's button: `sendRequest` with its token sends `$/cancelRequest`, the core drops the handler, 7.3 cancels the driver.

## 8. Order of work

Each row is one commit on `0.2-call-trees` and leaves the branch green. `main` takes 0.1 fixes only; merge `main` into the branch after each.

| # | Build | Proof |
|---|---|---|
| **7.1** | pylspt: the three checks and the four fields of §4 · push · the new `rev` in the driver | `pylspt-dev calls … --wire` shows the new fields on a two-file fixture · the driver still builds and its tests pass |
| **7.2** | the protocol types and the driver's `vnoc/callTree` (§5) | `drivers/python/tests/call_tree.rs` passes |
| **7.3** | the host: reader task, per-method timeout, cancel (§6.1) | the three host tests · the existing lifecycle test still passes |
| **7.4** | the merge and the core's `vnoc/callTree` (§6.2, §6.3) | the merge tests · a lifecycle test: the method on a fixture project returns a merged, mapped tree |
| **7.5** | the command and the "Calls" section (§7) | in the window suite: the command on a fixture function shows the rows · by hand on this repo's Python fixtures |
| **7.6** | measure the new analyzer (`python-analysis`, 15 §4) on its own tests folder (`language_driver_rs`) and on httpx: time of the first and of a second request on one entry, with **opening the project timed apart from the walk** | the numbers written here; decide T6 (cache or not) and whether `expand` needs anything more than a new `callTree`. A cache follows the steps of [`04-storage/02-index.md`](04-storage/02-index.md) §6.4 in order: keep the project open in the driver, memo dropped on any change → the core saves merged trees in SQLite → the analyzer saves summaries. Take a step only when the one before it is measured too slow |

7.1 is in the pylspt repo and comes first: 7.2 cannot be tested without it.

## 9. What is not part of this

- The graph, in any form.
- A cache across requests, unsaved-buffer text for call trees, SQLite: decided by 7.6.
- EXPLORE contexts and "Explore in New Context" (`10-ui-rework/03`): after 7.5, in its own plan.
- Callers ("who calls this").

## 10. A call site names its file (step 7.7)

Written 2026-11-12 from reading the code at `499bee6`. **Built in step 7.7**, as written; what the code settled that the text left open is at the end of this section. It is the step [`15-leave-pylspt.md`](15-leave-pylspt.md) §7 put off.

### 10.1 What is wrong

Every hop says a call site is "in the parent node's file". That is false for a call made in an inherited constructor:

```python
# pkg/base.py
class Base:
    def __init__(self):
        helper()          # the site is here

# pkg/child.py
class Child(Base):        # the parent node is Child, in child.py
    pass
```

The analyzer says which file (`Frame.call_site_file`), and since 15 §4.3 the driver reads the position in that file. Then the driver drops the
name: `CallNode` has no place for it. The canvas opens `pkg/child.py` at a line of `pkg/base.py`.

It is also false in the merge: two copies of one function with one ID, in two files, merge into one node (T4). Their children are called
from two files, and `call_sites` keeps one `Range` for both (`dedup` by range).

### 10.2 The rule

**A site is a file and a range, on every hop. Nothing after the core works out a site's file from the tree.**

| Hop | Today | After |
|---|---|---|
| driver protocol, `CallNode` | `call_site: Option<Range>` | plus `call_site_file: Option<String>` |
| merge, `Merged` | `call_sites: Vec<Range>` | `call_sites: Vec<Site>`, `Site { file: Option<String>, range: Range }` |
| core protocol, `CallTreeNode` | `call_sites: Vec<Range>` | `call_sites: Vec<CallSite>`, a range with a `path` |
| `shared/messages.ts` | `call_sites: Range[]` | `call_sites: CallSite[]`, `interface CallSite extends Range { path: string \| null }` |
| canvas | opens `row.parentPath` | opens `site.path` |

### 10.3 The driver protocol

```rust
pub struct CallNode {
    pub target: CallTarget,
    /// where the call is written; `null` on the root
    pub call_site: Option<Range>,
    /// the file `call_site` is in: relative to `root`, `/` separators. Absent: the parent node's `target.file` (a driver from before 7.7).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub call_site_file: Option<String>,
    …
}
```

- The Python driver fills both from one place (the `zip` in `Convert::node`), so a site never has a position from one file and the name of
  another. `call_site_file` is the analyzer's string as is: it is already relative to the root with `/`, on Windows too (EW).
- A site in a file outside `root` (an absolute path): the driver sends `call_site: null` **and** no file. So "a site with no file" has one
  meaning only, an older driver. The analyzer enters project code only, so this is a guard; say in the report whether a test could reach it.
- No capability and no version changes: the field is optional, and a core from before 7.7 ignores it (serde drops unknown fields).

**Rust note.** `#[serde(default)]` lets the field be missing when reading; `skip_serializing_if = "Option::is_none"` leaves it out when
writing `None`. Together they make the field additive: old JSON still parses and `None` adds no bytes.

### 10.4 The merge

`Site { file, range }` in `calls.rs`.

| Rule | |
|---|---|
| a site's file | the node's `call_site_file`; when absent, the `target.file` of **that node's own parent** in the driver's tree. Resolved before grouping: after a merge by ID the parents of two children can be in two files |
| a node with no `call_site` | no site (as today) |
| the same site twice | listed once. The key is `(file, range)`, no longer the range alone |
| order | by file, then by position. Inside one file that is source order, as today; the order of two files means nothing and is only stable |
| the root | no sites (as today) |

This is the one place the fallback lives. `file` is `None` only when the node gave none and its parent is outside the root.

### 10.5 The core's answer, and the canvas

```rust
/// One place a node is called.
pub struct CallSite {
    /// relative to the opened folder, as `CallTreeNode.path`; `null` when the file is not under it
    pub path: Option<String>,
    #[serde(flatten)]
    pub range: Range,
}
```

JSON: `{ "path": "pkg/base.py", "start": {…}, "end": {…} }`.

**Rust note.** `#[serde(flatten)]` writes the fields of `range` beside `path` instead of under a `"range"` key. So a site is still read as a
range (`site.start.line`), and code that only wants the position does not change.

- `path` is the driver's file as is, for the reason `CallTreeNode.path` is (§6.3: the driver's root is the opened folder).
- **Canvas.** A row lists the sites whose `path` is not `null`; with none, no toggle. A site opens `openAt({ path: site.path, range })`.
  The label is `line 12` when the site is in the file of the row's parent, else `base.py:12` (the file's name), so a jump to another file
  is not a surprise. `CallRow.parentPath` stays, for the label only.

### 10.6 Tests

Order (a signature change does not fail a test, it stops the crate compiling): **(1)** add the field and the two types, with every site's
file taken from the parent as today, and make the mechanical edits below: everything green, behaviour unchanged. **(2)** add the tests,
see them red. **(3)** the driver fills the field, the merge prefers it and keys on `(file, range)`: green.

| # | Where | Test | Red in (2) because |
|---|---|---|---|
| a | `vnoc-driver-protocol` | a `CallNode` JSON without `call_site_file` parses to `None`; a node with `None` writes no such key | green from (1): it pins the additive shape |
| b | `drivers/python/tests/call_tree.rs` | entry `Child` (`pkg/child.py`): the child's `call_site_file` is `pkg/base.py`. Entry a plain function: its child's is the entry's file. The root has none | the driver sends none |
| c | `calls.rs` | a node with `call_site_file: b.py` under a parent in `a.py`: the site's file is `b.py` | the merge takes the parent's |
| d | `calls.rs` | a node without the field: the site's file is its parent's | green from (1): it pins the fallback |
| e | `calls.rs` | two siblings with one ID, in `a.py` and `b.py`, each with one child `helper` at the same range and no field: `helper` has **two** sites, one per file | `dedup` by range leaves one |
| f | `crates/vnoc/tests/lifecycle.rs` | a copy of `drivers/python/tests/fixtures/call_tree/` in a temp folder (there `Base.__init__` in `pkg/base.py` calls `a()` from `pkg/calc.py`; `class Child(Base)` is in `pkg/child.py`): `vnoc/callTree` on `pkg/child.py::Child` gives `a` one site whose `path` is `pkg/base.py` | the path is the child's file |
| g | `webview/src/canvas/calls.test.ts` | the label of a site in the parent's file is `line N`, in another file `name.py:N`; a site with `path: null` is not listed | the label and the filter do not exist |

(f) needs the driver built from this tree (`cd drivers/python && cargo build`): the lifecycle test runs that binary.

**Mechanical edits to existing tests, allowed in (1). Anything else: stop and report.**

- every `CallNode { … }` literal gains `call_site_file: None`
- `calls.rs`: an expected `call_sites` value becomes sites in the parent's file (one helper, `sites(file, [..])`)
- `calls.test.ts`: the `call_sites` of its fixture nodes gain a `path`

The lifecycle helper that reads `start.line` of a site, the window suite and the driver's existing tests do not change: if one has to,
that is a finding.

### 10.7 Docs in the same commit

§5 (the `CallNode` shape), §6.2 (the merge table), §6.3 (`call_sites` in the JSON), §7 (the row's sites), the comments in
`shared/messages.ts` and `protocol.rs` that say "the parent's file", and 15 §7's first row ("done in step 7.7": a commit cannot name its own hash; the hash goes in with the next commit that touches the file).

### 10.8 As built

What the text leaves open, as built in 7.7:

- **The guard of §10.3 is in `Convert::node`** (`drivers/python/src/call_tree.rs`), and `Callers::read` does not read such a file. No test reaches it: `Frame` is
  `#[non_exhaustive]` and has a `Drop`, so a test outside the analyzer cannot build one, and a symlinked module under the root is named by its project path
  (`ext.py`), not by its real one. The analyzer reports project files only, as §10.3 says.
- **The order** of two files in `call_sites` is `Option<String>` order: a site with no file first, then by file name.
- **The merge** carries the parent's file down with each call (`Call { node, parent_file }`), so the fallback is worked out before grouping, as §10.4 says.
- **The webview**: `siteLabel` is unchanged (`line N`); `siteText(site, parentPath)` adds the file's name, and `listedSites(node)` is the filter. The jump sends the site's
  `start` and `end` as the range, not the site, so `path` does not travel as a part of a range.
- `vnoc-driver-protocol` has `serde_json` as a dev-dependency, for test (a).

## 11. Measurements (step 7.6)

Taken 2026-11-12 with `drivers/python/examples/measure_call_tree.rs` (`cargo run --example measure_call_tree -- <root> <file::qualname>… | --sweep`),
which times `Project::open` apart from `call_wire`, and with two small scripts that send real requests (to the driver, and to the core) and time
the answers. Analyzer: `python-analysis` at the pinned rev. Machine: an Apple-silicon Mac.

**What was measured, and what that leaves out**

- **Debug profile of the driver, dependencies at `opt-level 2`** (the driver's own `[profile.dev.package."*"]`). Not the release binary: a release build
  needs about 5 GB more and the disk had 6 GB. The analyzer and ty are the same speed in both; the driver's own conversion and JSON are slower here than in release.
- **No Python environment was set, and none was found**: no `.venv` in the project, `VIRTUAL_ENV` unset, and the analyzer does not configure one (ty's own
  discovery runs). Which interpreter ty then used was **not checked**. `httpcore`, `certifi`, `anyio` and the rest were library code either way: the tree
  drops it. A project whose environment resolves can be slower; none of these numbers say by how much.
- Two projects: `language_driver_rs/languages/python/crates/python-analysis/tests` (64 files, 569 functions, small fixtures) and `encode/httpx`
  (shallow clone, 60 files, 1,134 functions).

**One request, with opening the project apart from the walk** (ms; "2nd walk" is the same entry again on the *same* open project; "fresh again" is
what the next request costs the driver today: a new project, then the walk)

| Project and entry | nodes (unmerged) | open | 1st walk | 2nd walk | fresh again |
|---|---:|---:|---:|---:|---:|
| tests: `chains.py::fanout` | 19 | 1.7 | 31.6 | 0.5 | 25.8 |
| tests: `callbacks.py::uses_stdlib` | 1 | 0.7 | 25.0 | 0.4 | 25.5 |
| tests: `annotated.py::Handler.handle` | 1 | 0.7 | 17.5 | 0.1 | 17.9 |
| httpx: `_api.py::request` | 3,083 | 4.2 | 288 | 26 | 237 |
| httpx: `_api.py::get` | 3,084 | 1.5 | 247 | 26 | 407 |
| httpx: `_client.py::Client.request` | 1,588 | 0.9 | 190 | 19 | 195 |
| httpx: `_client.py::Client.send` | 1,038 | 0.9 | 189 | 17 | 237 |
| httpx: `_client.py::AsyncClient.send` | 1,038 | 0.9 | 200 | 17 | 179 |
| httpx: `_urls.py::URL.copy_with` | 112 | 0.8 | 63 | 3.6 | 62 |
| httpx: `_models.py::Response.json` | 1 | 0.9 | 39 | 1.1 | 35 |

Every def of a project, one after the other on **one** open project (`--sweep`): the tests folder, 569 functions in 152 ms (median 0.14 ms, slowest 26 ms,
the first, which pays the start-up); httpx, 1,134 functions in 9.7 s (median 3.6 ms, p90 26 ms, p99 34 ms, slowest 224 ms, the first big one).

**A request through the real driver, and through the real core** (httpx; the first and the second request in one process)

| Entry | driver alone: 1st / 2nd, response | core: `vnoc/callTree` 1st / 2nd, merged nodes, response |
|---|---|---|
| `_api.py::request` | 356 / 333 ms, 3,083 nodes, 653 KB | 557 / 395 ms, 995 nodes, 296 KB |
| `_client.py::Client.send` | 227 / 218 ms, 1,038 nodes, 220 KB | 259 / 259 ms, 373 nodes, 107 KB |
| `_api.py::get` | | 417 / 399 ms, 996 nodes, 296 KB |
| `_models.py::Response.json` | | 40 / 41 ms, 1 node |

The core's first build of httpx (60 files, outline of each) took 141 ms.

**What the numbers say**

1. **Opening is not the cost.** `Project::open` is 1–4 ms: the project is read lazily. The cost is the first walk on a project: about 17–30 ms for a
   small one (a floor that the smallest entries also pay), and 190–290 ms for httpx's biggest entries.
2. **The second request is not faster today**, because every request opens a new project (core: 557 then 395 ms; driver: 356 then 333). The memo
   that a kept-open project holds would make the walk about 10 times faster (httpx `request`: 288 → 26 ms; `Client.send`: 189 → 17 ms).
3. **A kept-open project would not make the request 10 times faster.** About 100–150 ms of the 395 ms is not the walk: the driver converting and
   writing 3,083 nodes (653 KB, in a debug build), and the core merging them, mapping keys and writing 296 KB. On the biggest entry, step 1 of the cache
   (`04-storage/02-index.md` §6.4) takes 395 ms to roughly 150–200 ms; on a typical entry (`Client.send`, `copy_with`) the request is already 60–260 ms.
4. **The merge does its job**: 3,083 calls become 995 rows (`_api.py::request`), 1,038 become 373.

**Decisions**

- **T6 stays: no cache for 0.2.** The slowest request measured is 0.56 s end to end, with a progress notification and a Cancel button on screen, and
  most are under 0.3 s. The rule is to take a cache step only when the step before is measured too slow, and none of these is. The first step, a
  project kept open in the driver with the memo dropped on any change, is the one to take if a real project is slow. What would make it worth taking:
  a request on a project that resolves its environment (not measured), or a release build that is still over a second on a big entry.
- **`expand` needs no more than a new `callTree`** whose entry is the callee: its cost is that callee's own (112 nodes: 62 ms; 1,038 nodes: 190–260 ms),
  and `render_depth` does not make the walk cheaper (the solver walks the whole function either way), only the answer smaller. So a depth-limited first
  request plus an expand would save bytes, not time; nothing in the driver or the core changes.
- **A gap, not a measurement**: §1 lists "expand a node that was cut". 7.5 shows the `✂ cut` badge, but a row has no button that runs Show Call Tree on
  its node (the menu on the node's own card does the same). The core never sends `depth` today, so a cut appears only at the node budget (100,000). Add the
  row button when something sends `depth`.
