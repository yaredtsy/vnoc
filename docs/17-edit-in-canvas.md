# 17. Edit in the canvas

Written 2026-11-12; brought up to date with the code on 2026-11-16 (after 0.2). The build plan for editing in the canvas. **It is not tied
to a release number** (your call, 2026-11-16): it ships in whichever release comes next, and no step here changes the version. It replaces the 0.3 row of [`02-mvp-and-releases.md`](02-mvp-and-releases.md) §3 ("Monaco as a view of
VS Code's document"), and it makes spike S5 much smaller. Where this file is more exact, it wins.

```
edit in the canvas
├── 1. what it ships
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

## 1. What it ships

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
| **E2** | **The index follows saves only**, as now (`extension/src/core.ts:172`, the file watcher). No outline, range or call tree of unsaved text. | Following unsaved text means a re-outline, moved ranges and a new call tree on every keystroke. |
| **E3** | **Save checks before it writes:** the document's version, then the function's text against the draft's starting text (§6). A function changed in the editor is never overwritten. | One check replaces a live view. |
| **E4** | **Save writes through a `WorkspaceEdit`, then saves the file only if it was not dirty before**: the rule `relink.ts`, `addDoc.ts` and `writeId.ts` already follow. | Unsaved work of the user's own in a tab is never saved for them. |
| **E5** | **Editing a file that has unsaved changes is refused**: "Save `<file>` first", with a **Save and Edit** button. | The card's range comes from the last save. In a dirty file it can point at the wrong lines (§5). |
| **E6** | **A draft lives in the webview** (a store beside `webview/src/state/view.ts`, saved with the same `vscode.setState` call: `setState` keeps one object per webview, so the draft must go into `view.ts`'s `Saved`, not a second call that would overwrite it), not in the extension. | The panel already has `retainContextWhenHidden`, so hiding the tab keeps the draft; `setState` keeps it through a webview reload. The extension stays stateless about drafts. Measured in 9.0 (check C): 100 KB and 1 MB survive hiding the tab and *Developer: Reload Webviews*, and one `setState` call takes about 1 ms at either size. **A `setState` that is not `view.ts`'s own is dropped within seconds**: its subscriber writes its own object at the first view change, even a hover, and `probe` was gone in under 2 s with nothing touched. That is why the draft goes into `Saved`. |
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
| reading | today's read-only Monaco. A dot and "unsaved in editor" in one line above the code (the code area, not the header) when the file has unsaved changes (today the slice can be wrong then, unmarked) |
| editing | Monaco writable, a dirty dot when the text differs from the start, **Save** and **Discard** in the card's header (below) |
| saving | the same, disabled, for the time one request takes |
| conflict | "Changed in the editor since you started": **Reload** (keep nothing), **Show Diff** (the draft against the file) |
| gone | "Not found in `<file>` any more": **Open in Editor**, **Copy Draft** |

Closing a card in editing asks "Save / Discard / Cancel".

**The header's buttons by state** (decided 2026-11-16; the header is `16-ui-redesign/04-node-cards.md` §3):

| The card | Header buttons |
|---|---|
| code closed | Focus · Go to Code · `</>` · Add Doc (unchanged) |
| code open, reading | Focus · Go to Code · `</>` · **Edit** · Add Doc |
| editing | Focus · Go to Code · **Save** · **Discard** · Add Doc |
| saving | `editing`'s buttons with **Save** and **Discard** disabled |
| conflict, gone | `editing`'s buttons with **Save** disabled |

**Call cards edit their callee** (decided 2026-11-16). A call card has `</>` today, so in reading it gets **Edit** too: Explore · Go to Code · `</>` · **Edit**. While editing it shows Explore · Go to Code · **Save** · **Discard** (the same states as the table: `saving` disables both, `conflict` disables Save). The draft is keyed by the callee's key, not by the card, so 9.5's one-draft-per-node rule covers a structure card and a call card on the same function. A card with a code slot is a class or function card with a range, or a call card whose callee is known and has a range. A file card has none (below, §7).

- Nothing appears, moves or hides on hover (`04` §3): the buttons change with the card's state only, and **Save** stays in the same place for the whole edit. It is disabled while the draft equals `base` and enabled once the draft differs. The dirty dot before the name shows at the same time.
- `</>` and **Edit** give their two slots to **Save** and **Discard**, so the code cannot be closed with its button in the middle of a draft. Closing the card still asks Save / Discard / Cancel.
- Codicons: `edit`, `save`, `discard`. A tooltip names the key where there is one: "Save (⌘S)".
- The footer has no Save or Discard. The conflict and gone states keep their buttons (**Reload**, **Show Diff**; **Open in Editor**, **Copy Draft**) in the card's body.
- With the code open in the reading state the header holds five buttons, and a name gets about 11 characters instead of 14.

## 5. Opening a draft

**Edit** on the card (the header's button, `cardButtons.ts`; and on the node menu, `16-ui-redesign/08`: the entry `vnoc.editInCard` in `4_code` after Go to Code, on a function, a class or a call card with a known callee, on the canvas only) does this:

1. The webview asks the host `editStart { path, range }`.
2. The host opens the document. **If it is dirty, it answers `{ dirty: true }`** and the card shows E5's message. **Save and Edit** saves the
   document and starts again from 1; the save makes the index update, so the range is fresh.
3. Otherwise the host answers `{ text, version, prefix }`:
   - `text`: the range's text, as `codeOf` reads it today (`extension/src/commands.ts:383`);
   - `version`: `document.version`;
   - `prefix`: the text of the first line **before** `range.start` (the `def`'s indentation, tabs or spaces as written).
4. The card keeps `{ key, path, range, version, base: text, prefix }` and shows the text dedented, as today (`NodeCode.tsx`, `dedent`).

**While a card has a draft it never reads its code again** (decided 2026-11-16). In reading, `NodeCode` reads again on a new range and on a flip of its file's dirty state (9.1). In `editing`, `saving`, `conflict` and `gone` it keeps the draft and the draft's own `{ path, range, version, base, prefix }`, whatever the index or the tab does: a change under it is found at save time, by §6's checks, not by moving the draft. Discard drops the draft, and the card reads again, as in reading.

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
the same bytes for spaces, tabs and empty lines, when every line after the first starts with `prefix` (`dedent` takes off only what is there).

**Gap (found in 9.2a, not decided).** The pair is not an inverse when a line does not start with `prefix`, so the claim "a docstring with less indent than the `def`" cannot hold: `dedent` takes off what is there and `reindent` gives back the whole prefix. Three cases lose bytes, and `indent.test.ts` pins each as a known loss:

| Line in the file | After `dedent` then `reindent` |
|---|---|
| a line indented less than `prefix` (a docstring line at the margin) | indented as `prefix`: **a docstring's text changes** |
| a line of only whitespace | empty |
| a blank line of a CRLF file | `dedent` drops its `\r`, and a blank `\r` line that is kept gets the prefix |

A draft saves only when it differs from its start, but it rewrites the whole range, so one edit can change a line it did not touch. Options, **ask**: (1) `editStart` refuses a slice that does not round-trip, "This function has lines indented less than its `def`: edit it in the editor" (never changes a byte the user did not type; cannot edit such a function in the card); (2) treat a whitespace-only line as empty in both directions, and refuse only the margin lines and CRLF (fewer refusals); (3) the draft keeps what each line lost and `reindent` gives it back (exact; more code, and a typed edit on such a line needs a rule). Recommendation: (1) in 9.3, before the first write, with (2) if it refuses too much of real code.

### Writing it

**The check is ours** (step 9.0, check B). A plain `WorkspaceEdit` carries no document version, and `vscode.workspace.applyEdit` applies it even after the document changed: with a line added at the top, an edit at the old range returned `true` and replaced the wrong lines. VS Code does no version check. The only one in the product is the language client's, for the core's `workspace/applyEdit` (`vscode-languageclient` 9.0.1, `lib/common/client.js:1457`; its converter drops the version from the edit), and it protects only the core's edits (`vnoc/attach`, `vnoc/writeId`, relink). `editSave` is the first extension-side text write, so the version and `base` checks of the table above are written in our code.

1. Read `document.version` and the text at the range. Choose the row of the table from those two. **No `await` between that read and the call to `applyEdit`**: any await lets the document move on, and nothing after it would notice.
2. `wasDirty = document.isDirty` (false, by E5, unless the user typed in the tab since the draft started).
3. A `WorkspaceEdit` that replaces the range with the re-indented text. If `applyEdit` returns false, answer `{ conflict: true }`.
4. If `!wasDirty`, `document.save()`. The watcher sends `vnoc/filesChanged`; the index updates; the card gets its new range with the next
   structure, as any card does after a save.
5. If `wasDirty`, the text is in the editor's buffer but not on disk. The card says "Written into `<file>`, not saved", and the index follows when
   the user saves the tab.

**The `vnoc/locate` await (decided 2026-11-16).** The second row awaits `vnoc/locate`, and the document can change during that await. After it answers, read `document.version` again. If it is not the version the core was given, answer `{ conflict: true }`. Otherwise choose the row again, with no `await` before `applyEdit`, as step 1 says. One `applyEdit` round trip stays open (the edit is applied in the main thread after our call, and nothing of ours can close that): accepted. 9.4 pins the rule without a race: the save logic takes `locate` as a parameter, and a window test (§11 g) passes a fake `locate` that edits the document before it answers. It expects a conflict and the file unchanged.

## 7. Edge cases

| Case | What happens |
|---|---|
| the draft deletes or changes the `ID:` line | before writing, the host checks `base` has an `ID:` line the text does not. It asks "This removes the node's ID: its docs will be Missing. Save Anyway / Cancel". After a save, the existing "ID was removed" flow takes over (`11-missing-relink-remove.md`) |
| the draft renames the function, or splits it into two | written as any edit. After the save the node is found by ID if it has one, else it goes Missing as after a rename in the editor. The card follows the ID, or shows "gone" |
| the file was deleted or renamed while the draft was open | `openTextDocument` fails, or `vnoc/locate` finds nothing: gone |
| two cards on the same function | the second **Edit** is refused: "Already being edited in another card"; that card is revealed |
| the same file in two cards (two functions) | each draft is its own range. The first save changes the version, so the second save takes the second row of §6, one `vnoc/locate` |
| ⌘S when the focus is in the card | **Decided (9.0, check A).** While the card is `editing`, a `keydown` listener on the card's container (bubble phase) calls `stopPropagation()` for ⌘S, ⌘⌥S and ⌘⇧S, and for no other key. Monaco's `addCommand(CtrlCmd + S)` runs first and saves the draft. ⌘⌥S in the card saves the draft only. ⌘⇧S does nothing. **Windows and Linux (decided, not tried):** the same with Ctrl: the card swallows Ctrl+S and Ctrl+Shift+S while editing. Ctrl+K S (Save All) is a chord whose first key must reach VS Code, so it is not swallowed: VS Code saves its dirty files, and the draft, which is not a file, stays in the card, unsaved. Accepted. Every other key passes, so ⌘P, ⌘W, ⌘B… still reach VS Code with the focus in the card. Which keys a card swallows, by edit state, is a pure function next to the card with a table test (9.2, §11 f). `preventDefault()` alone does not stop VS Code (measured; §10) |
| the window closes with a draft open | the draft is lost. VS Code cannot ask a webview before closing. The canvas's bar shows "1 unsaved draft" while there is one, so it is visible |
| a file and a folder card | **file: later; folder: never** (decided 2026-11-16). A file card has no `</>` (`cardButtons.ts`: code in the card is for a definition with a range), so it has no code to edit and no **Edit**. A folder has no code at all |

## 8. The doc panel: the same rule for Markdown

F7 (`08-decisions-to-log.md`) moved editing a doc inside the panel to this plan. It uses the same draft, with the whole file as the range.

**Open since the redesign (decide before 9.6):** there are now two places that show a doc: the **docs panel** on the canvas (ⓘ,
`webview/src/docs/DocPanel.tsx`) and the **docs tab** beside the editor (`webview/src/docs/DocsApp.tsx`). Recommendation: both, since they
share `markdown.ts` and the draft rule is the same; if only one, the docs tab, because it is where J6 sends you with the canvas hidden.


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

On a branch `edit-in-canvas` from `main`. Each row is one commit and leaves `pnpm -r test`, `cargo test` and the window suite green.

| # | Build | Proof |
|---|---|---|
| **9.0** | three checks, written into this file: ⌘S in a webview Monaco with `addCommand` and VS Code's Save both bound · `WorkspaceEdit` on a document that has moved on (Q4 in `08`) · `vscode.setState` with a 100 KB draft | **done 2026-11-16.** The answers in §7, §6 and E6, the **[check]** marks removed; the measurements are under the table |
| **9.1** | **done 2026-11-16.** reading: the host holds the set of files with unsaved changes (`extension/src/dirtyWatch.ts`; the pure part, `dirtyFiles.ts`, has the table test) and pushes it as the event `dirtyFiles { paths }` whenever it changes: on a keystroke that flips `isDirty`, on a save, on an undo back to the saved text, on a revert, on a close without saving. A page does not wait for a push: it asks once when it starts (the request `dirtyFiles`, as it asks for the session), because an event sent before a new page listens is lost. The card (`NodeCode`) marks "unsaved in editor" in one line above its code, in the code area, while its file is in the set, and reads its code again when that flips. A note `codeShown { key, dirty, text }` for the tests. (Not a field of `codeOf`'s answer: an answer is stale the moment the user types in the tab, and the card asks only when it opens or its range changes.) **As built:** the canvas draws only the cards on screen (`03` §6.1), so a card that left the page shows no mark, and one that comes back reads its code, and its file's state, again. | a window test (`edit.test.ts`): code shown, no mark · an edit in the file, the mark · a save, no mark · an edit and a revert, no mark |
| **9.2a** | **done 2026-11-16.** The pure rules, no change on screen: `webview/src/node-card/indent.ts` (`dedent` and `reindent` with the `prefix` string; `NodeCode` uses it, with today's output) · `cardKeys.ts`: which keys an editing card swallows (§7) · `cardButtons.ts` takes the card's edit state (`closed`, `reading`, `editing`, `saving`, `conflict`, `gone`) and `ActionStrip` can draw Edit, Save and Discard. The card passes none yet, so it draws what it drew (Edit is a button of `reading`, and `reading` is 9.2b's) | the re-indent tests (§11 a) · the key table test (§11 f) · the `cardButtons` table test, every state, for a function card and a call card |
| **9.2b** | **done 2026-11-16.** `editStart` (`shared/messages.ts`, `relay.ts`, `extension.ts`, `commands.ts`) · the node menu's `vnoc.editInCard`: `4_code@3` of the node menu (a function or class, canvas only) and `4_code@4` of the call menu · the draft store `state/drafts.ts`, saved by `view.ts`'s one subscriber · `NodeCode` writable while `editing`, the dirty dot in `NodeHeader`, Discard, the key listener · Save does nothing until 9.3 (`saveDraft` in `state/edit.ts`) | a window test: the menu's Edit, the card says `editing` · E5 at the host: refused, answered, saved first · by hand: Edit, type, Discard · a reload keeps the draft · ⌘P and ⌘S with the focus in the card. **As built, where it differs from the plan:** (1) a draft also keeps `card`, the id of the card that started it: the draft is keyed by the function, and a second card on the same function must not show it editing too (9.5 puts the message on it). Until 9.5 an Edit on a node that has a draft is ignored. (2) **Save and Edit does not start from the host's answer.** With `saveFirst` the host saves and answers for the range the card sent, which is stale until the index has read the save. The card ignores that answer, waits for a newer index revision (3 s at most: a save that changed no byte may not bump it), reads the node's range again and then asks `editStart` as in step 1 (`state/edit.ts`, `saveAndEdit`). (3) The key listener is a native `keydown` listener on Monaco's container, bubble phase, as measured in 9.0; a React `onKeyDown` was not measured. (4) The refusal's message and button are one line above the code, where 9.1's mark is (it replaces the mark while shown). (5) `vnoc.editInCard` is shown for a node by `hasCode && (kind == function || kind == class)`. (6) The card posts a note `editState { key, state }` for the tests |
| **9.3** | `editSave`, the first row of §6, and E4's save rule | a window test: edit a function in the card, ⌘S, the file on disk has the new text and the tree updates |
| **9.4** | `vnoc/locate` in the core · the second and third rows · the conflict and gone states · the save logic takes `locate` as a parameter (§6, the re-check after it answers) | the core test (§11 b) · window tests (§11 c, d, g) |
| **9.5** | §7: the `ID:` line warning, one draft per node, the close question, "1 unsaved draft" in the bar | window tests (§11 e) |
| **9.6** | §8: the doc panel's editor | a window test: edit a doc in the panel, ⌘S, the `.md` on disk changed |

**Answers (2026-11-16).** A was done by hand in a real project, with a dirty `Untitled-1` as the witness and the key log of *Developer: Toggle Keyboard Shortcuts Troubleshooting*. B and C ran as throwaway probes (a window test and a self-reporting page), removed.

| Check | Answer |
|---|---|
| ⌘S in a webview Monaco | **VS Code runs its own Save as well, and `preventDefault()` does not stop it; `stopPropagation()` on the card does.** *No blocker, ⌘S:* Monaco's command ran (`PROBE save`), the key was `defaultPrevented` on the card and on `window` (Monaco prevents the default of a key it handles), and VS Code still logged `Invoking command workbench.action.files.save`. So the webview forwards a key that is `defaultPrevented`. Nothing was saved only because the active editor was the canvas. *`stopPropagation()` in a bubble-phase listener on the card:* ⌘S and ⌘⌥S both ran Monaco's command, VS Code received only the Meta and Alt keydowns, never the S, no `Invoking command`, no Save As dialog. *Capture phase on the card:* blocks ⌘S and ⌘⇧S too, but Monaco's command did not run (the key never reached it), so the save would have to be called from that listener. **Not run:** ⌘⌥S with no blocker (**inferred** to invoke `workbench.action.files.saveAll` as ⌘S invokes `files.save`: it is the reason to swallow it); ⌘⇧S with the bubble-phase blocker (**inferred** from ⌘S and ⌘⌥S: Monaco has no command on it, so the key reaches the listener); `preventDefault()` alone as a blocker, `both` and the React `onKeyDown` (not needed); a dirty tab in another group or in the same group (the witness was `Untitled-1`, the key log is the evidence); Ctrl+S in Monaco; ⌘S on the canvas outside Monaco. The decision is in §7. **Windows and Linux (decided, not tried):** all of this was measured on macOS. There `CtrlCmd` is Ctrl, so the card swallows Ctrl+S and Ctrl+Shift+S while editing. Save All is the chord Ctrl+K S, whose first key must reach VS Code: it is not swallowed, VS Code saves its dirty files, and the draft, which is not a file, stays in the card, unsaved. Accepted. Nothing here was run on Windows or Linux. `cardKeys.ts` takes the platform (§7) |
| `WorkspaceEdit` on a document that has moved on | **a plain one applies at the old range. The check is ours.** The document at version 1; a line inserted at the top (version 2, dirty); a second `WorkspaceEdit` at the old range of `def beta():`..`return 2`, no version: `applyEdit` returned `true`, version 3, and the text had `# REPLACED` over the blank line and the `def beta():` line, with `    return 2` left behind. For Q4: the **core's** `workspace/applyEdit` with a stale `version` (`vnoc/attach` told version 1 of a document at 2) was refused by `vscode-languageclient` itself (`lib/common/client.js:1457`; the converter drops the version from the edit): `{ applied: false }`, the core said "the editor did not apply the edit; the document changed meanwhile?", the text did not change. Every write to source text today goes through the core's `workspace/applyEdit` (`vnoc/attach`, `vnoc/writeId`, relink). The one extension-side `WorkspaceEdit` is `trash.ts:19`, which renames files. Written into §6 and Q4 |
| `vscode.setState` with a 100 KB draft | **works, and one call is cheap.** *100 KB:* three calls took 0.4, 1.2, 0.5 ms (the call only: the copy to the host is not in it). *1 MB:* 1.2, 0.8, 0.8 ms. Both survived hiding the tab and showing it (the page did not reload) and *Developer: Reload Webviews*: the next load read `probe` back at its full length (100 000, then 1 000 000), in the same object as `view.ts`'s fields (`codeOpen`, `docsOpen`, `docWidth`, `minimap`, `panelShares`; only their default values were read, as a probe cannot click a card). **The overwrite:** when `probe` was set by its own `setState`, it was gone at the first check, under 2 s later, with nothing touched: `view.ts`'s subscriber had written its own object. When every `setState` carried `probe`, as a draft inside `Saved` would, it stayed. Written into E6. Not measured: *Developer: Reload Window*; a draft typed in a card |

## 11. Tests

| | Where | Checks |
|---|---|---|
| a | `webview/src/node-card/indent.test.ts` | `reindent(dedent(x, p), p) == x` for spaces, tabs, empty lines, a docstring indented as the body, a one-line function, no prefix · the three cases of the §6 gap pinned as known losses (a margin docstring line, a whitespace-only line, a CRLF blank line) |
| b | `crates/vnoc/tests/` (lifecycle) | `vnoc/locate` on a text where a function moved down 10 lines: the new range · by qualname when it has no ID · `null` when it was deleted |
| c | window suite | draft open → a line added at the top of the file in a tab and saved → ⌘S in the card writes at the new place |
| d | window suite | draft open → the same function changed in a tab and saved → ⌘S in the card: conflict, the file is unchanged |
| e | window suite | a draft that removes the `ID:` line asks first · a tab that was dirty is not saved by the card's ⌘S |
| f | `webview/src/node-card/cardKeys.test.ts` | a table of state × key: ⌘S, ⌘⌥S and ⌘⇧S are swallowed in `editing` and in no other state · ⌘P, ⌘W, ⌘B, ⌘C, a plain `s` and (on macOS) Ctrl+S pass in every state · on Windows and Linux: Ctrl+S and Ctrl+Shift+S are swallowed in `editing` and nowhere else, Ctrl+K and Meta+S pass |
| g | window suite (9.4) | the save logic with a fake `locate` that edits the document, then answers: ⌘S's save answers a conflict and the file is unchanged (§6, the re-check) |

Next: back to [`README.md`](README.md)
