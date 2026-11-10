# Future 01.05 Options, the better one, and the path

```
options and path
├── 1. three ways to compute a function diff
│   ├── A. map git's hunks onto function ranges
│   ├── B. compare two outlines                    ← recommended
│   └── C. full syntax-tree diff
├── 2. what already exists elsewhere
├── 3. the path: five phases, each useful alone
├── 4. what must be true before starting
├── 5. risks
└── 6. open questions
```

## 1. Three ways to compute it

### A. Map git's hunks onto function ranges

Take `git diff -U0`, which lists changed line ranges. Outline the new file (and the old one, for deletions). A function whose range overlaps a hunk
is "updated".

### B. Compare two outlines

Outline both versions, match the nodes, compare fingerprints. This is [`02-how-to-compute.md`](02-how-to-compute.md).

### C. Full syntax-tree diff

Parse both versions into complete syntax trees and compute an edit script between the trees (insert / delete / move / update of any tree node, down to
single expressions).

### Compared

| | A. hunks → ranges | **B. two outlines** | C. tree diff |
|---|---|---|---|
| Added / removed / updated | ✅ | ✅ | ✅ |
| Moved, renamed | ❌ shows as removed + added | ✅ exact with IDs, good without | ✅ |
| Signature vs body | ❌ | ✅ | ✅ |
| Ignores formatting-only changes | ❌ | ✅ with driver fingerprints | ✅ |
| Ignores V-NOC's own marker lines | needs a special case | ✅ by construction | needs a special case |
| Detail inside a function | lines (git's) | lines (git's or Monaco's) | expressions |
| Driver work | none | two hashes in the outline | a full tree-diff per language |
| Works for a simple (tier 1) driver | ✅ | ✅ (falls back to text hashes) | ❌ |
| Is the base for function-level merge | ❌ | ✅ | ✅ |
| Size of the work | days | weeks | months, per language |

**B is the better choice**, for three reasons:

1. It is the only one that turns IDs into a visible advantage (exact moves and renames).
2. It asks the driver for almost nothing, so every future language gets the feature with its outline.
3. Its matching step is the 0.4 matching ladder. One piece of work, two features.

**A is a fair phase-1 shortcut** if a first version is wanted before 0.4: it gives the Changes list with added / removed / updated and nothing else,
and it is thrown away later without regret because nothing depends on its internals.

**C is not worth it for V-NOC.** The function is the unit V-NOC shows; detail below that is served well enough by a line diff inside the card.

## 2. What already exists elsewhere

Worth an afternoon of trying before building, to learn what feels good and what does not. All **[check]**: names and features from memory.

| Tool | What it does | What V-NOC would add |
|---|---|---|
| `git diff --function-context`, `git log -L :name:file` | git can show a whole function around a change, and the history of one function, using simple per-language patterns | a real outline instead of patterns; moves; a graph view |
| Difftastic | a syntax-aware diff for the terminal: compares trees, ignores formatting | the unit is still a file; no identity across renames; no graph |
| GumTree | the research-grade tree diff (option C) | — |
| Mergiraf, and the older SemanticMerge | syntax-aware **merge** as a git merge driver | a UI, docs next to conflicts, IDs |
| JetBrains IDEs, VS Code | 3-way merge editors, "local history", structure views | none of them joins structure + diff + docs |

The honest summary: pieces of this exist. **Nobody shows the diff on a browsable graph with the docs attached.** That combination is the reason to
build it, and it is also why the UI matters more than the algorithm.

## 3. The path: five phases

Each phase ships alone and is useful alone. Stop after any of them.

| Phase | What the user gets | Needs | Writes anything? |
|---|---|---|---|
| **1. Changes list** | sidebar list of added / removed / updated functions for **uncommitted changes**; click opens VS Code's diff editor on that one function | 0.1 index · `git.rs` · option B without steps 3–4 of the matching (or option A) | no |
| **2. Any two versions, on the graph** | the compare picker · tree marks and roll-ups · the canvas in diff mode · moved / renamed · quiet changes folded · doc hints | 0.4 matching · driver fingerprints · SQLite cache for commit snapshots (nice to have) | no |
| **3. Actions on your own changes** | the function diff card with Revert / Restore / Remove / Stage this function | 0.3 (Monaco in the canvas) · the safety rules in [`04-resolving.md`](04-resolving.md) §4 | source files, through VS Code edits; the git index, on click |
| **4. Merge conflicts** | first the navigator (option 1), then the in-editor resolver (option 2) | phase 3 · three-way matching | source files, through VS Code edits |
| **5. Call-graph diff** | changed call edges · "who is affected" highlighting | 0.2 call trees, computed for two versions | no |

```
value ▲
      │                         ┌──────── 4. merge conflicts
      │                 ┌───────┘
      │           ┌─────┘ 3. actions
      │      ┌────┘ 2. graph
      │  ┌───┘ 1. list
      └──┴──────────────────────────────────────────► work
```

Phase 2 is where the idea becomes something no other tool has. Phase 4 is the largest step in both value and risk.

A `vnoc diff` command for agents and scripts ([`../02-agent-access/`](../02-agent-access/README.md)) can ship with phase 1: it is the same `ChangeSet`
printed as text or JSON.

## 4. What must be true before starting

- 0.1 is out and people use it. A diff view on a product nobody has installed is wasted work.
- The index is incremental and fast (Step 1 of 0.1), because "uncommitted changes" recomputes on every save.
- For phase 2: the matching module exists and was tuned on real refactors (spike S4's fixtures).
- For phase 3 and later: undo and the "refuse if the document changed" check are proven by the attach flow (open question Q4 in
  [`../../08-decisions-to-log.md`](../../08-decisions-to-log.md)).

## 5. Risks

| Risk | How likely | What limits it |
|---|---|---|
| The diff is **wrong** once (a change not shown) and trust is gone | medium | the *rest of file* node; fall back to file-level when a file does not parse; a visible "N files shown as plain file changes" line |
| "Updated" on every function because of markers or a formatter | high without fingerprints | the quiet-change rules ([`01-what-is-possible.md`](01-what-is-possible.md) §2) are part of phase 1, not polish |
| Function-level merge produces broken code | real | parse-check the result; start as suggestions; never auto-resolve silently |
| Scope: this is a product inside the product | high | the phases; a hard stop after phase 2 until users ask for more |
| Performance on big comparisons | medium | lazy outlining; cached commit snapshots |
| Each new language must support it | low | option B needs only the outline; fingerprints are optional |

## 6. Open questions

| # | Question | Lean |
|---|---|---|
| G1 | Is the default comparison "uncommitted changes" always on (like git decorations), or a mode the user turns on? | A mode at first. Always-on marks are a design decision for after phase 2. |
| G2 | Does the similarity threshold from spike S4 fit diffs, or does review need a stricter one than "find my doc again"? | Probably stricter: a wrong "moved" in a review is worse than a "removed + added". |
| G3 | Checkpoints: who creates one? The user, the agent at session start, or V-NOC on every commit / checkout? | The agent integration creates them (a hook at session start); a manual command for people. |
| G4 | Should a doc be attachable **to a change** ("why this function was rewritten"), not only to a node? | Tempting, and it would touch frozen formats. Keep it out until asked for; a normal doc on the node covers most of it. |
| G5 | Is "Stage this function" worth the git-index risk, when the user can stage hunks in VS Code already? | Decide after watching phase 3 without it. |
| G6 | A git merge driver (option 3 in [`04-resolving.md`](04-resolving.md) §5): compete or integrate with an existing one? | Integrate or skip. V-NOC's value is the UI. |

Back to the map: [`README.md`](README.md) · the other idea: [`../02-agent-access/`](../02-agent-access/README.md)
