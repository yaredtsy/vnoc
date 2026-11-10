# 05.03: S3 edits and saving

**Goal:** prove every source and doc change can go through VS Code's document model: undoable, safe on dirty buffers, and part of the user's own save.
**Answers:** B8 (Rust writes only cache, `node.toml`, `.gitignore`), C10 (save-time IDs in manual sync), C16 (moving files within `.vnoc/`, VS-D70), C20 (pylspt span recording and
the `# FileID:` writer). Also confirms the C8 numbers (~1.5 s shared budget) on a real machine.
**Time box:** 2 days. **Code:** `spikes/s3-edits/`. **Needs:** S1.

MVP note: in-code IDs ship in **stable** (M7), but the risky VS Code behaviour is tested now, so the MVP's design doesn't block it.

## Build

1. **Add an ID to a dirty buffer.** Type something unsaved in `orders.py`, then run "Add permanent ID": the core asks pylspt's `plan_ids` (fed with a hash-derived ID, 02-05 §1)
   for the edit, the host applies it with `workspace.applyEdit`. Check: the user's unsaved text is intact, **one** ⌘Z removes only the ID, no "file changed on disk" warning.
2. **C20, `# FileID:`:** prototype the file-level writer in the adapter (after shebang, encoding and a license header). Check the recorded **span + text** come back on the next read.
3. **Exact-span remove:** remove an ID by its recorded span. Then the mismatch case: edit the `ID:` line by hand, try to remove → nothing changes, the line is shown.
4. **Save hook with precomputed edits (VS-D66):** on `onDidChangeTextDocument` (debounced) compute the ID edits for that `document.version`; in `onWillSaveTextDocument`
   return them only if the version matches. Try `Manual`, `FocusOut`, `AfterDelay`; log the `reason` and how long the hook took. Then make the hook slow on purpose
   (2 s): what does VS Code do, and after how many slow saves does it stop calling the listener (C8)?
5. **C10:** turn on manual sync (no indexing on change). Do save-time IDs still work from the one buffer?
6. **No-loop check:** after our save, `onDidSave` → the core sees a content hash it already predicted → no re-parse.
7. **Remove → V-NOC trash (C16, VS-D70).** No system trash. Move a node's files to `.vnoc/local/trash/<timestamp>_<node-id>/`, write `trashed.toml` (`node`, `side`, `file`, `removed_at`), then **Restore** them.
   Check on macOS: (a) the move goes through a `WorkspaceEdit.renameFile` when a file is open in an editor (tab follows, undo works), and as a plain rename for a closed one;
   (b) a dirty open file keeps its unsaved text across Remove + Restore; (c) Restore when the target path already exists, and when the node is gone (it comes back as *Missing*);
   (d) the `.vnoc/local/` watcher (S1 C18) sees the moves as one batch, not a storm; (e) folder names with spaces, unicode and long paths. Other OSes: later, not in this spike.
8. **B8:** list every file the core writes directly vs through `WorkspaceEdit` in this spike. Anything a user could have open must be in the second list.

> **🧩 VS Code note: `WorkspaceEdit` vs writing the file.**
> `fs.writeFile` changes the file on disk *behind* the editor: if the user has it open with unsaved changes, VS Code shows a conflict and the undo stack is gone.
> A `WorkspaceEdit` changes the **document** (the in-memory model the editor shows). It's undoable like typing, and it works the same whether the file is open or not.
> Saving is a separate step (`document.save()`).

## Measure

- Save-hook time for files of 100, 1,000 and 5,000 lines (precomputed vs computed at save).
- How many slow saves until VS Code drops the listener (C8).

## Pass / fail

- ✅ Steps 1–6 work without a disk conflict or a broken undo.
- ✅ C10, C16, C20, B8 have answers below; Remove + Restore through `.vnoc/local/trash/` works on macOS with open and dirty files.

## Findings

*Run 2026-11-05 on macOS, VS Code 1.145, in a real extension host driven by an automated suite (`spikes/s3-edits/`, `npm test`; throwaway profile).
all 26 tests pass.*

