# 22.06 The core

```
core
├── 1. the module
├── 2. the library functions      what the server and the command line both call
├── 3. the requests               list · presets · suggest · new · update
├── 4. running, on the wire       run · output · ended · stop · runs
├── 5. the index                  a count per node
├── 6. creating: the attach flow, split
└── 7. what stays out of the core
```

P1 (`08` §1c): everything that decides is here. The extension draws lists, a form and a text box; `vnoc play` parses arguments and prints.

## 1. The module

```
crates/vnoc/src/play/
├── mod.rs        the functions of §2
├── recipe.rs     play.toml ⇄ Recipe (toml_edit, unknown keys kept) · the lookup of 02 §4 · placeholders · the word split · the rules of 02 §7
├── presets.rs    the built-in rows (04 §1) · reading [play.language.*] from vnoc.toml · the tools and their searches
├── store.rs      list · create · update, in the node folder (through userdata: layout, atomic)
├── run.rs        place · start · stream · stop · clean (03)
└── sweep.rs      the lock files, the leftovers (03 §5)
```

`recipe.rs` and `presets.rs` touch no disk and start no process: they take text and give values, so their tests are tables. New dependencies:
none expected for PG1 – PG2 (`toml_edit` is there); PG3 may add `libc` (and a Windows crate) for the process group, decided by check 2.

## 2. The library functions

```rust
// what a list shows: never fails for one bad playground
pub struct PlayInfo {
    pub slug: String,
    pub title: String,
    pub language: String,
    pub description: Option<String>,
    pub author: Option<String>,
    pub script: PathBuf,                       // absolute: what Edit Script opens
    pub recipe: PathBuf,                       // absolute: play.toml
    pub plan: Result<PlanShown, String>,       // dir and the command's words for this machine, or the one sentence why not
}

pub fn list(project, userdata, node: &NodeId, tools: &Tools) -> Vec<PlayInfo>
pub fn presets(settings) -> Vec<Preset>                                   // built-in rows with vnoc.toml's changes and additions
pub fn suggest(project, index, key: &str, language: &str) -> Suggestion   // dir (by anchor), file, run, the starter text
pub async fn create(env: attach::Env<'_>, new: NewPlay) -> Result<Created, PlayError>      // §6
pub fn update(userdata, node, slug, change: Change) -> Result<PlayInfo, PlayError>         // title, dir, file, run, timeout, description; None = remove the key
pub fn plan(project, userdata, node, slug, tools) -> Result<RunPlan, PlanError>            // 03 §2
pub async fn run(plan: RunPlan, events: impl FnMut(Event), stop: StopSignal) -> Ended      // 03 §3 – §8
pub fn sweep(project) -> Vec<PathBuf>                                     // what it removed
```

- **`plan` is separate from `run`.** Everything that can be refused is refused in `plan`, which touches nothing; `run` takes a plan and
  cannot be handed a recipe. The panel, `vnoc play show`, and a later "check every playground still runs" all stand on `plan`.
- `Event` is `Output { stream, text }`; `Ended` is 03 §10's row.
- `run` does not know about LSP or a terminal: the server turns events into notifications, the command line writes them to stdout and
  stderr.

## 3. The requests

All take the node's permanent ID (`Node::node_id`), as `vnoc/docs` does, except `playNew` and `playSuggest`, which take a tree key because
the node may have no ID yet (as `vnoc/attach`).

