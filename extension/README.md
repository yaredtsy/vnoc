# V-NOC

[![Marketplace](https://vsmarketplacebadges.dev/version/vnoc.vnoc.svg)](https://marketplace.visualstudio.com/items?itemName=vnoc.vnoc)
[![Discord](https://img.shields.io/badge/Discord-join-5865F2.svg?logo=discord&logoColor=white)](https://discord.gg/J5nfPHqyBr)
[![Licence: Apache-2.0](https://img.shields.io/badge/licence-Apache--2.0-blue.svg)](https://github.com/yaredtsy/vnoc/blob/main/LICENSE)

Browse Python and Bend code as a tree and a canvas, follow what a function calls, edit it on its card, search it, and attach docs that stay with the code when it is renamed or moved. Coding agents can read the same docs through a command line.

**This is a pre-release.** It works on Python and Bend code, on macOS (Apple silicon and Intel), Linux (x64) and Windows (x64).

![Open a function, see what it calls, follow a call to its own file](https://raw.githubusercontent.com/yaredtsy/vnoc/v0.2.0/extension/media/screenshots/calls.gif)

## How to start

1. Open a Python or Bend project.
2. Open the V-NOC view in the Activity Bar.
3. Click a function in the **Structure** view to see it; double-click to open its code.
4. Right-click a function, class, file or folder and choose **Add Doc…**.
5. Run **V-NOC: Open Canvas** (or the `⊞` button in the view's title) to see the code as a graph.

## What it does

- **Structure** (the side bar): the folders, files, classes and functions of the project, with a search box (`/` or `Ctrl/Cmd+F`), the arrow keys, and type-ahead. A click shows the node on the canvas when it is open, and its code otherwise.
- **Focus** a node (`⌖`, or `Ctrl/Cmd+Enter`) to show only it and what it holds, on the side bar and the canvas together. `Alt+←` goes back one level.
- **The canvas** draws what you open as cards. Open a function (**Show Calls**, or its chevron) to see what it calls, one level at a time, as cards joined by dashed lines.
- **Explore** a call (double-click its card): it opens in its own panel in the side bar, with the function where it lives, and the canvas follows.
- **Edit on the canvas**: type in a function's code on its card and press `Ctrl/Cmd+S`; the function in the file has the new text. A function you changed in the editor meanwhile is never overwritten. Docs are edited the same way, in the docs panel.
- **Search** (`V-NOC: Search…`, `Ctrl/Cmd+Alt+N`): a few letters of a name, a word in a doc, a pasted `file:line` or traceback line, and filters such as `kind:fn no:doc in:billing/`. While the box has text the canvas dims every card that is not a hit.
- **Docs** are attached to a function, class, file or folder. They are found again after a rename or a move: V-NOC writes a short ID into the code (a docstring line) and follows it.
- Docs that lost their code show under **Needs Attention**, where you can relink or remove them, and a function whose `ID:` line was lost can have it written back (**Write ID**).

![Add a doc, rename the function: the doc stays](https://raw.githubusercontent.com/yaredtsy/vnoc/v0.2.0/extension/media/screenshots/docs.gif)

## Coding agents

V-NOC has a command, `vnoc`, that prints a function, the notes attached to it and its code, so a coding agent (Claude Code, Codex, Cursor, Copilot, Gemini CLI and others) can read why code is the way it is before it changes it. It is **experimental**.

- **V-NOC: Install `vnoc` Command in PATH** puts the command on your `PATH`; **Uninstall** takes back only what it made.
- **V-NOC: Set Up Coding Agents** lists the agents, shows the files it will write (a skill, and a few lines in the file each agent reads) and writes them. It also offers to let Claude Code and Codex run V-NOC's *read* commands without asking (ticked by default; a command that writes still asks). It offers itself once, in a project where an agent is in use.

The command line, the skill and the permissions are described in the repository's [section on coding agents](https://github.com/yaredtsy/vnoc#using-v-noc-with-coding-agents). `vnoc guide` prints short how-tos.

## The menus

Right-click anything V-NOC draws (a card, a row of the Structure view, a call card, the canvas, an editor, the Explorer): the entries say what happens, and **V-NOC ▸** in the editor and the Explorer is the short list.

## The `.vnoc/` folder

The docs live in a `.vnoc/` folder at the root of your repository. Commit it with your code: your team then sees the same docs.

## Accessibility

Rows and cards are labelled for screen readers (*"function checkout, selected, 1 doc, cursor here, level 3, 2 of 4"*), the Structure view is a tree you can drive with the keyboard, and with *Reduce motion* on nothing pulses and spinners are still dots. The colours follow your theme, High Contrast included.

## What it does not do yet

- Languages other than Python and Bend (JavaScript/TypeScript and Rust are coming).
- Linux and Windows on ARM.
- Private docs that are not committed.

## Feedback

**V-NOC: Report Issue…** fills in your versions and opens a GitHub form. Questions: [Discord](https://discord.gg/J5nfPHqyBr). The source: [GitHub](https://github.com/yaredtsy/vnoc).

## Licence

Apache-2.0. Third-party code in the binaries: [THIRD_PARTY_NOTICES.md](https://github.com/yaredtsy/vnoc/blob/main/extension/THIRD_PARTY_NOTICES.md).
