# 05.07 — Step 8: the call graph, in two layers

> ★ The hard part. The question it answers: **"function `B` changed. Which stored
> call trees are now wrong, and what is the smallest write that fixes them?"**, including
> the case where the answer is **"none"** because `B`'s body changed but its calls didn't.

---

## 1. Why one change spreads

V-NOC shows, for every function, its **expanded** call tree. pylspt follows calls into
callee bodies, like Jedi did (`call_resolver.py::_analyze_function`):

```python
def a():  b(JsonWriter())        # a's tree:  a ─► b ─► JsonWriter.write
def b(w): w.write()              # b's tree:  b ─► (w unknown: nothing)
def c():  b(CsvWriter())         # c's tree:  c ─► b ─► CsvWriter.write
```

Three things to notice:

1. `b`'s **body** appears inside `a`'s and `c`'s trees. Edit `b` and the trees of `a` and `c` can change.
2. What `b` calls **depends on who calls it** (`JsonWriter` vs `CsvWriter`). pylspt models this as
   **contexts**: `b` with key `(w = JsonWriter)` and `b` with key `(w = CsvWriter)` are different
   nodes of its graph (`pylspt/plan/03-call-tree/14-output.md`).
3. pylspt analyses **every function as its own entry** (`16-entries.md`): `a`'s tree is computed by
   solving `a`, which walks into `b` in `a`'s context. No caller's data reaches `b`'s *own* entry.

## 2. The two layers (D4)

```
LAYER 1: facts (source of truth, small, deduplicated)
  CallContext   key = (func, digest)  →  edges: [site → target context | construct | external | unresolved]
                                          edges_hash
  EntrySummary  entry = a             →  root_context, reaches = {b, JsonWriter.write}, tree_hash,
                                          contexts = {…keys used}

LAYER 2: the view the UI shows (derived, big, user data can attach)
  Call nodes    id = hash(owner, target)       e.g.  Call/h(a, b)  →  Call/h(Call/h(a,b), JsonWriter.write)
                subtree_hash (Merkle), call_count, contexts, depth, marks
```

- **Layer 1** is what pylspt returns (`GraphOut`: contexts + edges) plus the `reaches` reverse
  index. It's enough to **re-render layer 2 without running the analyser** (useful for the
  lazy-materialisation option O3, and for rebuilding trees after a schema tweak).
