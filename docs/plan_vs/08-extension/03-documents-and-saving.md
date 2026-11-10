# 08.03: Documents, edits and saving

> **Status:** 📝 outline: written in full after **S3, S5** · **Program:** extension host (TS) · **Milestones:** M3, M6, M7

**Purpose:** The only part of the system that touches user-visible files: applying edits, serving document views to the webview, and the save hook.

**Builds on:** [02-05 §7 writing into source](../02-technology/05-identity-and-archive.md) · [02-03 editable everywhere](../02-technology/03-frontend-webview.md)

## Will define

1. **Applying `FileOps`:** create, edit, `renameFile` (including moves into the V-NOC trash), as one `WorkspaceEdit`; saving files that weren't dirty.
2. **Document views for the webview:** open, slice, follow changes, version-checked edits, the race rule.
3. **⌘S / Ctrl+S in the canvas:** the keybinding command, finding the focused document, `document.save()`.
4. **(Stable) the save hook:** precomputed ID edits per version, reasons, the time budget.
5. **Recognising our own changes** so they don't loop back as user edits.
6. **The V-NOC trash (VS-D70):** moving files into and out of `.vnoc/local/trash/` with `renameFile` (open and dirty files follow the move and keep their unsaved text), `trashed.toml`, Restore and its conflicts (never overwrite).
7. **Observed moves (VS-D76):** forwarding incremental `didChange` and `onWillRenameFiles` / `onDidRenameFiles` to the core; the reload test that ends tracking for a document.

## MVP vs stable

| MVP (0.x) | Stable (1.0) |
|---|---|
| FileOps, document views, ⌘S, V-NOC trash | the save hook for IDs |

## Questions to settle while writing

- Keep one `TextDocument` open per card, or open on demand?
