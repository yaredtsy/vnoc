# 00.01 — Lessons from the Python backend

Before designing the new system, we write down what went wrong in the old one.
Every lesson below turns into a **design rule** (in bold at the end of each section).
Later files point back to these rules.

Paths are relative to `~/Documents/Programing/ide/v-noc/src/backend/app/`.

---

## L1 — Groups are not invisible to sync

A *group* is a container that exists only in the graph, not on disk. There are
three kinds today: `StructureGroupSchema` (holds folders/files),
`CodeElementGroupSchema` (holds classes/functions) and `CallGroupSchema` (holds calls).

What happens on resync:

- `core/parser/graph_builder/discovery/change_detector.py` builds "who is my
  parent in the DB" from `FolderNode.children_by_type["folder_children" | "file_children"]` only.
  It never follows the `structure_group` edge.
- So a file inside a group has **no parent** in the DB map, but its parent on disk is its folder.
- `db_parent != current_parent` → the file is treated as **moved** → it gets
  re-attached to its folder → **the group is emptied**.
- The code-element side (`collection/ast_processor.py::_build_existing_map`) has the
  same bug in a different form: grouped *methods* are pulled out, grouped
  *top-level functions* survive only by accident.
- The call side (`call_graph/diff_calulator.py::_flatten_calls_skipping_groups`) does it
  **right**. It skips groups when comparing. So the codebase has three different answers to one question.

**Rule R1: there is exactly one definition of "logical parent" (the parent with groups skipped), and every comparison uses it. Better still, store it, so nobody has to compute it.**
→ [`01-data-model/03-edges-parent-and-owner.md`](../01-data-model/03-edges-parent-and-owner.md)

---

## L2 — No root node

Top-level folders and files have **no parent at all**. There is a special
`FolderSchema/init` (`core/model/schemas/structure_schema.py::INIT_FOLDER_ID`), but it
is only a "global theme/document" holder, and the change detector explicitly
filters it out.

Why this hurts:

- "Give me the top-level items" becomes "give me every folder/file that **no one** points to".
  In a graph DB that is a negation (`not(triple(_, folder_children, X))`), and negation means a **full scan**.
- Every tree walk has two cases: "has a parent" and "is top-level". Two code paths means twice the bugs.
- `is_root` is stored as the **string** `"true"`/`"false"` to work around a TerminusDB boolean issue. That's a smell.

**Rule R2: every project graph has one `Root` node. Every node has a parent, except Root. "Top level" means `parent == Root`, which is a fast indexed lookup.**

---

## L3 — A move across batches is seen as delete + create

The Python pipeline processes files in batches and **commits per batch**. Take a
function `parse()` that the user cut from `a.py` and pasted into `z.py`:

```
batch 1: a.py → parse() is gone      → delete Function/parse   (commit)
batch 2: z.py → parse() is here, new → create Function/parse   (commit)
```

Its stable ID (the docstring `ID:`) is the same, so it *could* have been a move. But
by the time batch 2 sees it, batch 1 has already deleted the node, along with
its documents, theme and group membership.

The same thing happens with folders and files when a sync only sees part of the
change (for example a watcher event that covers only the "delete" side).

**Rule R3: nothing is deleted while a sync is still running. Removals are collected, compared against everything that appeared during the *whole* sync, and only the true leftovers are tombstoned (soft-deleted). Hard delete happens later, in garbage collection.**
→ [`05-orchestrator/06-moves-and-tombstones.md`](../05-orchestrator/06-moves-and-tombstones.md)

---

## L4 — The call tree is fully expanded, so one change has a huge blast radius

`core/parser/jedi_adapter/call_resolver/call_resolver.py` follows each call **into the
callee's body** (`_analyze_function`), guarded only by an ancestor check. So
`FunctionA`'s stored tree contains `A → B → C → …`. That's what the UI shows, and
it's useful, but:

- If `C` changes its calls, **every tree that contains `C` anywhere** is stale.
- The Python code only re-diffs the trees of functions **in changed files**, so the others go stale silently.
- There's no fingerprint to answer "did `C`'s calls change, or just its body?".
  An edit to a comment inside `C` costs as much as adding a new call.
- Call node IDs are random UUIDs (`diff_calulator.py`: `CallSchema/{uuid4()}`), so the only way to diff is to load the old tree and match by target.

