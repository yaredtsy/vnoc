# 05.05 — Steps 5–6: parse and diff classes/functions

---

## Step 5: parse (async, windowed)

```rust
let outlines: Vec<FileOutline> = futures::stream::iter(sdiff.files_to_parse().chunks(256))
    .map(|chunk| ctx.drivers.outline(chunk))        // grouped by driver, ONE call per chunk
    .buffer_unordered(4)
    .try_concat().await?;
```

Before that, tell drivers what changed: `driver.files_changed(&changes)`, so pylspt updates its
salsa inputs. Then parse once, and both the code diff and the call step (8) reuse the driver's
cache. Python parsed every file twice (L5 analysis, section 4).

A file whose outline is `Err(ParseProblem)` goes to `report.files_failed` and is **excluded from
step 6 completely**. Its old classes/functions stay as they are, and none of them become "missing".

## Step 6: the diff (pure)

```rust
pub fn diff(ids: &dyn IdStrategy, outlines: &[FileOutline], snap: &CodeSnapshot,
            file_ids: &FxHashMap<RelPath, FileId>) -> CodeDiff;

pub struct CodeDiff {
    pub new:           Vec<DesiredDef>,
    pub resurrected:   Vec<DesiredDef>,
    pub moved:         Vec<DefMove>,                 // owner and/or file changed (re-nesting, cross-file)
    pub changed:       Vec<(NodeId, DefPatch)>,      // name / qname / span / bases / body_hash
    pub missing:       Vec<NodeId>,                  // candidates only (step 7)
    pub dup_ids:       Vec<DupId>,
    pub bodies_changed: FxHashSet<NodeId>,           // → step 8 (calls)
    pub outline_hash:  Vec<(FileId, Hash128)>,       // → File patch
}
```

### Which snapshot rows?

`CodeSnapshot` = `snapshot_code(parsed files)` (rows where `file ∈ parsed files`) **plus**
`code_rows_by_ids(ids seen in outlines but not in those rows)`. The second query finds:

- a function that moved **in** from a file that didn't change in this sync (rare, but possible with a scoped sync),
- a function whose old row is **Tombstoned** from an earlier sync (cut in sync 1, pasted in sync 2), which is then resurrected.

### Algorithm

```
desired = []
for outline in outlines (ok only):
    walk defs depth-first; for each def:
        owner = enclosing def's id, or the FileId at top level
        id = ids.code_id(def)               # in-source: the docstring ID
             or pending match: snapshot row with id_pending, same owner, same kind, same name
             or mint (minted = true)
        desired.push(DesiredDef { id, owner, file, name, qname, span, body_hash, bases, kind })

resolve duplicate IDs across ALL desired defs (01-data-model/02)

for d in desired:
    match snap.get(d.id):
        None                       → new
        Some(r) if r.tombstoned    → resurrected (+ compare as below)
        Some(r):
            r.owner != d.owner or r.file != d.file → moved
            any S-field differs                     → changed (patch only those fields)
            r.body_hash != d.body_hash              → bodies_changed.insert(d.id)

missing = snap Active rows of the parsed files whose id is not in desired
```

The important line is **"across ALL desired defs"**. The `desired` list covers every parsed file
of the sync, so a function that left `a.py` and arrived in `z.py` is one entry with a new owner
(`moved`), never a `missing` in one batch plus a `new` in another. Windowing (step 5) only affects
*how* we parse; the diff runs once over the complete result.

### Owner is "the enclosing def", groups invisible again

A method inside a `Group[Code]` under its class has `parent = Group`, `owner = Class`. The disk
says its enclosing def is the class, which matches the owner, so it's unchanged and stays in its
group. Python got this wrong for grouped methods (L1, `ast_processor.py::_build_existing_map`
fell back to the file as parent).

### Spans change a lot

Adding a line at the top of a file shifts every span below it. That's a `changed` patch with
only `span` for each def: tiny triples, all in the same commit. It **doesn't** touch
`bodies_changed` (the body hash ignores position), so it causes **no call re-analysis**.

## Tests (pure)

| Case | Expected |
|---|---|
| edit inside a function body | `bodies_changed = {f}`, `changed` has `body_hash` only |
| add a blank line above a function | `changed` has `span` only, `bodies_changed` empty |
| rename a function (ID kept) | `changed` name+qname |
| move a method from class A to class B (same file) | `moved` owner A → B |
| cut `f` from a.py, paste into z.py, same sync | `moved` (file + owner), **not** missing+new |
| cut in sync 1 (tombstoned), paste in sync 2 | `resurrected` + `moved` |
| a.py has a syntax error | a.py's defs are in none of the lists |
| new function without ID | `new`, minted |
| grouped method, untouched | nothing |
| copy-paste a function (dup ID) | the original keeps it, the copy is `new` with a minted id |

Next: [`06-moves-and-tombstones.md`](06-moves-and-tombstones.md)
