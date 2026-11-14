# Changelog

## Unreleased (0.2)

Menus (docs/16-ui-redesign/08-menus.md). **Commands that are gone: a key binding of your own on one of them stops working.**

- **Removed `V-NOC: Show on Canvas`** (`vnoc.showOnCanvas`). It focused a node *and* opened the canvas. Use **Focus** (`vnoc.focus`) and **Open in Canvas** (`vnoc.openInCanvas`).
- **Removed `V-NOC: Open Canvas to the Side`** (`vnoc.openCanvasBeside`). **Open in Canvas** from an editor or a tab now opens the canvas beside it.
- **`V-NOC: Open Canvas` takes no node any more**: it brings the canvas up as it was. On a node, use **Open in Canvas**.
- **Removed `Show Call Tree`** (`vnoc.showCallTree`, never released) in favour of **Show Calls** (`vnoc.showCalls`).
- **Removed `Drop This Focus Step`** (`vnoc.focusDrop`) and **`Focus Selection`** (`vnoc.focusSelection`): the sidebar handles `Ctrl/Cmd+Enter` and `Alt+←` itself.
- New: **Open in Canvas**, **Explore**, **Go to Call Site…**, **Close** and **Close Others** (the sidebar's panels and the canvas's contexts).
- The right-click menu of a card and of a sidebar row is one list; a call card has its own menu; the editor's and the Explorer's **V-NOC** submenus are shorter.

## 0.1.0

The docs loop, as a pre-release.

- The tree and the canvas of a Python project: folders, files, classes and functions.
- Add Doc on a function, class, file or folder. The doc is found again after a rename or a move, in the editor, in the Explorer and with `git mv`.
- Needs Attention: docs whose code is gone can be relinked, removed and restored.
- macOS (Apple silicon and Intel), Linux x64 and Windows x64.

## 0.0.1

The first pre-release, for macOS on Apple silicon only.
