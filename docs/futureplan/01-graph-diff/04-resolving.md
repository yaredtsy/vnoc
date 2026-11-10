# Future 01.04 Resolving at function level

**Status: design only.** The diff views in [`03-showing-it.md`](03-showing-it.md) come first and are useful without any of this. This file describes
what "fix it per function" could mean, so the read-only phases do not close the door on it.

```
resolving
├── 1. two different jobs with the same name
│   ├── A. your own changes        revert / stage / keep, one function at a time
│   └── B. a merge conflict        ours / theirs / both / edit, one function at a time
├── 2. job A: actions on a changed function
├── 3. job B: function-level merge
│   ├── why it is better than line merge
│   ├── the decision table
│   ├── the conflict card
│   └── what stays hard
├── 4. the safety rules
└── 5. how deep to go: three options
```

## 1. Two jobs

| | **A. Your own changes** | **B. A merge conflict** |
|---|---|---|
| Versions | 2: last commit ↔ working tree | 3: base ↔ ours ↔ theirs |
| Question | "do I keep this change to this function?" | "whose version of this function wins?" |
| Risk | low: it is a text edit with undo | high: a wrong answer ships broken code |
| Value | medium | high, and rare elsewhere |

Build A first. It teaches everything B needs (replacing one function's text safely) at a fraction of the risk.

## 2. Job A: actions on a changed function

Each action is **one text edit on one range**, applied by VS Code, exactly like writing a marker
([`../../05-identity/01-id-generation.md`](../../05-identity/01-id-generation.md) §4).

| On a node that is… | Action | What happens |
|---|---|---|
| updated | **Revert this function** | replace the function's range in the working file with its text from the old version |
| added | **Remove this function** | delete its range (and the blank lines that belonged to it) |
| removed | **Restore this function** | insert the old text back. *Where?* After the nearest earlier sibling that still exists, else at the end of its old parent. |
| moved | **Move back** | two edits in two files, applied as one `WorkspaceEdit` so they succeed or fail together |
| any | **Stage this function** | put only this function's change into the git index (below) |
| any | **Open in editor** | always there; the escape to doing it by hand |

**Staging one function** is the only action that touches git. Two ways:

```
a) build a patch that contains only the hunks inside the function's range, then   git apply --cached <patch>
b) build the full staged text of the file (old file + this one function replaced), then   git update-index  with that blob
```

(b) is simpler to get right: it never has to cut a hunk in two. It is what "stage selected lines" does in editors, with the selection chosen by V-NOC.
**[check both on a file with CRLF line endings and with no newline at the end]**

What the restore position rule shows: **order inside a file matters and the outline knows it.** Keep the sibling order of each node in the snapshot.

## 3. Job B: function-level merge

### Why it is better than a line merge

Git merges by lines. It reports a conflict when both sides touched the same or neighbouring lines, even when the changes have nothing to do with each
other. Three very common false conflicts:

| Situation | Line merge | Function merge |
|---|---|---|
| Both branches **add a different function at the same place** (end of the file, end of a class) | conflict | no conflict: two added nodes |
| One branch **moves** a function, the other **edits** it | conflict, or worse: a silent duplicate | moved + updated: apply the edit at the new place |
| One branch **reorders** functions, the other edits one | conflict | no conflict |
| Both branches edit **different functions** that happen to be adjacent | sometimes a conflict | no conflict |

And one case that a line merge gets *wrong without saying so*: one side renames a function, the other adds a new call to the old name. Lines merge
cleanly, the code is broken. A function-level view cannot fix that, but with call trees it can **warn** ("`validate` was renamed on one side and is
called by a new function on the other").

### The decision table

Three snapshots (base, ours, theirs), nodes matched across all three with the rules in [`02-how-to-compute.md`](02-how-to-compute.md) §4. Then, for
each node:

| Ours vs base | Theirs vs base | Result |
|---|---|---|
| unchanged | unchanged | keep |
| changed | unchanged | **take ours** (automatic) |
| unchanged | changed | **take theirs** (automatic) |
| changed | changed, **identical** | take it (automatic) |
| changed | changed, **different** | → try a line merge **inside this function only**. Clean → take it, marked "merged, please look". Not clean → **function conflict** |
| removed | unchanged | removed |
| removed | changed | **conflict**: "deleted on one side, edited on the other" |
| added | added, same name, different text | **conflict** |
| moved | updated | take the update at the new place (automatic, marked) |

Only the rows marked conflict need a person. Everything else is decided, and shown as decided, so the user can still overrule it.

### The conflict card

```
┌─ OrderService.parse_order ─────────────── CONFLICT ─────────────────────┐
│  both sides changed this function                    📄 2 docs   ⓘ      │
├─────────────────────────────────┬───────────────────────────────────────┤
│ OURS  (feature/decode)          │ THEIRS  (main)                        │
│  def parse_order(self, raw):    │  def parse_order(self, raw, *, tz):   │
│      data = self._decode(raw)   │      data = json.loads(raw)           │
│      return Order(**data)       │      return Order(**data, tz=tz)      │
├─────────────────────────────────┴───────────────────────────────────────┤
│  [Take ours]  [Take theirs]  [Keep both]  [Edit…]        base ▸         │
└─────────────────────────────────────────────────────────────────────────┘

Merge: 14 functions merged automatically · 2 conflicts left · rest of file: 1 conflict (imports)
```

- **Take ours / theirs**: this function's text comes from that side.
- **Keep both**: both versions are written, the second under a temporary name, for the user to combine. Honest about being unfinished.
- **Edit…**: opens VS Code's own 3-way merge editor scoped to this function **[check whether it can be opened on virtual documents]**, else the file.
- The docs of the function are one click away. This is the moment a "why" note is most valuable.
- The canvas shows where conflicts are: red cards, roll-ups on parents, "next conflict" jumps between them.

### What stays hard

| Problem | Why | Plan |
|---|---|---|
| **The conflicted file does not parse** | git has written `<<<<<<<` markers into it | Do not outline the working file. Read the three clean versions from git's index: `git show :1:path` (base), `:2:path` (ours), `:3:path` (theirs). |
| **Rest of file** | imports and constants are in no function; both sides often add an import | Line-merge that part. Imports are a known special case: a set union is usually right, but that is language knowledge → a driver capability later, a plain line conflict at first. |
| **Order** | where does a function added by theirs go in our file? | After the same neighbour it follows in theirs; if that neighbour is gone, at the end of the parent. |
| **Building the result** | the merged file is assembled from pieces of three files | Assemble, then **outline the result**. If it does not parse, or a chosen function is missing from it, refuse and fall back to the normal merge. Never write a file V-NOC cannot read back. |
| **Marker conflicts** | both branches attached a doc to the same function | Already solved by design: hash-derived IDs give both branches the **same** `ID:` line ([`../../05-identity/01-id-generation.md`](../../05-identity/01-id-generation.md) §2). |
| **`.vnoc/` conflicts** | two people edited the same doc | Plain Markdown: git's normal merge is the right tool. Show it on the node, do nothing clever. |
| **Trust** | one wrong automatic merge and nobody uses it again | Every automatic decision is listed and reversible before anything is written; the feature starts as *suggestions on top of git's result*, not as a replacement. |

## 4. The safety rules

The same spirit as [`../../04-storage/03-writing-safely.md`](../../04-storage/03-writing-safely.md):

1. **Every change to source code is a text edit applied by VS Code.** Undo works. The core and the driver still never write source files.
2. **Check before and after.** Before: the range still holds the text the diff was computed from (compare the hash; refuse if the user typed since).
   After: outline the result; the file must parse and the node must be there.
3. **V-NOC never commits, never resolves the merge for git, never runs `git add` without a click on an action whose name says so.** It prepares the
   file; the user finishes the merge the usual way.
4. **Whole-file fallback is always one click away.** "Open the normal merge editor" on every conflict.
5. **Nothing is lost.** Before a multi-function resolution is applied, the three input texts are still in git's index; V-NOC adds no state of its own.

## 5. How deep to go: three options

| Option | What it is | Advantage | Disadvantage |
|---|---|---|---|
| **1. Navigator** | V-NOC shows *which functions* conflict and opens VS Code's merge editor at the right place. No merging of its own. | Tiny. No risk. Already better than a list of files. | Does not remove false conflicts. |
| **2. In-editor resolver** *(recommended target)* | The decision table and the conflict card above, inside the extension, for a merge that git has already stopped on. | Removes most false conflicts; the user sees and approves everything. | Real work: 3-way matching, assembling files, the hard cases above. |
| **3. Git merge driver** | A `vnoc merge-file` program registered in `.gitattributes` (`*.py merge=vnoc`), so `git merge` itself uses function-level merging, also on the command line and in CI. | Works everywhere git runs. | Runs with no UI and no human: it must be right or decline. Needs the CLI ([`../02-agent-access/02-cli-and-skill.md`](../02-agent-access/02-cli-and-skill.md)) and a driver on every machine. Tools that do only this exist (for example Mergiraf **[check]**); study them before competing. |

Go 1 → 2. Treat 3 as a separate product decision, taken only if 2 is trusted in daily use.

Next: [`05-options-and-path.md`](05-options-and-path.md)
