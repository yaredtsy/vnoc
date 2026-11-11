# V-NOC

Browse Python code as a tree and a canvas, and attach docs that stay with the code when it is renamed or moved.

**This is a pre-release. It runs on macOS (Apple silicon) only for now, and on Python code only.**

- Open a Python project, then open the V-NOC view in the Activity Bar.
- Right-click a function, class, file or folder and choose **Add Doc** to attach a doc to it.
- The doc is found again after a rename or a move: V-NOC writes a short ID into the code (a docstring line) and follows it.
- Docs that lost their code show under **Needs Attention**, where you can relink or remove them.
- The docs live in a `.vnoc/` folder at the root of your repository. Commit it with your code.
