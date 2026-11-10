# Future 01. Graph diff: changes as functions, not as lines

**Status: idea only. Not scheduled.** Earliest sensible moment: after 0.4, because it reuses the matching work planned there (§4).

```
graph diff
├── README.md                  this file: the idea in one page
├── 01-what-is-possible.md     the levels of diff, and which two versions can be compared
├── 02-how-to-compute.md       two outlines → matched nodes → a list of changes
├── 03-showing-it.md           tree badges · canvas colours · the function diff card
├── 04-resolving.md            fixing things one function at a time            (design only)
└── 05-options-and-path.md     three ways to build it, which is better, in what order
```

## 1. The idea

Git answers: *which lines of which files changed?*
V-NOC can answer: *which functions, classes, files and folders changed, and how?*

```
git diff (today)                           V-NOC diff (this idea)
─────────────────                          ──────────────────────
 src/orders.py     | 48 ++++----           src/orders.py
 src/parsing.py    | 31 +++++               ├── OrderService
 src/helpers.py    | 12 ----               │   ├── parse_order        UPDATED   body
                                            │   └── validate           UPDATED   signature
                                            └── old_total             REMOVED
                                           src/parsing.py
                                            └── parse_payload         MOVED     from src/helpers.py
                                           src/helpers.py
                                            └── fmt_money             ADDED
```

The right column is what a reviewer actually wants to know. The left column makes them work it out.

## 2. Why V-NOC is in a good position to do this

| V-NOC already has | What it gives the diff |
|---|---|
| The driver's **outline**: every function and class with its exact range | cutting a file into functions, in any language that has a driver |
| The outline works on **text**, not only on a file on disk | outlining an *old* version that git hands over as text |
| **IDs in markers** | a documented function that was renamed *and* moved is still known to be the same function |
| The **tree and the canvas** | a place to show the result that the user already knows |
| Docs attached to nodes | a reviewer sees *why* the function exists next to *what* changed in it |
| The planned **matching ladder** (0.4) | the same question ("is this the node I knew before?") asked about two versions |

## 3. What it is not

- **Not a new version control system.** Git stores the versions. V-NOC reads two of them and compares. V-NOC never writes commits, branches or the git index
  by itself in any of the read-only phases.
- **Not a semantic diff.** It says "the body of `parse_order` changed". It does not say "the behaviour changed" or "this change is safe".
- **Not a replacement for the line diff.** Inside one function, the user still reads lines. The function is the unit you *navigate and decide* by.

## 4. Where it sits in the product

```
needs first                                   then this becomes possible
───────────                                   ──────────────────────────
0.1  index + outline                    ──►   phase 1: a list of changed functions (working tree vs last commit)
0.2  call trees                         ──►   phase 5: changed calls, "who is affected"
0.3  editing in the canvas (Monaco)     ──►   phase 3: the function diff card with actions
0.4  matching ladder + saved state      ──►   phase 2: moved / renamed detection for unmarked functions
```

The phases are in [`05-options-and-path.md`](05-options-and-path.md).

## 5. The words used in this folder

| Word | Meaning |
|---|---|
| **version** | one state of the code: a commit, the staged files, or the files as they are on disk now |
| **snapshot** | the outline of every file in one version (a small `Index` without docs) |
| **pair** | one node in the old snapshot matched with one node in the new snapshot |
| **change** | what happened to a pair: added, removed, updated, moved, renamed (or a mix) |
| **roll-up** | a parent shows that something inside it changed (a class, a file, a folder) |
| **rest of file** | the code in a file that is in no function or class: imports, constants, top-level statements |

Next: [`01-what-is-possible.md`](01-what-is-possible.md)