- **Layer 2** gets deterministic IDs (01-data-model/02) and **Merkle hashes**: a node's
  `subtree_hash` = hash(target, call_count, sorted children's subtree_hashes). Equal hashes
  mean an equal subtree, so we don't look further down.

## 3. The propagation algorithm

Input from step 6/7: `bodies_changed`, new functions, tombstoned functions.

```
A. AFFECTED ENTRIES  (one DB query)
   changed  = bodies_changed ∪ new_functions ∪ tombstoned_functions
   affected = changed_functions_still_active
            ∪ entries_reaching(changed)                 # reverse lookup on EntrySummary.reaches
   (first import: affected = all functions)

B. RE-SOLVE  (compute only, no DB)
   graphs = driver.call_graphs(affected)                # pylspt, warm salsa, parallel, windowed
   for each entry e: render tree, compute ids + subtree_hashes bottom-up → new_tree_hash(e)

C. CUT-OFF  ★ the "did the calls actually change?" check
   for each entry e:
       if new_tree_hash(e) == stored EntrySummary(e).tree_hash:
           → NO layer-2 writes for e            ← "B's body changed but no tree containing B changed"
       else:
           → e goes to D
   layer 1: upsert CallContexts whose edges_hash differs (or are new);
            upsert EntrySummary(e) only if reaches/tree_hash/contexts changed

D. MERKLE DIFF  (level by level, batched across all entries in D)
   level = [(entry e, owner = e, new children of e)]
   while level not empty:
       old_rows = call_level(owners of level)           # ONE query per depth level, all entries together
       for each (owner, new_children):
           old = old_rows[owner] by id
           for n in new_children:
               match old.get(n.id):
                   Some(o) if o.subtree_hash == n.hash → skip (don't descend)
                   Some(o)                             → patch o (count, contexts, marks, hash); push n's children to next level
                   None (or tombstoned)                → create/resurrect n AND its whole new subtree (no more reads needed)
           for o in old not in new → remove subtree(o)  (hard delete, or tombstone if it has user data)
       level = next level
```

### Why step C is the answer to "identify this one case"

You asked for the case **"re-sync the callers only if the call actually changed"**. There are two
separate costs, and the design treats them differently:

| Cost | When we pay it | How big |
|---|---|---|
| **re-solving** (CPU, in-process) | for every entry that *can* be affected (A) | bounded by salsa caching; no I/O |
| **writing** (DB, commits, layers) | only for entries whose tree hash *actually* changed (C → D), and inside those only for the changed branches | usually tiny |

The DB never sees the edit to `b`'s comment, the rename of a local variable, or a reformat.
Those change `body_hash` (so `b`'s callers are re-solved), but their tree hashes come out the
same, so step C stops them. Only an edit that changes what is called changes a hash, and then
step D rewrites only the branches under the call nodes whose hashes differ.

### Worked example

```
stored:  a ─► b ─► JsonWriter.write          c ─► b ─► CsvWriter.write
edit 1:  in b, rename a local variable
   A: bodies_changed={b}; affected={b} ∪ entries_reaching(b)={a,c} → {a,b,c}
   B: re-solve a, b, c
   C: all three tree_hashes equal → ZERO writes. (b's CallContexts: edges_hash equal → no upsert)

edit 2:  in b, add a call  log(w)
   A/B: same
   C: a, b, c tree hashes all differ
   D: level 1 of a: Call/h(a,b): hash differs → patch, descend
      level 2 under Call/h(a,b): JsonWriter.write unchanged (hash equal, skip); log is new → create
      … same for c and b.
   writes: 3 creates (one `log` node per tree) + 6 small patches (hash/count). One commit.
```

## 4. Groups and user data on calls

- **Call groups are invisible to the diff**: `call_level(owner)` selects by `owner`, so a call node
  inside a `Group[Call]` is found the same as an ungrouped one (08-groups.md). Python got this
  right with `_flatten_calls_skipping_groups`; v2 gets it from the data model.
- **IDs survive re-analysis**: same (owner, target) means the same `Call/<hash>`, so the node keeps
  its group, docs and theme.
- **Removed subtree with user data**: tombstoned, not deleted. If the call comes back (it's the same
  deterministic ID), it's resurrected with its docs and group.

## 5. Limits and size

Expanded trees can be very large, which is why pylspt has `render_nodes` (default 100k) and
`max_contexts`. For storage:

- 🟡 **Proposed per-entry node budget: 5,000** materialised call nodes (configurable). Deeper parts
  get a `Truncated` mark. The UI shows "…more" and can ask for an on-demand expansion (O3, deferred).
- `depth` is stored so the UI can page by depth.
- Statistic to watch in M8: total call nodes per project. If it's ≫ 10× the number of functions,
  turn on lazy materialisation (O3): store layer 1 only and render branches on request. The sync code
  stays the same; step D runs only for branches the user has opened.

## 6. Layer-1 housekeeping

- `CallContext` docs are shared between entries. A context no longer referenced by any
  `EntrySummary.contexts` is garbage. Removed by the **GC job** (mark and sweep), not by the sync,
  so the sync never has to compute reference counts.
- `EntrySummary` of a tombstoned function is deleted in the same plan (its call nodes are removed
  or tombstoned per rule 6 of 01-data-model/04).

## 7. Which things are entries?

Every Function and Class (as Python did: `body_parser.py::_traverse_and_collect` walks each
class/function scope), **plus each File's module-level code** (Python included the file scope
too). **[check]** pylspt's entry API for module-level code (`16-entries.md` covers functions,
methods, properties, closures). If it's missing, add a "module entry" to pylspt in M7.

## 8. Further optimisation (only if M7 numbers say so)

Re-solving every entry that reaches a hot utility function is the main cost. If it's too slow:

1. **Order affected entries by depth from the change**, solve the changed function first, and stop
   early when its own contexts' (edges, returns) are unchanged for every key the DB knows. **[check]**
   this needs a pylspt API to re-solve a stored context key directly.
2. **Persist pylspt summaries** keyed by `StableContextId` + the body hashes of what they reach (pylspt's
   planned "v1.1 cache", `14-output.md` "Stable identity"). Then re-solving `a` reuses `b`'s
   unchanged summaries even across process restarts.

Both are pure performance work. The sync algorithm and the stored model don't change.

---

## Exit criteria (M7 layer 1, M8 layer 2)

- Worked example above as a scenario test: edit 1 → **0** call writes; edit 2 → exactly the listed writes.
- A call node in a `Group[Call]` with a document survives 10 re-analyses unchanged, and survives "call removed, then re-added" via tombstone/resurrect.
- First import of the pylspt fixture corpus: call trees equal the golden output of `pylspt-dev` (merged form).
- Benchmark: edit a hot utility function in a 2k-function project: re-solve time and number of writes reported in `SyncReport`.

Next: [`08-groups.md`](08-groups.md)
