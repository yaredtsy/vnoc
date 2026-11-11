# V-NOC

Browse Python code as a tree and a canvas, and attach docs that stay with the code when it is renamed or moved.

**This is a pre-release.** It works on Python code only, on macOS (Apple silicon and Intel), Linux (x64) and Windows (x64).

## How to start

1. Open a Python project.
2. Open the V-NOC view in the Activity Bar.
3. Right-click a function, class, file or folder and choose **Add Doc**.

## What it does

- The tree and the canvas show the folders, files, classes and functions of the project.
- A doc is attached to a function, class, file or folder. It is found again after a rename or a move: V-NOC writes a short ID into the code (a docstring line) and follows it.
- Docs that lost their code show under **Needs Attention**, where you can relink or remove them. A removed doc can be restored.

## The `.vnoc/` folder

The docs live in a `.vnoc/` folder at the root of your repository. Commit it with your code: your team then sees the same docs.

## What 0.1 does not do yet

- Languages other than Python.
- Linux and Windows on ARM.
- Call trees.
- Editing code or docs inside the canvas: "Go to code" opens the editor, and a doc is edited in a normal editor tab.
- Private docs that are not committed.

## Licence

Apache-2.0.