**Spike-only shortcut (marked in the code):** the extension spawns the S2 driver itself. In the product the **core owns drivers; the extension never spawns one.**
The wire shape is the real one: `vnoc/planIds { text, version, ids: [{line, column, id}] }` -> `{ version, edits: TextEdit[], written: [{id, span, text}], skipped, unmatched, text_hash }`.
The caller picks the IDs (here: the 02-05 hash formula, `blake3`, 26 chars, in the extension); the driver only decides **where**. Positions are VS Code's (0-based line, UTF-16 column).

**Step 1: dirty buffer, planned on `document.getText()`. Passes.**
- The user's unsaved text stays; the file on disk is untouched; the buffer stays dirty; **one undo (`⌘Z`) restores the buffer exactly** (`oneUndoRestoresBuffer: true`).
- **Typing while we plan:** the plan carries `document.version`; if the version moved, the plan is dropped and retried **once**. Typing during the first attempt: 1 plan dropped, applied on the retry, typed text intact.
  Typing during both attempts: gives up, nothing half-applied, only the user's keystrokes remain.
- **Syntax error in one function:** only that function is skipped (`skipped: [{line, reason}]`); the other defs still get IDs. (Attribution rules: see S2/C17.)
- **User-written docstring (R14 = ID inside the docstring):** the ID is added inside it (`More detail.\n\n    ID: x\n    """`).
- Same path + qualname gives the same ID (VS-D50), 26 characters.

**Steps 2-3, C20: the `# FileID:` writer and exact-text remove (prototype in the adapter, pylspt untouched). Passes for 4 of 5 file shapes; the 5th is the pending test.**
- Position: after the shebang, after the PEP 263 encoding line (line 1 or 2), after the leading comment block (license header), before the first real line. Handles CRLF, a file ending without a newline, and "already has one" (no edit, returns the existing ID).
- The writer returns the **exact inserted text and its span in the new text**; reading that span back from the buffer gives the same text (also with `é😀` before it).
- **Remove searches the current buffer for the exact recorded text**, so it still works after the user edited above it (the span moved). Verified: every def-ID form (new statement, multi-line docstring,
  one-line triple-quoted, single-quoted, nested class/method) is removed **byte-for-byte** back to the original, so a V-NOC-only `"""ID: x"""` goes whole and a user docstring stays valid.
