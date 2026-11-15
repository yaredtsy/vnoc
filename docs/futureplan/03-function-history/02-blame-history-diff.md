# Future 03.02 Blame, history, and the function-only diff

```
computing history
├── 1. last change: blame the function's lines      fast, with one blind spot
├── 2. why not `git log -L`
├── 3. the history walk                             exact: outline both sides, match, compare fingerprints
│   ├── the loop
│   ├── following a function into another file
│   └── before the ID existed
├── 4. the function-only diff
├── 5. filtering commits
├── 6. where the code lives, and the protocol sketch
└── 7. cost and limits
```

## 1. Last change: blame the function's lines

The outline already gives every node's line range. One call:

```
git blame --porcelain -w -M -C  [--ignore-revs-file .git-blame-ignore-revs]  [--contents -]  -L 40,72  -- src/orders.py
```

- **Last change** = the line with the newest `author-time` in the range → its commit, author and summary.
- **Mostly written by** = the author with the most lines in the range (cheap extra).
- `-w` ignores whitespace, `-M -C` follow lines moved inside or across files, `--ignore-revs-file` skips the formatter commits a team has listed.
  Pass it only if the file exists **[check: blame fails on a missing ignore file]**.
- **The buffer is dirty:** pipe the editor's text with `--contents -`, so the line numbers are the ones on screen. Lines with the all-zero commit hash
  are "not committed yet".
- **Leave out the docstring's `ID:` line** from the range, or the commit that added IDs becomes everyone's "last change". Better: leave out the whole
  docstring and say "last code change". Doc edits are already tracked by V-NOC's own docs.

**The blind spot:** blame only sees lines that still exist. A commit that only **deleted** lines inside the function is invisible. Fine for a badge
or a hover; not good enough for a history list. That is §3.

## 2. Why not `git log -L`

`git log -L :parse_order:src/orders.py` gives the history of one function with its diffs, for free. It is a good prototype, and a poor product:

| `git log -L` | the history walk (§3) |
|---|---|
| finds the function by regex patterns (`diff=python` in `.gitattributes`) | the driver's real parser |
| loses a function that moved to another file or class | follows it by ID |
| reports formatting-only commits as changes | `header` / `body` fingerprints tell formatting apart |
| re-computes everything on every call; slow on long histories | every blob's outline is cached forever |

## 3. The history walk

### The loop

```
input:  a node: its ID (if marked), path P, qualname, occurrence

git log --no-merges --raw --no-abbrev -M --format=%H%x00%an%x00%ae%x00%at%x00%s  -- P
        → per commit: hash, author, date, subject,
          and a raw line  ":100644 100644 <old blob> <new blob> M|R087|A  <path>"

for each commit, newest first:
    old = outline(old blob)            ← cached by blob hash: a blob never changes
    new = outline(new blob)
    a = find the node in new           ← ID, else the location it had in the newer commit
    b = match a in old                 ← the matching ladder: ID → location → same body → similar body
    classify (a, b) by fingerprints    ← exactly graph diff §5
        quiet (format, comment, marker) → skip, or show greyed
        body / signature / renamed      → a history row
        b not found                     → "added" here (or moved in, see below); stop
    continue with b's location in the older commit
```

`--raw --no-abbrev` is the key flag: it hands over both blob hashes, so the texts come from one `git cat-file --batch` process and the outline cache
is keyed by something immutable. Each version of a file is parsed once, ever.

**Merges.** Start with `--no-merges`: a merge rarely changes a function by itself. Showing conflict resolutions in merges is a later refinement
**[check how history simplification interacts with `-M` here]**.

### Following a function into another file

When the node is not found in the old blob of P, it may have been moved, not added. That commit's other changed files are already known
(`git show --raw --no-abbrev <commit>`):

```
look for the node in the OLD side of every other file changed in that commit
    found by ID          → "moved from src/helpers.py"; continue the walk with  git log <commit>^ -- src/helpers.py
    found by same body   → "probably moved" (unmarked function)
    not found            → "added"
```

This is phase 4. Phases 2–3 can stop at "added (or moved in)".

### Before the ID existed

