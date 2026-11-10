# Future 01.02 How to compute it

```
computing a diff
├── 1. the pipeline in one picture
├── 2. getting the two versions            from git, as text
├── 3. two snapshots                       outline only the files that changed
├── 4. matching nodes                      ID → location → same body → similar body
├── 5. classifying a pair                  three fingerprints per node
├── 6. roll-up
├── 7. where the code lives                modules, driver additions, storage
└── 8. cost
```

## 1. The pipeline

```
 version A (old)        version B (new)
      │                       │
      ▼                       ▼
 list the files that differ between A and B              git, one call
      │
      ▼
 outline each of those files, in A and in B              the driver, text in → defs out
      │
      ▼
 snapshot A             snapshot B                       two small indexes (code only)
      └──────────┬───────────┘
                 ▼
            match nodes                                  pairs + unmatched
                 ▼
            classify each pair                           added / removed / updated / moved / renamed
                 ▼
            roll up to classes, files, folders
                 ▼
            ChangeSet                                    what the UI and the CLI read
```

Every box except the first two is a **pure function**: values in, values out, no disk and no git. That is the same shape as `resolve()` in
[`../../05-identity/05-finding-nodes-again.md`](../../05-identity/05-finding-nodes-again.md), and it is testable the same way: a table of small cases.

## 2. Getting the two versions

A version only has to answer two questions: *which files do you have?* and *give me the text of this file*.

