# 17. Release 0.3: edit in the canvas

Written 2026-11-12. The build plan for 0.3. It replaces the 0.3 row of [`02-mvp-and-releases.md`](02-mvp-and-releases.md) §3 ("Monaco as a view of
VS Code's document"), and it makes spike S5 much smaller. Where this file is more exact, it wins.

```
edit in the canvas
├── 1. what 0.3 ships
├── 2. decisions
├── 3. why a draft, not a live view
├── 4. a draft: its states
├── 5. opening a draft
├── 6. saving a draft
│   ├── the three cases
│   ├── putting the indent back
│   └── writing it
├── 7. edge cases
├── 8. the doc panel: the same rule for Markdown
├── 9. what is not part of this
├── 10. order of work
└── 11. tests
```

## 1. What 0.3 ships

Open a function's code on its card, press **Edit**, type, press **⌘S**. The function in the file now has the new text. The tree, the doc badges
and the call trees follow after the save, as they do today when you save in a normal editor.

| In | Out (later) |
|---|---|
| editing one function, class or file slice in its card | a live view: the card and an editor tab showing each other's typing |
| ⌘S in the card writes it into the file | the outline, ranges or call trees following unsaved text |
| a function that was changed in the editor is never overwritten | undo that spans the card and the tab |
| editing a doc in the doc panel, by the same rule (§8) | syntax errors under the cursor while typing |

## 2. Decisions

| # | Decision | Why |
|---|---|---|
| **E1** | **The card edits a draft.** Nothing leaves the card until ⌘S. | Nothing to send per keystroke, so nothing re-runs, and no edit bounces back and forth between the card and VS Code's document. |
| **E2** | **The index follows saves only**, as now (`extension/src/core.ts:122`, the file watcher). No outline, range or call tree of unsaved text. | Following unsaved text means a re-outline, moved ranges and a new call tree on every keystroke. |
| **E3** | **Save checks before it writes:** the document's version, then the function's text against the draft's starting text (§6). A function changed in the editor is never overwritten. | One check replaces a live view. |
| **E4** | **Save writes through a `WorkspaceEdit`, then saves the file only if it was not dirty before**: the rule `relink.ts`, `addDoc.ts` and `writeId.ts` already follow. | Unsaved work of the user's own in a tab is never saved for them. |
| **E5** | **Editing a file that has unsaved changes is refused**: "Save `<file>` first", with a **Save and Edit** button. | The card's range comes from the last save. In a dirty file it can point at the wrong lines (§5). |
| **E6** | **A draft lives in the webview** (React state, copied to `vscode.setState`), not in the extension. | The panel already has `retainContextWhenHidden`, so hiding the tab keeps the draft; `setState` keeps it through a webview reload. The extension stays stateless about drafts. |
| **E7** | **The live view (S5's first question) is later**, and it is not a format change. | Nothing on disk depends on how the card edits. |

## 3. Why a draft, not a live view

The 0.3 row in `02` said "Monaco as a view of VS Code's document". That means two copies of the text kept equal on every keystroke, in both
directions. It brings in:

- **edits that bounce**: the card writes to the document, the document's change event comes back to the card, the card applies it again;
- **range tracking**: the function's range moves with every edit above it, and the card has to move it too, without the driver;
- **re-runs**: everything that hangs on a range (badges, Missing, calls) would want to follow, or show stale results.

A draft has none of these. Its one cost is that the card and an open tab do not see each other's unsaved typing, and §6's check makes that safe.

## 4. A draft: its states

```
reading ──Edit──► editing ──⌘S──► saving ──ok──► reading (the new text, after the index updates)
   ▲                 │               │
   │                 │               ├──changed in the editor──► conflict ──Reload──► editing (new start)
   │                 │               │                               └────Show Diff──► a diff tab
   └──Discard────────┘               └──the function is gone──────► gone ("open in editor")
```

| State | The card shows |
|---|---|
| reading | today's read-only Monaco. A dot and "unsaved in editor" when the file has unsaved changes (today the slice can be wrong then, unmarked) |
| editing | Monaco writable, a dirty dot when the text differs from the start, **Save** and **Discard** in the card's footer |
| saving | the same, disabled, for the time one request takes |
| conflict | "Changed in the editor since you started": **Reload** (keep nothing), **Show Diff** (the draft against the file) |
| gone | "Not found in `<file>` any more": **Open in Editor**, **Copy Draft** |

Closing a card in editing asks "Save / Discard / Cancel".

## 5. Opening a draft

**Edit** on the card (and on the node menu, `16-ui-redesign/08`) does this:

1. The webview asks the host `editStart { path, range }`.
2. The host opens the document. **If it is dirty, it answers `{ dirty: true }`** and the card shows E5's message. **Save and Edit** saves the
   document and starts again from 1; the save makes the index update, so the range is fresh.
3. Otherwise the host answers `{ text, version, prefix }`:
   - `text`: the range's text, as `codeOf` reads it today (`extension/src/commands.ts:300`);
   - `version`: `document.version`;
   - `prefix`: the text of the first line **before** `range.start` (the `def`'s indentation, tabs or spaces as written).
4. The card keeps `{ key, path, range, version, base: text, prefix }` and shows the text dedented, as today (`NodeCode.tsx`, `dedent`).

## 6. Saving a draft

The webview sends `editSave { key, path, range, version, base, prefix, text }`. Everything below runs in the extension.

### The three cases

| Case | What the host does |
|---|---|
| **the document's version is still `version`** | write `text` at `range` |
| **the version changed, and the function's text still equals `base`** (an edit elsewhere in the file) | ask the core where the node is now: `vnoc/locate` (below). Write at the new range |
| **the version changed, and the function's text differs from `base`**, or `vnoc/locate` finds it with different text | answer `{ conflict: true }`; write nothing |
| `vnoc/locate` finds no such node | answer `{ gone: true }`; write nothing |

**`vnoc/locate { key, document }` → `{ range } | null`**, a new core method. `document` is the `{ uri, version, text }` that `vnoc/attach`
and `vnoc/writeId` already take. The core outlines that text (the driver's `vnoc/outline` already accepts `text`, `06-drivers.md` §2) and finds
the node with that key, by ID first, then by qualname, as `identity/resolve.rs` does. It is **one driver call per save**, and only in the second
row. Nothing is cached.

In the second row the host reads the text at the new range and compares it with `base` before it writes. A match by qualname whose text changed
is the third row, not the second.

### Putting the indent back

The card shows the text with the first line's indent taken off the other lines. On save, every line after the first that is not empty gets
`prefix` back in front. The first line goes in as it is (the range starts after `prefix`). A test pins that `dedent` then re-indent gives back
the same bytes for spaces, tabs, empty lines and a docstring with less indent than the `def` (`dedent` takes off only what is there).

### Writing it

1. `wasDirty = document.isDirty` (false, by E5, unless the user typed in the tab since the draft started).
2. A `WorkspaceEdit` that replaces the range with the re-indented text. If `applyEdit` returns false, answer `{ conflict: true }`.
3. If `!wasDirty`, `document.save()`. The watcher sends `vnoc/filesChanged`; the index updates; the card gets its new range with the next
   structure, as any card does after a save.
4. If `wasDirty`, the text is in the editor's buffer but not on disk. The card says "Written into `<file>`, not saved", and the index follows when
   the user saves the tab.

## 7. Edge cases

| Case | What happens |
|---|---|
| the draft deletes or changes the `ID:` line | before writing, the host checks `base` has an `ID:` line the text does not. It asks "This removes the node's ID: its docs will be Missing. Save Anyway / Cancel". After a save, the existing "ID was removed" flow takes over (`11-missing-relink-remove.md`) |
| the draft renames the function, or splits it into two | written as any edit. After the save the node is found by ID if it has one, else it goes Missing as after a rename in the editor. The card follows the ID, or shows "gone" |
| the file was deleted or renamed while the draft was open | `openTextDocument` fails, or `vnoc/locate` finds nothing: gone |
| two cards on the same function | the second **Edit** is refused: "Already being edited in another card"; that card is revealed |
| the same file in two cards (two functions) | each draft is its own range. The first save changes the version, so the second save takes the second row of §6, one `vnoc/locate` |
| ⌘S when the focus is in the card | Monaco's `addCommand(CtrlCmd + S)` saves the draft. VS Code must not also run its own Save **[check in 9.0]** |
| the window closes with a draft open | the draft is lost. VS Code cannot ask a webview before closing. The canvas's bar shows "1 unsaved draft" while there is one, so it is visible |
| a file and a folder card | file: the slice is the whole file, same flow. A folder has no code, so no **Edit** |

## 8. The doc panel: the same rule for Markdown

F7 (`08-decisions-to-log.md`) moved editing a doc inside the panel to 0.3. It uses the same draft, with the whole file as the range:

- **Edit** in the panel starts a draft of the `.md`, refused if the file is dirty (E5).
- **⌘S**: if the version is the same, write it; if not, and the file's text still equals `base`, write it; otherwise conflict. There is no
  `vnoc/locate`: the range is the whole file.
- The frontmatter is shown and edited as text. The core reads it on the next `filesChanged`, as after an edit in a tab.

## 9. What is not part of this

- The live view of §3. If users ask for it, it gets its own plan, and S5 is run for it then.
- Outlines, call trees, syntax errors or badges of unsaved text (E2).
- Formatting on save, completions, go to definition inside the card: Monaco in the webview has no language server. "Open in Editor" is the way.
- Undo after a save from the card: it is VS Code's undo, in a tab of that file.

## 10. Order of work

On a branch `0.3-edit` from `main` after 0.2 is merged. Each row is one commit and leaves `pnpm -r test`, `cargo test` and the window suite green.

| # | Build | Proof |
|---|---|---|
| **9.0** | three checks, written into this file: ⌘S in a webview Monaco with `addCommand` and VS Code's Save both bound · `WorkspaceEdit` on a document that has moved on (Q4 in `08`) · `vscode.setState` with a 100 KB draft | the answers in §7 and §6, the **[check]** marks removed |
| **9.1** | reading: the card marks "unsaved in editor" when the document is dirty (`codeOf` returns `dirty` beside the text) | a window test: type in a tab, the card shows the mark |
| **9.2** | `editStart` in the relay and `commands.ts` · the card's editing state, Save and Discard, the draft in `setState` · `dedent` / re-indent as a pair | the re-indent tests (§11 a) · by hand: Edit, type, Discard |
| **9.3** | `editSave`, the first row of §6, and E4's save rule | a window test: edit a function in the card, ⌘S, the file on disk has the new text and the tree updates |
| **9.4** | `vnoc/locate` in the core · the second and third rows · the conflict and gone states | the core test (§11 b) · window tests (§11 c, d) |
| **9.5** | §7: the `ID:` line warning, one draft per node, the close question, "1 unsaved draft" in the bar | window tests (§11 e) |
| **9.6** | §8: the doc panel's editor | a window test: edit a doc in the panel, ⌘S, the `.md` on disk changed |

## 11. Tests

| | Where | Checks |
|---|---|---|
| a | `webview/src/node-card/indent.test.ts` | `reindent(dedent(x)) == x` for spaces, tabs, empty lines, a short-indented docstring, a one-line function |
| b | `crates/vnoc/tests/` (lifecycle) | `vnoc/locate` on a text where a function moved down 10 lines: the new range · by qualname when it has no ID · `null` when it was deleted |
| c | window suite | draft open → a line added at the top of the file in a tab and saved → ⌘S in the card writes at the new place |
| d | window suite | draft open → the same function changed in a tab and saved → ⌘S in the card: conflict, the file is unchanged |
| e | window suite | a draft that removes the `ID:` line asks first · a tab that was dirty is not saved by the card's ⌘S |

Next: back to [`README.md`](README.md)
