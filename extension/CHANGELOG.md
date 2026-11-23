# Changelog

## 0.3.1

Bend, as a second language, and fixes to the canvas.

### Bend

- **V-NOC reads Bend (`.bend`) projects as well as Python**: the tree, the canvas, IDs and docs, and call trees.
- A project can hold **several languages**. Each language is its own driver program, found beside `vnoc`, and `vnoc status` lists each driver.

### Fixes

- A call card finds its function in the whole project, not only in the code you are looking at.
- A pick from the search ends on the picked card, every time.
- A reveal moves the camera only when it has to.
- Show Docs from the editor shows the function at the cursor, also when a call card is selected on the canvas.
- The right-click menus are there as soon as a folder with Python or Bend files is opened; the V-NOC view does not have to be opened first.
- Markdown tables in the docs panel have borders and padding.
- A file that is not UTF-8 is read now (the bad bytes show as �), instead of failing.
- The Marketplace badge on the README page is fixed.

## 0.3.0

Editing on the canvas, search, and a command line for people and coding agents.

### Edit on the canvas

- **Edit in Card**: type in a function's or class's code on its card, press **⌘S / Ctrl+S**, and the function in the file has the new text. Typing starts a
  *draft* in the card; nothing leaves the card until you save. The tree, the doc badges and the call trees follow the save, as they do after a save in a normal editor.
- Save **checks before it writes**: a function you changed in the editor in the meantime is never overwritten (the card tells you, and you choose). It finds
  a function that moved, puts the indentation back, and asks before it removes a function's `ID:` line.
- A file with unsaved changes cannot be edited from a card, and the card says so first, with a **Save** button.
- Drafts survive hiding the canvas and reloading the window, and closing the canvas with drafts asks right after, so nothing is lost. The bar counts the
  drafts; **Save All** and **Discard** act on them.
- **Edit a doc in the docs panel and the docs tab**, by the same rule: a draft, saved with ⌘S.
- **Write ID** in Needs Attention: a function whose `ID:` line was lost can have it written back from the tree.

### Search

- The search box finds a node by **a few letters of its name** (`chor` finds `checkout_order`), its **qualified name** (`OrderService.par`), or a **pasted
  reference**: `src/orders.py:142`, `file.py#L12`, a traceback line, an ID.
- **Filters** in the query: `kind:fn`, `has:doc`, `no:doc`, `in:billing/`, and `-` in front of one to leave it out.
- **Docs are found by the words in them** (`retry`, `"retry policy"`), with a snippet around the match.
- While the box has text, the **canvas dims every card that is not a hit**. **V-NOC: Search…** opens the box.

### The command line (experimental)

**New, and experimental until 1.0:** the command names and the `--json` output are a new public surface, and can still change; the release notes will say when.

- **`vnoc` is a command as well as the editor's core.** `vnoc status`, `show`, `code`, `tree`, `find`, `grep`, `calls`, `missing`, `check`, `tags`, `doc
  list | read | new | tag`, `tag`, `relink`, `repair`, `init`, `guide`, `skill`, `agent`. Text for people (one fact per line, colour on a terminal), `--json` for
  programs, `--quiet`, and the exit codes `0` / `1` (`check` found problems) / `2` (the request cannot be met) / `3` (the environment). `vnoc guide` and `vnoc <command> --help` say the rest.
- **Search from the shell**: `vnoc find` and `vnoc tree --where` use the same query language as the box.
- **Notes from the shell**: `vnoc doc new` attaches a note to a node, `vnoc doc tag` tags it, `vnoc tags` lists the tags and `vnoc tag describe` writes what one means.
  `vnoc relink` and `vnoc repair` fix a Missing node and a lost `ID:` line.
- **Coding agents**: `vnoc agent install` writes the V-NOC **skill** and a short **rules block** where the agents the user names look for them (Claude Code, Codex,
  Cursor, Copilot, Gemini CLI and others: `vnoc agent list`). In a terminal it is a menu; it never overwrites a file of yours, and `vnoc agent remove` takes back
  only what it wrote. It also offers to let Claude Code and Codex run V-NOC's *read* commands without asking (the question is yes by default; a write still asks).
- **Two VS Code commands**: **Install `vnoc` Command in PATH** (a link in `~/.local/bin`, as VS Code does for `code`) and **Set Up Coding Agents**, which lists the
  agents, shows the files it will write, and writes them. The extension offers the second once, in a project where an agent is in use. **Uninstall `vnoc` Command from PATH**
  takes back only what the first made.
- **The command in every release**: each release now has `vnoc-cli-<system>.tar.gz` (`.zip` on Windows) holding `vnoc` and the Python driver, and an
  `install.sh` that installs the one for your system after checking its sum: `curl -fsSL https://github.com/yaredtsy/vnoc/releases/download/v0.3.0/install.sh | sh`.
  One `SHA256SUMS` covers every file of the release, and the archives and the script carry build provenance like the VSIX files.
- Text from your files (a note, a code line, a file name) is **shown, not obeyed** on a terminal: control characters such as `ESC` print as `^[`. A pipe and
  `--json` get the bytes as they are.
- **`vnoc doc read` starts with the node's header** (the same line `show` prints), so a note is never read without knowing what it is attached to. A filter typed as a
  flag (`vnoc find retry --kind fn`) is still a usage error, now with a line that says where filters go: `filters go in the query: vnoc find 'retry kind:fn'`.
- **Tested with agents**: Claude Code (Sonnet) and Codex (`gpt-reserve`), six scenarios on a sample project, run from a fresh session each
  (`docs/21-agent-cli/06-skill.md` §8).
- **What was run**: the macOS Apple-silicon files (the extension and the `vnoc` archive) were run before the release. The files for the other three systems are built
  and packaged by CI, and have not been run.

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
- A new EXPLORE context opens at 100 % at most; it used to open zoomed in to 200 %.
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
