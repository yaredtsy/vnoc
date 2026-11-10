# 05.05: S5 canvas editors

**Goal:** edit a function's code and a doc **inside the canvas** as views of VS Code's documents, save with ⌘S / Ctrl+S, and never corrupt a file when the same text is edited
in a regular editor at the same time.
**Answers:** C15 (opening a library file read-only), C21 (Monaco in a webview), C23 (the ⌘S keybinding and picking the right document).
**Time box:** 2 days. **Code:** `spikes/s5-editors/`. **Needs:** S1's webview.

## Build

1. **Monaco in the webview (C21).** Bundle `monaco-editor` with Vite. Monaco needs **web workers**: allow them in the CSP (`worker-src ${webview.cspSource} blob:`) and load
   them through `asWebviewUri`. Map the theme from VS Code's CSS variables. Measure the bundle size and the time to the first editor.
2. **A slice of a real document.** The host opens `orders.py` with `workspace.openTextDocument`, sends lines 42–71 to a Monaco editor in a card.
   Typing in the card → the host turns each change into a `WorkspaceEdit` (offset = slice start + offset in the slice). Typing in a regular editor →
   `onDidChangeTextDocument` → the card updates.
3. **The race.** Type in the card and the regular editor at the same time (a script can do one side). Each webview edit carries the `version` it was based on; a stale one is
   re-applied if it doesn't overlap, else dropped and the card refreshed (02-03). After 1,000 random edits the file must equal what the regular editor shows.
4. **One undo, one dirty marker.** Undo in the regular editor undoes card edits too; the card's ● matches the tab's dirty dot.
5. **⌘S / Ctrl+S (C23).** Contribute `{ command: "vnoc.canvas.save", key: "ctrl+s", mac: "cmd+s", when: "activeWebviewPanelId == 'vnoc.canvas'" }`. Check: with a card focused,
   only our command runs; it saves **that card's** document; `onWillSaveTextDocument` sees `Manual`. With nothing focused it does nothing. Then the fallback (`keydown` in the webview).
6. **The doc panel.** A `.md` doc: rendered with `react-markdown`, **[✎]** switches to Monaco in Markdown mode on the same document, ⌘S saves it.
7. **C15, read-only library file:** jump to a `site-packages` function. Compare a read-only virtual document (a `TextDocumentContentProvider` on a `vnoc-lib:` scheme) with
   opening the real file and running *View: Toggle Read-Only in Session*. Which one keeps syntax colouring and go-to-definition?

> **🧩 VS Code note: the document vs the editor.**
> A `TextDocument` is the text model of one file, and there's only ever **one** per file in a window. Editors (tabs) are views onto it. Our webview editor becomes one more view:
> it never owns the text, it sends edits to the document and redraws when the document changes. That's why undo, the dirty marker and other editors all stay consistent.

## Measure

- Monaco bundle size and first-editor time (C21).
- Edits dropped in the race test (should be rare); file mismatches (must be zero).

## Pass / fail

- ✅ The race test ends with zero mismatches.
- ✅ ⌘S in a card saves exactly that document, with reason `Manual`.
- ✅ C15, C21, C23 have answers below.

## Findings

*(empty until S5 runs)*

Next: [`../06-core/01-crates-and-domain.md`](../06-core/01-crates-and-domain.md)
