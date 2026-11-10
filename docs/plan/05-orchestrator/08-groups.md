# 05.08 — Groups: invisible to sync

This page collects, in one place, how groups interact with every sync step. The short version:
**sync compares `owner`, the user edits `parent`, and the two never collide** (01-data-model/03).

---

## Per step

| Step | What it compares | Group impact |
|---|---|---|
| 4 structure diff | `disk container == row.owner` | none. A grouped file still in its folder is `unchanged`. |
| 6 code diff | `enclosing def == row.owner` | none. A grouped method still in its class is `unchanged`. |
| 7 reconcile | IDs, plus owner chains for cascades | cascades follow `owner`, so a tombstoned folder tombstones its files even if they're shown in a group elsewhere |
| 8 calls | `call_level(owner)` | none. Grouped call nodes are found by owner. |
| 9 plan: disk moves | — | **the only place groups matter**: the P17 rule (below) |
| UI reads | `children(parent)` | groups appear as containers |

## The one rule: what happens to a grouped item that moves on disk (P17)

```
item X moved on disk: owner old → new
X.parent is a Group G?
  ├─ no            → parent = new
  ├─ G.owner == old → parent = new        (local group of the old place: X leaves it)
  └─ G.owner != old → parent stays G      (virtual folder spanning places: X stays)
```

Python's behaviour, for comparison: every resync moved grouped items back out, even when
they *hadn't* moved on disk (L1). v2 never touches a grouped item that didn't move, and only
the P17 rule decides about the ones that did.

## Empty groups

When all of a group's children are tombstoned, the group stays (empty), shown greyed out by the
UI. Sync never deletes groups. A group's children list in the UI hides tombstoned children
(rule 2 of 01-data-model/04). If they're resurrected, they reappear **in the group**, because
tombstoning keeps `parent` (I7).

## Group operations (service layer, not sync)

Handled by `GroupService` in `vnoc-app` (06-api/03). Each one is a small `WritePlan` through the
project writer:

| Operation | Checks | Plan |
|---|---|---|
| create group under P | family allowed under P (table in 01-node-kinds) | create Group { parent: P, owner: owner_of(P) } |
| move X into G | family allows X; no cycle; **G.owner == X.owner** for `Code`/`Call` families (🟡 proposed: code/call groups are always local), any owner for `Structure` (virtual folders allowed) | patch X.parent = G |
| move X out of G | target is a group (rules above) or `X.owner` | patch X.parent |
| move group G under H | family match; no cycle | patch G.parent = H, G.owner = owner_of(H) (if G's owner changes, the P17 meaning of "local" changes with it, which is correct) |
| delete G | — | children's parent = G.parent; delete G |
| rename / theme / docs | — | user fields only |

## Exit criteria

Scenario tests (memory store): grouped file untouched ×10 syncs → stays; grouped file moved to
another folder with a local group → leaves; with a virtual folder → stays; grouped method in an
edited class → stays; all children tombstoned → empty group kept; resurrected → back in group.

Next: [`09-scheduler-and-watcher.md`](09-scheduler-and-watcher.md)
