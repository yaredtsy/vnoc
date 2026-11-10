# 01.03 — Edges: `parent` and `owner`

> ★ This page solves the group problem (L1) **in the data model**, so the sync
> code doesn't have to be clever.

---

## The two links

Every node except `Root` stores two links to containers:

| Link | Meaning | May point to a Group? | Who changes it |
|---|---|---|---|
| `parent` | **where the node is shown** in the tree | yes | the user (moving into/out of groups) and sync (disk moves) |
| `owner` | **where the node is on disk / in the code** | **never** | sync only |

For a file, `owner` is its folder (or `Root`). For a method, `owner` is its class. For a
call node, `owner` is the call node or entry function it was found under.

```
                        parent              owner
Folder "src"            Root                Root
Group "core stuff"      Folder "src"        Folder "src"   (a group's owner = first non-group ancestor)
File "engine.py"        Group "core stuff"  Folder "src"   ◄── shown in the group, lives in src/
File "util.py"          Folder "src"        Folder "src"   ◄── not grouped: parent == owner
```

## Why this makes groups invisible to sync

Sync's question is always: *"on disk, X is inside Y. Does the DB agree?"*

```
disk_container(X) == X.owner  ?
```

That's it. Sync never looks at `parent`, so whether X is in zero, one or five nested
groups doesn't matter, and there's no group resolver, no flattening and no special
cases. Python had three different implementations of "skip the groups"
(L1), and v2 has none, because the answer is stored.

The price is one extra field per node, plus the rule that keeps it correct (below).

---

## Invariants

These must always hold. The memory store checks them after every `apply` in tests
(03-storage/06), so a bug that breaks them fails a test immediately.

| # | Invariant |
|---|---|
| I1 | Every node except Root has exactly one `parent` and one `owner`. |
| I2 | `owner` is never a Group. |
| I3 | If `parent` is not a Group, then `parent == owner`. |
| I4 | A Group's `owner` is its first non-group ancestor. |
| I5 | Family rules: a Group holds only what its family allows (table in 01-node-kinds). |
| I6 | No cycles in `parent` (moving a group into its own descendant is rejected). |
| I7 | A tombstoned node keeps its `parent` and `owner` (that's what resurrection uses). |
| I8 | Every active node's `owner` chain reaches Root through active nodes. |

---

## The operations

### 1. Sync: "X now lives in Y on disk" (`owner` changed)

```
old = X.owner,  new = Y,  old != new

X.owner = new
if X.parent is not a Group:
    X.parent = new                          # normal move
else:
    G = X.parent
    if G.owner == old:                      # the group belonged to the OLD place
        X.parent = new                      #   → X leaves the group (it was a local grouping)
    else:                                   # the group is elsewhere (a cross-cutting "virtual folder")
        keep X.parent = G                   #   → the user's grouping survives the move
```

🟡 **Proposed — confirm (P17):** this "local group vs virtual folder" rule. The idea is
that a group *inside* `src/` is about organising `src/`. Once the file leaves `src/`, the group
no longer applies. A group that holds files from many folders is a virtual folder, and
moving a file on disk shouldn't break it.

### 2. Sync: "X is still in Y" (`owner` unchanged)

Nothing to write. **Even if X is in a group.** This is the case Python got wrong.

### 3. User: move X into group G

```
check family(G) allows kind(X)              (I5)
check G is not X or a descendant of X       (I6)
X.parent = G                                 # owner untouched (I2, sync-only)
```

### 4. User: move X out of a group / to another container

- To another group: case 3.
- To a **non-group** container Z: allowed only if `Z == X.owner`. A user can't move a file to
  another folder in the graph, because the disk decides where files live. (Moving files on disk
  through the IDE is a separate "file operation" feature: it renames on disk, then sync sees it.)

### 5. User: delete group G

Its children move to `G.parent` (one level up), which keeps I3/I4. Then G is deleted. Groups
have no disk counterpart, so they're hard-deleted, not tombstoned (they're user-owned, and the
user asked).

---

## Reading the tree

| Question | Query | Cost |
|---|---|---|
| What does the UI show under X? | nodes where `parent == X` | one indexed lookup |
| What does sync consider X's children? | nodes where `owner == X` | one indexed lookup |
| Top-level items | nodes where `parent == Root/root` | one indexed lookup (R2) |
| Path from X to Root (breadcrumbs) | follow `parent` upward | depth × lookup, or one recursive path query |
| All elements of file F | Class/Function where `file == F` | one lookup (denormalised `file` field) |

> In TerminusDB, `parent` is an edge `X —parent→ Y`. "Nodes where `parent == X`" is a
> lookup by **object**. **[check]** that terminus-store serves object-bound triple
> patterns from an index (it keeps object-indexed layers, so it should). Benchmark it in
> milestone M2 with 100k nodes, because the whole design relies on it.

---

## Why not keep child sets on the parent (like Python)?

| | child sets on parent (v1) | parent link on child (v2) |
|---|---|---|
| move X from A to B | rewrite A's set and B's set (read-modify-write, 2 docs) | change 1 field on X |
| concurrent adds to the same folder | conflict on the folder doc | no shared document |
| "who is X's parent?" | reverse lookup over 5 fields | read 1 field |
| types of children | one field per child type (`file_children`, `folder_children`, …) | none needed: the child knows its own type |
| ordering of children | unordered set anyway | unordered (add an `order` key later if the UI needs it) |

---

## Exit criteria

- Invariants I1–I8 written as a `check_invariants(&Graph) -> Vec<Violation>` function in the memory store.
- Table tests for the operations above, including "file in a local group moves folder → leaves group"
  and "file in a virtual folder moves folder → stays in group".

Next: [`04-lifecycle-and-tombstones.md`](04-lifecycle-and-tombstones.md)
