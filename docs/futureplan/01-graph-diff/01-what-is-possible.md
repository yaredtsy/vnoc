# Future 01.01 What is possible

```
what is possible
├── 1. four levels of diff
│   ├── L0 lines            git, today
│   ├── L1 functions        added · removed · updated · moved · renamed
│   ├── L2 graph            the tree with roll-ups, and (later) call edges
│   └── L3 meaning          out of reach
├── 2. the kinds of change a node can have
├── 3. which two versions can be compared
├── 4. what docs add to a diff
└── 5. the hard limits
```

## 1. Four levels

| Level | Question it answers | Who can do it | Cost |
|---|---|---|---|
| **L0 lines** | which lines changed in which file? | git | free |
| **L1 functions** | which functions and classes changed, and how? | V-NOC with one driver call per changed file | low |
| **L2 graph** | what does the change look like on the structure (folders → files → classes → functions) and on the call tree? | V-NOC: L1 plus roll-ups; call edges need 0.2 | medium |
| **L3 meaning** | did the behaviour change? is it safe? | nobody reliably. Tests and people. | — |

This plan is about **L1 and L2**. L0 stays as the way to read *inside* one function.

L2 has two halves, and they are very different in cost:

```
L2 graph
├── structure graph     who CONTAINS whom       cheap: it is the tree V-NOC already draws, with a change mark on each node
└── call graph          who CALLS whom          expensive: needs call trees for both versions (0.2 driver work, twice)
```

Build the structure half first. The call half is the last phase.

## 2. The kinds of change

For one node (a function, a class, a file, a folder), comparing an old and a new version:

| Change | Meaning | How it is known |
|---|---|---|
| **added** | exists only in the new version | no pair found |
| **removed** | exists only in the old version | no pair found |
| **updated: body** | same node, the code inside changed | pair found, body text differs |
| **updated: signature** | name is the same, parameters / return type / decorators changed | pair found, the header part differs |
| **renamed** | same node, new name, same file | pair found by ID or by body, name differs |
| **moved** | same node, different file or different parent class | pair found by ID or by body, path differs |
| **moved + updated** | both | pair found by ID (or by similarity, with less certainty) |
| **unchanged** | — | pair found, text equal |

And four that should be **hidden by default**, because they are noise:

| Quiet change | Why it is noise |
|---|---|
| **only a V-NOC marker changed** (`ID:` line added or removed) | V-NOC did that itself. Showing "updated" on every function that got a doc would make the feature useless. |
| **only formatting changed** (spaces, line breaks, quotes) | a formatter ran. Useful as a count ("42 reformatted"), not as 42 rows. |
| **only the docstring or comments changed** | a real change, but a different kind. Its own mark ("docs only"), filterable. |
| **only the position changed** (lines above were added) | not a change to this function at all. Line diffs get this right already; the function diff must too. |

Parents get a **roll-up**, not a change of their own:

```
src/billing/           ● 3 changed inside                  (folder)
└── invoice.py         ● 2 changed inside, 1 removed       (file)
    └── Invoice        ● 1 changed inside                  (class)
        └── total      UPDATED body                        (the real change)
```

A class can also change by itself (its base classes, its decorators, its class-level attributes). That is "updated: signature" on the class, plus the
roll-up from its methods.

## 3. Which two versions

Any two versions can be compared, because a version is only "a way to get the text of each file". The useful pairs:

| Compare | Name in the UI | Used for | Difficulty |
|---|---|---|---|
| files on disk ↔ last commit (`HEAD`) | **Uncommitted changes** | "what have I (or my agent) changed?" | easiest: the new side is the live index |
| staged ↔ `HEAD` | Staged changes | review before commit | easy |
| commit ↔ its parent | **This commit** | reading history | easy |
| branch tip ↔ merge base with `main` | **This branch / pull request** | code review | easy (needs the merge base from git) |
| any commit ↔ any commit | Compare… | "what happened since the release?" | easy, can be large |
| a saved moment ↔ now | **Since checkpoint** | "what did the agent do in this session?" (see [`../02-agent-access/`](../02-agent-access/README.md)) | medium: needs the checkpoint stored |
| base ↔ ours ↔ theirs | **Merge in progress** | resolving conflicts ([`04-resolving.md`](04-resolving.md)) | hard: three snapshots, and one file may not parse |

The first row is where to start: it needs only one extra snapshot (the old side), and it is the question people ask most.

## 4. What docs add

This is the part no other diff tool has.

- **The reviewer sees the "why" next to the change.** A changed function with docs shows its doc badge in the diff. One click opens the doc panel.
- **Doc changes are part of the same picture.** A new or edited `.md` under `.vnoc/nodes/<id>/` is shown **on its node** ("doc added", "doc edited"),
  not as a stray file in a hidden folder.
- **"The code changed, the doc did not."** If a documented function is `updated` and none of its docs changed in the same diff, show a small hint:
  *doc may need a look*. This is the cheap version of the doc-freshness idea (`body_hash`, planned for 0.6+ in
  [`../../04-storage/04-growth-path.md`](../../04-storage/04-growth-path.md)); it needs nothing stored.
- **A removed function with docs** is a decision the user should see: the docs are about to become Missing. The diff can say so before the commit.

## 5. The hard limits

Know these before promising anything.

| Limit | Why | What to do |
|---|---|---|
| **Code outside functions** | imports, constants, `if __name__ == "__main__":`, decorators between defs. It belongs to no function node. | A pseudo-node per file: **rest of file**. It is diffed by lines. Without it, changes would silently disappear from the view. |
| **A file that does not parse** | half-written code, conflict markers | Fall back to the line diff for that file and say so. Never guess. |
| **Unmarked functions that are renamed and edited at once** | nothing links old and new except similarity | Shown as removed + added, or as "probably moved" when similarity is high. Markers make this exact. |
| **Split and merge** | one function becomes two, or two become one | Shown as one updated + one added (or one removed). Do not try to be clever. |
| **Nested functions and lambdas** | a change in an inner function is also a change in the outer one | Mark the innermost node as the change, roll up to the outer one. |
| **Languages without a driver** | no outline | File-level only (added / removed / changed), same as git. |
| **Big comparisons** | 2,000 changed files between two releases | Outline lazily: files first, functions when a file is opened in the diff view. |
| **Generated and vendored code** | noise | Honour `[index] exclude` from `vnoc.toml`, as the tree does. |

Next: [`02-how-to-compute.md`](02-how-to-compute.md)