| Version | List of files | Text of a file |
|---|---|---|
| files on disk | the scan V-NOC already does | read the file (or VS Code's buffer) |
| a commit | `git ls-tree -r <commit>` | `git show <commit>:<path>` (or one `git cat-file --batch` for many) |
| staged | `git ls-files -s` | `git show :<path>` |

And one call gives the files that differ, with renames detected by git:

```
git diff --name-status -M <A> <B>
M       src/orders.py
R087    src/helpers.py    src/parsing.py        (renamed, 87 % similar)
A       src/money.py
D       src/legacy.py
```

**How to talk to git.** Two choices:

| | Run the `git` program | Use a library (`gix`) |
|---|---|---|
| Work to start | almost none | learn the API |
| Needs git installed | yes (every user of this feature has it) | no |
| Speed for many small reads | fine with `cat-file --batch` | best |
| Behaves exactly like the user's git (config, attributes, LFS) | yes | mostly |

Start by running `git`. The plan already postponed `gix` once ([`../../02-mvp-and-releases.md`](../../02-mvp-and-releases.md) §2); this feature is not
a reason to bring it back early. Hide the choice behind one small module (`git.rs`) with four functions, so it can change later.

## 3. Two snapshots

A snapshot is the code half of the index: `path → outline`. Two rules keep it cheap:

1. **Outline only the files git says differ.** A file that is the same in A and B cannot contain a change. On a normal commit that is 1–20 files.
2. **Reuse the live index for the "files on disk" side.** It is already built and kept fresh ([`../../04-storage/02-index.md`](../../04-storage/02-index.md) §4).

The driver needs nothing new for this: `vnoc/outline` already accepts `text` in place of a file path, which is exactly what `git show` produces.

**The exception is "moved".** A function that moved from `a.py` to `b.py` is found only if both files are in the comparison. They will be: both files
differ between A and B, so git lists both.

## 4. Matching nodes

The central step. For each node in A, find the same node in B. First rule that gives an answer wins:

```
1. SAME ID          both sides carry the same V-NOC marker
                    → certain. Works through any rename, move and edit.

2. SAME LOCATION    same path + qualified name + occurrence, and step 1 did not claim either side
                    → the normal case: the function stayed where it was.

3. SAME BODY        the body fingerprint is equal, among nodes still unmatched
                    → an unmarked function that was moved or renamed without being edited.

4. SIMILAR BODY     similarity above a threshold, among nodes still unmatched, only within files that differ
                    → an unmarked function that was moved AND edited. Marked "probably", never shown as certain.

5. nothing          left in A → removed.   left in B → added.
```

Three things to notice:

- **This is the matching ladder.** Steps 3 and 4 are the same work as "finding a node again without a marker", planned for 0.4 and already prototyped in
  spike S4 (`spikes/s4-git/spike-match`). Build it once, in one module, and use it for both. This is the main reason the diff should wait for 0.4.
- **Files are matched first, then their contents.** Git's rename line (`R087 a.py b.py`) pairs the two files; the functions inside are then matched by
  step 2 using the *new* path. Without that, a renamed file would show every function as removed and added.
- **Markers make the diff better.** That is a real, visible benefit of IDs for a user who is unsure about them, and worth saying in the UI: a moved
  function with an ID is "moved"; without one it is "probably moved".

## 5. Classifying a pair

To tell the kinds of change in [`01-what-is-possible.md`](01-what-is-possible.md) §2 apart, compare three fingerprints per node instead of raw text:

| Fingerprint | Made from | Equal in A and B means |
|---|---|---|
| `header` | the tokens of the signature: name, parameters, return type, decorators, base classes | signature did not change |
| `body` | the tokens of the body, **without** comments, the docstring, and the V-NOC marker | the code did not change |
| `text` | the exact source text of the node | nothing at all changed |

```
text equal                                        → unchanged
text differs, header + body equal                 → quiet: formatting, comments, docstring or marker only
header differs, body equal                        → updated: signature     (or renamed, if only the name differs)
body differs                                      → updated: body
path or parent differs                            → + moved
```

"Tokens" is the important word. Hashing tokens instead of characters makes the fingerprint blind to spaces, line breaks and quote style, so a
formatter run does not show as 400 updated functions.

**Who computes the fingerprints: the driver.** Only it knows what a token, a comment and a docstring are in its language. This is a small addition to
the outline result, behind a capability flag, in the style of [`../../06-drivers.md`](../../06-drivers.md) §3:

```
capability   "outline.fingerprints": true
outline def  + "header_hash": "…", "body_hash": "…"
```

A driver without the capability still works: the core falls back to hashing the raw text of the range, and the diff simply cannot tell formatting from
a real change. This is the same `body_hash` the doc-freshness idea needs, so one driver change pays twice.

**Nested nodes.** A method's change must not also count as a body change of its class. Compute a class's own `body` fingerprint from what is left after
removing its child nodes' ranges.

## 6. Roll-up

After classification, walk the tree upward once:

```
for each changed node:  add 1 to the matching counter of every ancestor (class, file, folder)
```

A parent then knows `{ added, removed, updated, moved }` inside it. The UI shows the dot and the counts; a collapsed folder still tells the truth.

The **rest of file** pseudo-node (code outside any def) is classified by comparing the file text with all def ranges cut out.

## 7. Where the code lives

Following the module map in [`../../03-repo-structure.md`](../../03-repo-structure.md) §4:

```
crates/vnoc/src/
├── git.rs             NEW  diff_names(a, b) · show(rev, path) · merge_base(a, b) · status()
├── matching/          NEW in 0.4 (shared with identity/resolve): same body, similar body
└── diff/              NEW
    ├── mod.rs              pub fn diff(a: &Snapshot, b: &Snapshot) -> ChangeSet        ← pure
    ├── snapshot.rs         build a Snapshot from a Version (git or disk) with the driver
    ├── classify.rs         pair → Change                                                ← pure
    └── rollup.rs                                                                        ← pure
```

Dependency rule, same direction as the existing one: `server → diff → {index, matching, git, driver_host}`. `diff` does not know `userdata`.
Docs are added to a `ChangeSet` one level up, by joining on the node ID.

**Storage: nothing new on disk.** Asking the one question from [`../../04-storage/README.md`](../../04-storage/README.md) §2 (*did a person create it?*):
no, a diff is computed. It is index data (place C).

- Keep the last few `ChangeSet`s in memory, keyed by `(A, B)`.
- When SQLite exists, a snapshot of a **commit** can be cached forever under `.vnoc/local/cache/`, because a commit never changes. That makes
  history browsing fast. Still no migration, still deletable.
- The one exception is a **checkpoint** ("since the agent started"): the working tree at a moment that is not a commit. Store it under
  `.vnoc/local/` (never committed) as a list of `path → blob hash`, with the blobs written by `git hash-object -w` or kept as plain files **[check
  which is simpler]**.

**The protocol** is free to change (it is not frozen), so only a sketch:

```
vnoc/diff        { from: "HEAD", to: "WORKTREE" }            → { revision, changes: [...], rollup: {...} }
vnoc/diffNode    { from, to, node }                          → { old_text, new_text, old_range, new_range }
```

## 8. Cost

Rough, to be measured:

| Step | Cost |
|---|---|
| `git diff --name-status` | milliseconds |
| `git show` per changed file | ~1 ms each with `--batch` |
| outline per changed file | the same as today's index update for one file |
| matching | linear for steps 1–3 (hash maps). Step 4 compares only the unmatched leftovers, usually a handful. |

So "uncommitted changes" on a normal working day should feel instant. The slow case is a comparison across hundreds of files; that is what lazy
outlining in the UI is for ([`03-showing-it.md`](03-showing-it.md) §5).

Next: [`03-showing-it.md`](03-showing-it.md)