| Method | Params → Result | Notes |
|---|---|---|
| `vnoc/plays` | `{ id }` → `PlayInfo[]` | sorted by title. A broken playground is in the list with its sentence |
| `vnoc/playPresets` | → `Preset[]` | `name`, `ext`, `file`, `run`, `note`: the form's language list and its placeholders' defaults |
| `vnoc/playSuggest` | `{ key, language }` → `{ dir, file, run, dirs }` | `dirs`: the folders worth offering in "Run in" (the suggestion, the node's own folder, the root), so the picker has no logic |
| `vnoc/playNew` | `{ key, title, language, dir, file?, run?, script?, markers?, document? }` → `{ id, slug, script, idInCode, notice }` | `markers` and `document` exactly as `AttachParams`. `script`: the text, else the preset's starter |
| `vnoc/playUpdate` | `{ id, slug, change }` → `PlayInfo` | |

Removing has no request: 07 §5.

## 4. Running, on the wire

| Method | Kind | Shape |
|---|---|---|
| `vnoc/playRun` | request | `{ id, slug, tools? }` → `{ run }` (a number, unique in this core's life). Answers when the process has started, or with the error of steps 1 – 5 |
| `vnoc/playOutput` | notification, core → editor | `{ run, stream: "out" \| "err", text }` |
| `vnoc/playEnded` | notification, core → editor | `{ run, how, code, ms, cut, left }` |
| `vnoc/playStop` | request | `{ run }` → nothing. Stopping a run that has ended is not an error |
| `vnoc/playRuns` | request | → `[{ run, id, slug, state: "running" \| Ended, output }]`: every run in progress, and the last one of each playground since the core started |

- **`playRuns` is for a page that starts or reloads** while something runs, or after it: the panel asks once and draws the state. The
  output it returns is what the core kept: the same 1 MB at most per run, and only the last run of each playground (G14). A core that
  restarts forgets it, and that is fine.
- Output chunks are coalesced: at most one notification per run every 50 ms, so a loop that prints a million lines does not flood the pipe
  the tree and the search also use. **[measure]** PG3: a script printing 100,000 lines; the window stays usable.
- A run is a spawned task (`tokio::spawn`), not something a request handler awaits: the server keeps answering `vnoc/structure` and
  `vnoc/search` while it runs. The state it shares is one new field on the server's `State`, `runs: Mutex<HashMap<u64, RunHandle>>`
  (a plain mutex: it is held for a map lookup, never across an `await`).
- Shutdown: the core stops every run it holds (03 §7).

## 5. The index

A node's card and row say how many docs it has; they will say how many playgrounds.

- `userdata/read.rs` `NodeRecord` gains `plays: Vec<String>` (the slugs: folders under `play/` that hold a `play.toml`), and they are part
  of its equality, as `docs` are. So creating or removing a playground moves the index revision and the pages redraw; **editing a script or
  a recipe does not** (the names did not change), exactly as editing a doc's body does not (20, Q2).
- `index::Node` and the wire's `Node` gain `plays: usize`; `MissingNode` gains `plays` beside `docs` (a Missing node's playgrounds wait with
  its docs, and Relink brings both).
- The watcher already covers `.vnoc/**`: a playground an agent created from a terminal appears in the open window.

## 6. Creating: the attach flow, split

Today `identity::attach` does: which ID → the marker → `node.toml` → `create_doc`. A playground needs the first three and a different fourth.

```rust
// identity/attach.rs
pub async fn ensure_node(env: &mut Env<'_>, key: &str, markers: Option<bool>, document: Option<Document>)
    -> Result<Ensured, AttachError>                 // Ensured { id, id_in_code, notice, hint }
pub async fn attach(env, params) -> Result<AttachResult, AttachError>     // = ensure_node, then userdata.create_doc: behaviour unchanged
// play/store.rs
pub async fn create(env, new) = ensure_node, then the folder, play.toml, the script (each with create_new: a taken slug becomes -2)
```

`ensure_node` on a node that already has a folder writes nothing and returns its ID. The first-attach question ("write IDs into the code?")
is the same question, asked by the same code in the extension (`addDoc.ts`), so it moves to a helper both flows call.

This is the same function doc 21's A7 gives an `EditSink`. The two changes do not conflict in meaning (one splits the end off, the other
swaps who applies the marker's edit), but they touch the same lines: the second to land rebases on the first. The command line's
`vnoc play new` needs both.

## 7. What stays out of the core

| | Where | Why |
|---|---|---|
| opening the script in a tab; saving it before Run | the extension | editor acts |
| moving a playground's folder to the system trash | the extension (`workspace.fs.delete` with `useTrash`) after the core says the path is a playground (`store::is_play`, as `is_doc`) | the system trash is the editor's to reach; and it is the one destructive act, kept where a person confirms it |
| the interpreter the Python extension has selected | the extension, passed as `tools` | another extension's state (later: 10 §2) |
| Workspace Trust | the extension tells; the core enforces (05 §2) | — |

Next: [`07-on-screen.md`](07-on-screen.md)