Commits older than the `ID:` line only have location and body to match on. That is fine: the location step covers a function that stayed put, and
the commit that added the ID is classified as quiet (the marker is not part of the `body` fingerprint). A rename before the ID existed is found only by
body; show it as "probably renamed".

## 4. The function-only diff

With the pair from §3, this is the easy part:

```
old_text = old blob [ b.range ]        new_text = new blob [ a.range ]
dedent both                            ← a function moved into a class would otherwise differ on every line
split header | body                    ← the signature change is shown apart from the body change
drop the ID: line                      ← or every first-commit-with-an-ID shows a fake change
```

- **In the webview:** send the two texts and let the editor diff them. Monaco has a diff editor, and Monaco arrives in 0.3 (editing in the canvas). The
  core does not need a diff algorithm for this.
- **In the core** (for the CLI, an agent, or "+5 −2" counts): the `similar` crate (Myers / patience, unified output, word-level inline changes) or
  `imara-diff` (faster, used by gix and helix, lower level). Start with `similar`.

This is the same message as graph diff's `vnoc/diffNode` ([`../01-graph-diff/02-how-to-compute.md`](../01-graph-diff/02-how-to-compute.md) §7).

## 5. Filtering commits

Two layers. Git's filters first, because they are cheap; V-NOC's node filter on what is left.

| Filter | Done by |
|---|---|
| author, committer | `--author=` / `--committer=` |
| date range | `--since=` / `--until=` |
| message text | `--grep=` (`-i`, `--all-match`) |
| file or folder | `-- <path>` |
| "the code added or removed this text" | `-S<text>` (pickaxe) or `-G<regex>` |
| **this function** | the history walk (§3): the commits it returns |
| **anything under this class, file or folder** | per commit, graph diff between the commit and its parent, restricted to that subtree |

The last row is graph diff run once per commit. With the blob cache it is cheap for the last few hundred commits; past that, page the list.

## 6. Where the code lives, and the protocol sketch

```
crates/vnoc/src/
├── git.rs             shared with graph diff; adds  blame(path, range, contents?) · log(filters, path) · show_raw(commit) · cat_file_batch()
├── outline_cache.rs   NEW  blob hash → outline; in memory, later on disk under .vnoc/local/cache/
├── matching/          shared with graph diff and identity (0.4)
└── history/           NEW
    ├── last_change.rs      blame lines → { commit, author, time, summary }                 ← pure parser over porcelain text
    ├── walk.rs             the loop of §3
    └── filter.rs           git filters + the node filter
```

Protocol (not frozen, so only a sketch):

```
vnoc/lastChange    { nodes: [id…] }                         → [{ node, commit, author, time, summary, uncommitted }]
vnoc/nodeHistory   { node, limit, cursor }                  → { rows: [{ commit, author, time, subject, change, moved_from? }], cursor }
vnoc/commits       { author?, since?, until?, grep?, path?, node?, limit, cursor }   → { rows, cursor }
vnoc/diffNode      { from, to, node }                       → { old_text, new_text, old_range, new_range }     (graph diff's)
```

Tests in the style of `resolve()`: `last_change.rs` and the classification are pure functions over text, so they take tables of small cases. `walk.rs`
gets one fixture repo built by a test script (`git init`, a few scripted commits: edit, rename, move, format-only, delete-only).

## 7. Cost and limits

| Step | Cost |
|---|---|
| blame one function | milliseconds to ~100 ms on a long-lived file **[measure]** |
| `git log --raw` for one file | milliseconds for hundreds of commits |
| outline per blob | once per blob, ever; a few ms each |
| history of a function with 200 commits on its file | ~400 outlines the first time, nearly free after |

Limits to state in the UI rather than hide:

- **Shallow clones:** blame and log stop at the clone boundary. Say "history starts at <commit> (shallow clone)".
- **Rewritten history** (rebase, force push): a cache keyed by blob hash survives it; anything keyed by commit would not. Key by blob.
- **Splits and merges** (one function becomes two): only a guess by body similarity, shown as "probably".
- **Unmarked functions** get "probably" on every rename and move. One more visible reason for IDs.

Back: [`README.md`](README.md)