**Rule R4: store the direct call facts once (layer 1) and derive the expanded tree from them (layer 2). Give tree nodes deterministic IDs and subtree hashes, so "nothing changed" costs one hash comparison and "something changed" rewrites only that branch.**
→ [`05-orchestrator/07-call-graph-sync.md`](../05-orchestrator/07-call-graph-sync.md)

---

## L5 — The DB is used in the most expensive way

From the refactor analysis (`v-noc/plan/parser-orchestrator-refactor/01-current-state-analysis.md`):

- **~49 places create a commit.** One resync creates many commits. TerminusDB commits
  are *layers* (like git objects), and more layers means slower reads until you optimise.
- **Read-modify-write updates.** To add one child, the repo loads the whole parent
  document, edits a set in Python and writes the whole document back. That's two
  round trips, and it rewrites triples that didn't change.
- **Children are stored as five different sets on the parent** (`class_children`,
  `function_children`, `call_children`, `code_element_group`, `call_group`). A move
  rewrites two parents. Every type has its own field names (`child_raw.py` is a whole
  module just to map names).
- **Full-document snapshots for change detection** (`get_all(FileSchema)`) when only
  `{id, path, hash, parent}` is needed.
- **No layer maintenance.** Nothing ever calls `optimize`.

**Rule R5: one sync = one logical commit. The link to the parent lives on the child (one field), so a move is one small write. Snapshots read only the fields they need. Optimise after big syncs.**
→ [`03-storage/05-terminus-best-practices.md`](../03-storage/05-terminus-best-practices.md)

---

## L6 — Storage leaks everywhere

WOQL is built in services, repositories, the parser and the API. `BaseRepo` is a
~450-line class that mixes CRUD, path queries and move logic. Subclasses
override with incompatible signatures. Errors are `print(e); return None`.

**Rule R6: the DB is behind traits. No crate except `vnoc-store-terminus` knows that WOQL exists. Errors are typed values, never prints.**
→ [`03-storage/01-store-traits.md`](../03-storage/01-store-traits.md)

---

## L7 — ID injection writes to user files while *reading*

`read_or_inject_file_id` runs for **every scanned file on every sync**, even unchanged
ones. It may *write* to the file (to add a `FileID`), and that write can trigger the
watcher again, which starts another sync. `read_or_inject_folder_id` even **creates an
`__init__.py` in every folder**.

We are keeping this ID approach (decision D5), but we fix *when* and *how* it runs:

**Rule R7: reading an ID never writes. Writing IDs is a separate, explicit step ("assign IDs to these new things"), runs only for things that need one, and its writes are announced to the watcher so they don't trigger a new sync. All of it sits behind an `IdStrategy` trait so the approach can change later.**
→ [`04-drivers-and-ids/04-id-strategy.md`](../04-drivers-and-ids/04-id-strategy.md)

---

## L8 — The watcher fights the sync

`core/watcher/service.py`: file events throw away *which* paths changed (every save
means a full rescan), the watch is paused during a sync (so events are lost), and
temp files and git-ignored paths trigger syncs.

**Rule R8: events are filtered, keep their paths, and are coalesced into a "sync scope". There is one serialized sync queue per project, and events that arrive during a sync mark the project dirty so it syncs again. Nothing is dropped.**
→ [`05-orchestrator/09-scheduler-and-watcher.md`](../05-orchestrator/09-scheduler-and-watcher.md)

---

## What the Python code got right (keep it)

- **Stable IDs in source** survive renames and moves made outside the IDE. That's why we keep them (D5).
- **ID-first change detection** (`change_detector.py`): compare by ID first and by path second. Correct idea.
- **Skipping groups during call diffing** (`_flatten_calls_skipping_groups`). We generalise it.
- **Content stored separately from the file node** (`CodeContentSchema`), so loading the tree doesn't load the code.
- **Topological insert order** for calls (`call_insert_order.py`). We won't need it: with parent-on-child
  links and one commit, insert order inside the commit doesn't matter. But the reason it existed is worth knowing.

---

## Exit criteria for this file

You can say each rule R1–R8 in one sentence and point to the file that implements it.

Next: [`02-decision-log.md`](02-decision-log.md)
