# Changelog

## 0.2.0

Calls, and a new side bar.

- **Show Calls**: open a function on the canvas to see what it calls, one level at a time, as call cards joined by dashed lines. A chevron
  shows only when there is something to open.
- **Explore** a call (double-click its card): it opens in a panel of its own in the side bar, focused where the function lives, and the canvas
  follows. Each context has a chip on the canvas and a panel in the side bar; **Close** and **Close Others**.
- **Go to Call Site…**: every place a function is called, with a preview.
- **The side bar is new**: a search box, the focus bar, context panels you can resize. Keyboard and screen reader support as before.
- **Cards** show their code (Show Code in Card) and their docs count, and follow your theme (checked in Dark, Light and both High Contrast themes).
- Closing what you opened stops its request; the Python driver keeps the project open, so the next Show Calls is fast.
- **Report Issue…** and **Help and Feedback…**.
- The canvas and docs tabs show the V-NOC logo.
- New **Show Selected Node** in the canvas's **…** menu: it centres the selected card.
- Press **F** on the canvas to **Fit View**, like the **…** menu's entry.
- Fixed: the docs tab kept showing the previous node's docs when you moved to a node that has none.

### Commands and menus

**A key binding of your own on a removed command stops working.**

- **Removed `V-NOC: Show on Canvas`** (`vnoc.showOnCanvas`). It focused a node *and* opened the canvas. Use **Focus** (`vnoc.focus`) and **Open in Canvas** (`vnoc.openInCanvas`).
- **Removed `V-NOC: Open Canvas to the Side`** (`vnoc.openCanvasBeside`). **Open in Canvas** from an editor or a tab now opens the canvas beside it.
- **`V-NOC: Open Canvas` takes no node any more**: it brings the canvas up as it was. On a node, use **Open in Canvas**.
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