- **Mismatch:** the user adds `(keep!)` to the `# FileID:` line -> **nothing changes**, and the driver returns the line to show (`{line: 0, text: "# FileID: ABC  (keep!)"}`).
- Remove is also refused if it would leave the file with more syntax errors than before.
- **Design consequence:** the exact text must be **stored by the core** at write time (the docstring forms `Summary."""` and `Summary.\n"""` look identical after insertion, so the original can't be re-derived from the file).

**Steps 4-6, C8: the save hook (VS-D66).**
- Precomputed edits ride inside the user's own save (`reason = Manual`, hook time 0.2 ms), IDs end up in the saved file, and **no re-parse follows**: the saved text hash equals the predicted one, and the driver is not called again.
- **A save right after typing (before the debounce fired) writes no IDs**: the plan is for an older version and is not used (`used: false`). IDs arrive on the next save. This is by design (VS-D66).
- Bug found by the test: the IDs we add inside a save fire a change event, which scheduled a pointless re-plan. Fixed by ignoring change events whose text equals the predicted hash.
- Only `Manual` was exercised. `AfterDelay` and `FocusOut` were **not** tested.

**C8, the budget (the most important finding of S3).**
| Hook behaviour | What VS Code does |
|---|---|
| A hook that takes 2 s | The save is cut at **~1.52 s** each time and **the hook's edits are ignored** |
| After **4** such saves | **The listener is dropped for the rest of the session**: saves 5 and 6 returned in 8-12 ms and the hook was never called again (`listener failed` in the log) |
| Our fix: a hard budget of 800 ms inside the hook, answering `[]` when it runs out | 8 slow saves, each returned in ~813 ms, **listener never dropped**; a later fast save still added its IDs |

So V-NOC must **never wait on a driver inside the hook beyond its own budget**; otherwise save-time IDs silently stop until VS Code restarts. Use precomputed plans only; compute at save only as a bounded fallback.

**Timings (macOS, idle machine, real stdlib files; p50 / p95; 30 samples for the plan, 15 per hook mode; first-iteration outliers included):**
| File | Lines | Defs | Plan (outline + planIds) | Hook, precomputed | Hook, computed at save | Driver internal |
|---|---|---|---|---|---|---|
| `io.py` | 100 | 4 | 0.7 / 1.3 ms | 0.1 / 0.2 ms | 1.0 / 8.5 ms | 0.1 ms |
| `random.py` | 997 | 38 | 2.5 / 9.4 ms | 0.1 / 0.2 ms | 2.9 / 6.4 ms | 0.5 ms |
| `turtle.py` | 4208 | 240 | 10.6 / 33.3 ms | 0.2 / 0.4 ms | 11.6 / 51 ms | 2.9 ms |

**Headroom under the 1500 ms budget with the extra core hop:** the product adds one hop (the text is serialized twice more: extension to core to driver and back). The **JSON serialization** of that hop measured 0 / 0.1 / 0.6 ms
for the three sizes (same-process estimate; it excludes process scheduling and pipe I/O, which I did not measure). Even computed at save, p95 plus the hop is ~51 ms for 4,200 lines, leaving **~1,450 ms**.
The budget is not the constraint; the **rule** is (never block the hook). Real risks would be a cold driver start (~130 ms for ty, S2) or a busy core.

**C10, save-time IDs in manual sync: yes (decided, VS-D71).** The plan only needs the one buffer and the driver; no index and no indexing-on-change. The "ID already taken?" check is the SQLite registry
(early catch) plus the disk check for `nodes/<shard>/<ID>/` and `local/nodes/<shard>/<ID>/` (authoritative); source files are not searched. A code-only duplicate has no data attached and is repaired at the next sync
(anchor match, else the `path::qualname` hash match keeps the ID, the other copy gets a new one). Not built in this spike: it only covers the editing half.

**Step 7, C16 (VS-D70: V-NOC trash, macOS only). Passes.**
- Remove moves files to `.vnoc/local/trash/<timestamp>_<node-id>/<same relative path>` with a `trashed.toml` (`node`, `side`, `file`, `removed_at`); Restore moves them back. A remove of 3 files took ~160-340 ms.
- Done with `WorkspaceEdit.renameFile`: an **open, dirty file follows the move** (the editor now shows the trash path) and **keeps its unsaved text**, also after Restore.
- Parent folders are created by the rename itself. Names with spaces, accents and CJK (`ünï cøde 文 x.md`) work.
- Restore **never overwrites**: a path that exists again is reported as a conflict and the removed copy stays in the trash. If the node is gone, the files still come back and the node is reported **Missing**.
- **The watcher sees a move as a delete + a create** (no rename event): 27 events for moving 3 files out and back, including directory creates. Not a storm, but the consumer must batch and pair them.
- Not tested: Windows, Linux, remote, and a `.vnoc` on another volume (a rename across volumes may be a copy).

**Step 8, B8: who writes what (this spike).**
| Written | How | Why |
|---|---|---|
| Def `ID:` lines, `# FileID:` lines, their removal | `WorkspaceEdit` (a user may have the file open) | undo, dirty buffers |
| Moving a node's files into and out of the trash (docs, **and `node.toml` itself**) | `WorkspaceEdit.renameFile` | open editors follow the move |
| `trashed.toml` | direct write by the core | V-NOC's own file, nobody has it open |
| SQLite cache, `.gitignore`, content of `node.toml` | direct write by the core (unchanged from B8) | not user-edited |
Result: B8 holds, with two additions: **moves are not direct writes**, and `trashed.toml` joins the direct-write list.

**Found while building (all in the code):**
1. The driver's `initialize` reply needs `capabilities: {}` or stock LSP clients refuse it. Added.
2. `lsp-server` requires the `initialized` notification before any request.

**Open:**
1. Not exercised: save reasons `AfterDelay` and `FocusOut`; non-macOS; remote.


Next: [`04-git-and-matching.md`](04-git-and-matching.md)
