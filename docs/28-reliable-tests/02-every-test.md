# 28.02 — Every window test, by the numbers

Made by a script from `extension/src/test/suite/*.test.ts` at 739a9c7. It counts; it does not read. The judgement is in [01-review.md](01-review.md).
**208 tests: 76 high, 77 some, 55 low.**


- **sleep**: `sleep(` calls in the test's body.
- **latest**: waits or reads that take the newest drawing (`last()`, `.at(-1)`, `stageShows`, `sectionShows`, `barShows`, `listShows`, `stageStep`…). The newest drawing may be an old one, or one made halfway through a change.
- **outside**: steps that change the world outside the window with no "done" (a file written, a `vnoc` or `git` command run).
- **risk**: `high` = a sleep, or `outside` together with `latest`; `some` = `latest` or `outside` alone; `low` = none of them.
- Hooks (`suiteSetup`, `setup`…) are not rows; their sleeps are in the suite's total.

## skeleton — 6 tests, 0 sleeps in the file · high 0, some 0, low 6

| line | test | sleep | latest | outside | risk |
|---|---|---|---|---|---|
| 18 | the extension starts the core, and the core answers |  |  |  | low |
| 24 | the core reads the opened folder through the driver |  |  |  | low |
| 34 | 'V-NOC: Open canvas' opens a webview that draws the structure |  |  |  | low |
| 49 | both commands are registered |  |  |  | low |
| 54 | the facts a report carries: the extension's version, and the core's, which is the version the core answers a p |  |  |  | low |
| 62 | the log a report copies has what the core wrote: the tee sees the channel's lines |  |  |  | low |

## bar — 3 tests, 0 sleeps in the file · high 0, some 2, low 1

| line | test | sleep | latest | outside | risk |
|---|---|---|---|---|---|
| 41 | a file that cannot be read shows the ⚠, and it goes when the file is fixed |  |  | 4 | some |
| 67 | the ⚠'s click target exists: `vnoc.attention.focus` runs without an error |  |  |  | low |
| 80 | a refresh that fails over a drawn canvas keeps the cards and turns the ⚠ red |  | 1 |  | some |

## session — 5 tests, 2 sleeps in the file · high 2, some 0, low 3

| line | test | sleep | latest | outside | risk |
|---|---|---|---|---|---|
| 34 | two Open in Canvas on a node before the canvas is up: the second one is the one that waits for it |  |  |  | low |
| 43 | a reveal that was done is not done again by a canvas opened later | 1 |  |  | high |
| 51 | Open Canvas takes no node: it opens the canvas and reveals nothing |  |  |  | low |
| 60 | Focus focuses without opening the canvas; Open in Canvas opens it and reveals the node without focusing | 1 |  |  | high |
| 74 | a focus is saved with the workspace, for the next start |  |  |  | low |

## sidebar — 16 tests, 6 sleeps in the file · high 3, some 5, low 8

| line | test | sleep | latest | outside | risk |
|---|---|---|---|---|---|
| 74 | every command package.json declares is registered by the extension |  |  |  | low |
| 118 | [check] vnoc.structure.focus brings the webview view up: its page is made and reports what it drew |  | 1 |  | some |
| 126 | the first read of the code has VS Code's progress line on the view; a read after rows are drawn does not (06 § | 1 | 1 |  | high |
| 136 | with the canvas open, a click selects and reveals on the canvas, and opens no code tab | 1 |  |  | high |
| 148 | with the canvas closed, a click opens one preview tab, and the next click replaces it |  |  |  | low |
| 159 | `vnoc.open` is the same click, for the palette and for older callers |  |  |  | low |
| 167 | a selection made elsewhere (a card, a reveal) is reported by the page as the selected row, in view |  |  |  | low |
| 179 | a selection far down a long list is scrolled into view by the page itself |  | 2 |  | some |
| 198 | Reveal in Sidebar selects the node and the page shows it |  |  |  | low |
| 206 | Focus from the menu (a row's context object) focuses the sidebar and the canvas on the node, and Back and Clea |  | 2 |  | some |
| 214 | with an EXPLORE context active, Reveal in Sidebar selects the row in the EXPLORE panel, drawn and in view: the |  | 1 |  | some |
| 244 | the call menu's commands take what a call card's context gives (08 §3, §5) |  |  |  | low |
| 301 | moving through the search rows points at nodes without selecting; clearing the box clears the pointer |  |  |  | low |
| 332 | picking a result the focus hides (*V-NOC: Search…*, then Enter in the box) shortens the focus and selects the  |  | 2 |  | some |
| 360 | a click selects the row; clicking the selected row again changes nothing in the contexts | 3 |  |  | high |
| 378 | no V-NOC page crashed while the other tests ran |  |  |  | low |

## attention — 9 tests, 1 sleeps in the file · high 1, some 5, low 3

| line | test | sleep | latest | outside | risk |
|---|---|---|---|---|---|
| 58 | a documented function that is deleted shows under Missing, and leaves when it is back |  |  | 2 | some |
| 71 | a Missing row opens its docs in the docs tab, pinned to it | 1 |  | 2 | high |
| 84 | a problem on a folder is revealed in the Explorer, not opened as a document |  |  | 2 | some |
| 101 | Remove then Restore gives back the same bytes, and an open doc follows the move |  |  | 4 | some |
| 142 | relinking a Missing function to another puts its docs there, and it leaves Missing |  |  | 2 | some |
| 158 | a problem on a file opens it at the line |  |  |  | low |
| 165 | Remove All Missing moves every Missing node in one go, and Restore brings each back |  |  | 3 | some |
| 217 | a node whose ID line was deleted is found by name, and Write ID makes it found by marker again |  |  |  | low |
| 241 | a deleted ID line shows under ID lost, a click goes to the node, and Write ID on the row puts the line back an |  |  |  | low |

## refactor — 6 tests, 0 sleeps in the file · high 0, some 4, low 2

| line | test | sleep | latest | outside | risk |
|---|---|---|---|---|---|
| 111 | 1. a function renamed in the editor keeps its doc |  |  | 1 | some |
| 125 | 2. a function moved to another file keeps its doc |  |  | 2 | some |
| 148 | 3. a file renamed in the Explorer keeps its doc, and so does the function in it |  |  |  | low |
| 156 | 4. a file renamed with git mv keeps its doc, and so does the function in it |  |  | 1 | some |
| 164 | 5. a folder renamed in the Explorer keeps its doc, and so do the file and function in it |  |  |  | low |
| 173 | 6. a folder renamed with git mv keeps its doc, and so do the file and function in it |  |  | 1 | some |

## calls — 16 tests, 4 sleeps in the file · high 5, some 7, low 4

| line | test | sleep | latest | outside | risk |
|---|---|---|---|---|---|
| 66 | the function cards of a file get their chevrons from probes: main and twice have one, leaf (it calls nothing)  |  | 1 |  | some |
| 80 | opening a card that has a chevron fetches its tree and a card draws it |  |  |  | low |
| 89 | Show Calls on a function opens the canvas on it and opens it: its tree arrives (loading, then idle) and a card |  |  |  | low |
| 102 | closing it before the answer cancels the request: no tree is kept for it | 1 |  |  | high |
| 119 | when the code changes, a tree that is open is fetched again by itself, and the page is not left behind |  |  | 1 | some |
| 128 | a core restart while a function is open: every tree is dropped, and the open one comes back from the new core | 1 |  |  | high |
| 141 | Show Calls on a class with its own __init__ shows the __init__'s calls (R7): the tree is the __init__'s, not t |  |  | 1 | some |
| 153 | a function that is open has its call cards drawn, and a call card is a row of the layout like any other |  | 1 |  | some |
| 159 | selecting a call card opens its EXPLORE context, not activated; double-click (Explore) activates it |  |  |  | low |
| 173 | a double-click on a call card draws a chip, active; the Main chip brings the canvas back; the ✕ closes the con |  | 1 |  | some |
| 202 | a cut call card that is opened gets its callee's tree fetched |  | 2 | 1 | high |
| 224 | what is opened inside a focus stays inside it: clearing the focus gives the project's set as it was left, and  |  | 2 |  | some |
| 240 | a big file opened draws at most 200 cards, and what it held back is hidden, not lost; focused, it is drawn who |  | 5 | 2 | high |
| 267 | the command refuses a file: calls start at a function or a class | 1 |  |  | high |
| 275 | Show Docs from the editor, with a call card selected on the canvas: the docs tab shows the function at the cur |  | 2 |  | some |
| 302 | no V-NOC page crashed |  |  |  | low |

## edit — 33 tests, 21 sleeps in the file · high 12, some 6, low 15

| line | test | sleep | latest | outside | risk |
|---|---|---|---|---|---|
| 93 | a card's code is marked while its file is dirty, and not after a save or a revert | 2 | 4 |  | high |
| 180 | the menu's Edit in Card shows the code, with no draft; typing starts the draft (`editing`, the bar counts it); |  | 2 |  | some |
| 208 | keys typed in a card's code stay in the code: the canvas takes none of them (Space, Backspace, F…) (9.2c) | 1 | 5 |  | high |
| 234 | a file with unsaved changes in a tab: the code is read-only and says Save to edit here, typing starts no draft | 1 | 1 |  | high |
| 274 | E5, at the host: the code answers its baseline (text, version, prefix, eol) and whether the file has unsaved c |  |  |  | low |
| 369 | a draft changed and saved: the file has the new text and not another byte moved, the file is saved, and the ca | 1 | 1 |  | high |
| 400 | a function renamed in the draft: the file has the new name, and the index follows |  |  |  | low |
| 418 | a line added at the top of the file in a tab and saved: Save writes the draft at the function's new place, and |  |  |  | low |
| 440 | the file changed in a tab and saved since the draft started: Save is a conflict, and the file is as the tab le |  | 1 |  | some |
| 466 | an edit elsewhere in a tab that is not saved: Save writes into the buffer and not the disk, the tab stays dirt |  | 1 |  | some |
| 489 | the function is deleted in a tab and saved: Save says gone, and writes nothing |  | 1 |  | some |
| 520 | the core's answer comes after the document moved on: a conflict, and nothing is written (§11 g, §6's re-check) |  |  |  | low |
| 546 | Save twice on the same draft: the second answers saved and writes nothing |  |  |  | low |
| 584 | a draft, the canvas closed: a notification at once; Reopen Canvas brings the draft back, editing, with its tex | 1 | 1 |  | high |
| 616 | a draft, the canvas closed, Save All: the file on disk has the draft, and the copy is empty (9.5c) | 1 |  |  | high |
| 638 | a draft, the canvas closed, Discard: the canvas opens later with no draft (9.5c) | 1 |  |  | high |
| 665 | the notification dismissed: the copy stays; at start the host asks again, with Reopen Canvas and Discard (9.5c | 1 |  |  | high |
| 712 | while the structure card holds a function's draft, its call card is read-only and says so, typing there starts | 3 | 1 |  | high |
| 803 | a draft that deletes the ID line makes the host ask: Cancel writes nothing, Save Anyway writes the draft (§11  |  |  |  | low |
| 824 | a draft that keeps the ID is not asked, and neither is one that changes it to a text that holds it elsewhere |  |  |  | low |
| 841 | a draft that changes the ID asks; and the question is asked only in the row that writes: a conflict is answere |  |  |  | low |
| 860 | the document moves while the question is open: after Save Anyway the rows run again from the top, and the draf |  |  |  | low |
| 926 | Edit on a doc answers its whole text, the version, no prefix and the whole file as the range |  |  |  | low |
| 941 | a doc's text edited and saved: the .md on disk changed byte for byte, the file saved, and no question about an |  |  |  | low |
| 961 | a CRLF doc comes back with CRLF, whatever line endings Monaco's text has, and a second Save of the same draft  |  |  | 2 | some |
| 984 | the doc changed in a tab and saved since the draft started: Save is a conflict, and the file is as the tab lef |  |  |  | low |
| 1003 | an edit in the tab that left the text as it was (typed and undone): the version moved, the text still equals b |  |  |  | low |
| 1024 | a doc with unsaved changes in a tab is refused, and Save and Edit saves it first and answers at once (E5, §8) |  |  |  | low |
| 1042 | the .md was deleted since the draft started: Save is gone, and nothing is written |  |  | 3 | some |
| 1056 | one draft per doc across the pages: while the canvas holds a draft of the doc, the docs tab's Edit is busy; th | 1 |  |  | high |
| 1157 | on the canvas: the pencil in the docs panel shows the source with no draft, typing starts the draft, Save: the | 2 |  |  | high |
| 1192 | in the docs tab: the same, and the tab's own drafts are the `docs` page's in the host's copy |  |  |  | low |
| 1213 | one draft per doc across the pages, and a docs tab closed with a draft asks: the canvas's panel is busy while  | 2 |  |  | high |

## search — 14 tests, 8 sleeps in the file · high 6, some 2, low 6

| line | test | sleep | latest | outside | risk |
|---|---|---|---|---|---|
| 90 | the box shows the core's rows, which the old ranking could not give: `outer.meth` is in no name and no path |  |  |  | low |
| 104 | typing fast, and at a person's pace: every key is asked for, and the last answer drawn is the last text's | 3 | 2 |  | high |
| 124 | the tree changes while the box has text: the rows are asked for again |  | 1 | 2 | high |
| 144 | *V-NOC: Search…* with a starting query: the cursor is in the sidebar's box with that text, its rows are the co |  | 1 |  | some |
| 181 | `kind:method` draws exactly the fixture's methods, in reading order, and no note |  |  |  | low |
| 201 | `kind:fnn` draws no rows, and its note |  |  |  | low |
| 212 | [measure] the box: from the change of the text to the drawn rows, and from the request |  |  |  | low |
| 307 | a word only a doc holds finds the doc: no node row, one doc row, and the row's node is pointed at |  |  |  | low |
| 320 | picking a doc reveals its node and shows that doc in the docs tab, pinned: in a tab the pick opens, and in one | 1 | 2 |  | high |
| 345 | a Missing node's doc is marked, points at nothing, and its pick pins the tab to the Missing node with that doc | 1 | 1 |  | high |
| 362 | a doc whose node is neither in the tree nor Missing: its pick opens the .md in an editor tab, with no docs tab | 1 |  | 5 | high |
| 392 | ↓ walks from the last node row to the first doc row |  |  |  | low |
| 410 | the canvas dims the cards the query does not find, never the selected one, and not a card whose doc is the hit |  | 3 |  | some |
| 470 | typing the callee's name keeps its call card bright; picking it selects that card and the focus stays; a node  |  | 1 | 3 | high |

## cli — 7 tests, 0 sleeps in the file · high 0, some 6, low 1

| line | test | sleep | latest | outside | risk |
|---|---|---|---|---|---|
| 69 | Install makes the link to the binary this window runs, the context key follows, and Uninstall takes back exact |  |  | 1 | some |
| 98 | a foreign file, a foreign link, and another version's link in a window that is not production are refused and  |  |  | 3 | some |
| 134 | Uninstall leaves a link to a program that is only named vnoc, and says what to type |  |  | 2 | some |
| 159 | Set Up Coding Agents with { agents, confirm } writes exactly the files the dry run listed, and the context key |  |  | 3 | some |
| 200 | Set Up Coding Agents writes the permissions when permissions is true (the default) and none when it is false |  |  | 3 | some |
| 242 | a name the binary does not know shows the binary's own error line, and writes nothing |  |  |  | low |
| 251 | the offer is decided once per workspace: the first attach settles it, asked or not |  |  | 3 | some |

## bend — 6 tests, 0 sleeps in the file · high 0, some 2, low 4

| line | test | sleep | latest | outside | risk |
|---|---|---|---|---|---|
| 58 | the core reports both languages, running, each with its extensions, and the extension builds its context keys  |  |  |  | low |
| 72 | the tree shows the .bend files and their defs next to the Python file |  |  |  | low |
| 85 | a file the watcher's glob names is read when it appears, and gone when it is deleted |  |  | 2 | some |
| 93 | Open in Canvas on a Bend def opens the canvas with it selected |  |  |  | low |
| 101 | Write ID on a Bend def puts `# ID: <id>` on the line above it, and the def is found by its marker |  |  | 1 | some |
| 119 | Show Calls on a Bend def gives the def it calls in another file |  |  |  | low |

## tour — 85 tests, 45 sleeps in the file · high 46, some 37, low 2

| line | test | sleep | latest | outside | risk |
|---|---|---|---|---|---|
| 265 | Open Tour… lists the tour, and its command with the tour's id opens the tour's context on the cover |  | 1 |  | some |
| 287 | Next walks to the end card through a block and its details, and Prev walks back |  | 5 |  | some |
| 323 | a stop of the tour: the code pane shows the file's own lines, with their numbers; a file stop shows no code |  | 3 |  | some |
| 341 | the keys → ← Space Home End move the place, and Home and End go to the cover and the end |  | 8 |  | some |
| 366 | the rest of the keys (design 05 §6): Enter starts, ↓ ↑ are the next and the previous stop, Alt+← goes back aft |  | 13 |  | some |
| 418 | a canvas 520 px wide: the stage stacks, nothing is cut, Next is reachable, the mini bar and the cover fit; lon **(skipped on CI)** |  | 5 |  | some |
| 451 | the `▾ N more lit lines` chip has a row of its own under the code, at 520 px and at full width; the cover's he | 2 | 5 |  | high |
| 495 | the selection follows the stop: the node of the stop the place is on |  | 1 |  | some |
| 502 | Go to Code opens the file at the step's first line |  | 1 |  | some |
| 515 | `vnoc tour go` run from a terminal moves the stage, and the place is the one the core keeps |  | 3 | 3 | high |
| 526 | after three clicks on Next, `vnoc tour where` says the step the stage shows |  | 1 | 1 | high |
| 536 | closing the tour gives MAIN back |  | 5 |  | some |
| 573 | it folds and unfolds; unfolded, the list shows the suite's tours newest first, in the core's words |  | 2 |  | some |
| 586 | a click on a row of a tour never opened opens it at the cover, with its route in the section |  | 2 |  | some |
| 597 | leave a tour on stop 3: its row says so, and a click on the row returns to that place |  | 6 |  | some |
| 629 | ‹ Tours shows the list and the tour stays open; its row has the bar and a click on it brings the route back |  | 4 |  | some |
| 644 | a click on a stop of the route moves the stage, and on a step of the stop you are in |  | 6 | 1 | high |
| 662 | “called on line N” jumps to the step of its caller that holds the line |  | 2 |  | some |
| 671 | the keys: ↓ ↑ move the focus, Enter opens or jumps, Escape in the route goes back to the list |  | 3 |  | some |
| 685 | the end card says how many stops were not seen, and its first button goes to the first one skipped: the core's |  | 6 | 1 | high |
| 732 | the map has one card for every stop, in the route's order, with the ring on the current one |  | 1 |  | some |
| 740 | a stop reached by a call is a call card with its line, joined to its caller by a dashed edge; one with no call |  |  |  | low |
| 748 | a click on a card is a jump to the stop: the stage moves, the ring moves, and the edges from the first stop ar |  | 3 |  | some |
| 758 | a call in the code that is a later stop of the tour is a jump to it |  | 4 |  | some |
| 766 | Main ⇄ tour keeps the step and both viewports **(skipped on CI)** | 1 | 7 |  | high |
| 817 | the chip is `▶ title` and the crumbs are the stops from the first to the current one |  | 5 |  | some |
| 829 | ← is back to where you were, Alt+← is the same, and it is disabled with nothing to go back to |  | 3 |  | some |
| 839 | `vnoc tour go` from a terminal says `moved by the agent` for a few seconds, and a click of the reader's does n | 1 | 4 | 1 | high |
| 852 | Start Again goes to the cover |  | 2 |  | some |
| 859 | the menus of a stop: Reveal in Main makes MAIN active on that node and the tour stays where it was; Go to Code |  | 2 |  | some |
| 884 | Esc closes the tour's context and MAIN is as it was: its selection, its focus and its viewport | 1 | 2 |  | high |
| 948 | a list, a table and a Mermaid flowchart are drawn, and the code pane has syntax colours |  | 1 |  | some |
| 957 | a diagram that does not parse shows its source and one line, and the next step is drawn |  | 3 |  | some |
| 1010 | a tour that starts while the canvas is hidden opens nothing: one notification, once, and its Open Tour brings  | 2 | 1 | 2 | high |
| 1029 | the stops of the plan are on the route and the map at once, not written; the bar, the foot, the chip and the s |  | 4 |  | some |
| 1045 | an add puts the stop on the route and the map with no reload, and the stage stays on the reader's step |  | 5 | 1 | high |
| 1067 | reading ahead onto a stop that is not written shows it being written, and its text fades in where the grey bar |  | 5 | 1 | high |
| 1094 | ■ on the bar is the core's halt: the bar, the foot and the stage say stopping until the agent answers, and sto |  | 9 | 2 | high |
| 1131 | a tour with no plan: the written count only, one dotted segment and a `◌ …` row, Next dim on the last written  | 2 | 10 | 5 | high |
| 1177 | a tour with no news for five minutes says so, and the agent writing again takes the words back without a word | 1 | 8 | 2 | high |
| 1208 | a second tour that appears while one is open leaves the open one, the list has it, and one notification says s | 1 | 1 | 1 | high |
| 1222 | W11: with the canvas visible and no tour open, a tour that appears and is being written becomes the tour conte | 2 | 2 | 2 | high |
| 1244 | the canvas hidden and no tour open: a tour that appears unfolds the section, is announced, and nothing opens | 1 | 2 | 1 | high |
| 1321 | a pinned tour whose function was edited shows the step as it was written, with its label; Now shows the code a |  | 8 | 1 | high |
| 1364 | the same edit in a tour that is not pinned: the code as it is, no label, no switch |  | 2 | 1 | high |
| 1380 | a function deleted: gone and struck through; pinned, the step is shown as written with no Now; not pinned, the |  | 5 | 1 | high |
| 1407 | most of a pinned tour is about older code: the cover says so, and the route's foot says how many stops changed |  | 2 | 3 | high |
| 1423 | a core that starts again: the list says changed from the record, and a tour opened first is answered from the  |  | 2 | 2 | high |
| 1496 | On the selected node lists the tours through a function with the core's words; a click opens at that stop, or  |  | 4 |  | some |
| 1530 | a file lists the tours with stops inside it, and says how many |  | 1 |  | some |
| 1540 | a node no tour goes through says so, with Show All; with nothing selected On selection is disabled and the lis | 1 | 7 |  | high |
| 1569 | ▶ N is on the card and the row of a node that is on tours, from the core's count; a click shows its tours; del | 1 | 4 | 5 | high |
| 1618 | a tour card shows +N: its node's calls that are not on the tour, as the core counts them | 1 | 2 |  | high |
| 1709 | a tour of uncommitted work: the cover says not pinned, why, and what it offers; Commit and Pin… lists exactly  | 1 | 4 | 6 | high |
| 1739 | Commit and Pin commits those files and the cover says pinned at …; the check box writes commit = always and ch |  | 4 | 3 | high |
| 1768 | with never the cover offers nothing but still says not pinned, and the menu's Commit and Pin… still works |  | 2 |  | some |
| 1784 | Pin on a tour with files not committed is refused in the core's words, and its message offers Commit and Pin… |  | 1 |  | some |
| 1798 | Pin on a tour that can be pinned pins it, at HEAD |  |  | 4 | some |
| 1813 | Write IDs for Its Stops: a dirty stop file is saved first, after a question; then the IDs are written, and a r | 2 |  | 3 | high |
| 1857 | Write IDs on a clean tour whose stops have no IDs: the file gets both IDs, and NEEDS ATTENTION shows nothing f |  |  | 8 | some |
| 1896 | Write IDs in a project that writes no IDs shows the core's refusal |  |  | 2 | some |
| 1910 | Tour Settings… picks a setting and then a value, and writes it: ids = never |  |  |  | low |
| 1967 | Rename… sends the new title and says the core's words; an empty or unchanged title sends nothing; the core's r |  | 1 | 3 | high |
| 1992 | Share with the Project moves the file and the row says shared ·; Make Private Again moves it back; the end car |  | 4 | 1 | high |
| 2028 | Delete: the row becomes the core's words and Undo for six seconds, Undo brings the tour and its state back to  |  | 3 | 1 | high |
| 2059 | a deleted row goes after six seconds, and the file stays in the trash | 2 | 3 |  | high |
| 2074 | the Delete key deletes the focused row; deleting the open tour closes it |  | 2 |  | some |
| 2095 | a tour being written cannot be deleted: the core refuses in words and the row stays; ■ Stop on its row is the  | 1 | 7 | 3 | high |
| 2127 | the strip counts the tours being written, and the open tour's route header has its own Stop | 2 | 2 | 5 | high |
| 2152 | the open tour's row says it is playing with a Stop Playing that closes the tour and keeps the place |  | 3 |  | some |
| 2174 | the status bar item's click offers Stop Writing and Stop Playing, and each does its own thing | 1 |  | 3 | high |
| 2190 | Tour Settings… and Rename… stay open when the focus goes elsewhere | 2 |  |  | high |
| 2209 | a tour that cannot be read is dim and says so, and can be deleted | 1 | 2 | 3 | high |
| 2225 | Delete Finished Tours takes the private finished ones with one Undo for all, and leaves a shared tour and one  | 1 | 3 | 4 | high |
| 2310 | Open as JSON: change one text, Import into This Tour, and the stage shows it at the same step; a bad edit list |  | 2 | 3 | high |
| 2360 | Export as JSON… writes the file the dialog chose, with the text `vnoc tour export` prints; a dismissed dialog  |  |  | 2 | some |
| 2375 | Import a Tour from JSON… makes a new tour and a new row; a file with a fault has its refusals on the file |  | 1 | 3 | high |
| 2409 | Check All Tours checks every tour and the row says what it found |  | 2 | 1 | high |
| 2422 | Ask for a Tour… is a dialog of the canvas page, filled in from where it was opened; its prompt is the core's,  | 1 | 4 | 5 | high |
| 2514 | Open Tour… in the bar's ⋯ asks which, with the selected node's tours first, and opens the one picked | 1 | 1 |  | high |
| 2546 | Show Docs in a stop's menu opens the docs panel, as the bar's ⓘ does |  | 3 |  | some |
| 2619 | G6: a move to a block of another stop, far below the function's start, scrolls its lit lines into view |  | 5 | 5 | high |
| 2635 | G8: the key Next brings the next lit block into view with the pointer over the pane | 1 | 3 | 1 | high |
| 2650 | G9: the grip goes to 70% of the canvas and no further |  | 4 |  | some |
| 2669 | G10: a nested function's stop is named by its own name, not its qualified one |  | 1 | 1 | high |

## waits — 2 tests, 0 sleeps in the file · high 1, some 1, low 0

| line | test | sleep | latest | outside | risk |
|---|---|---|---|---|---|
| 35 | the core refuses with data.askAgain; the host asks again, so the list and a tour's cover come when the build e |  | 2 | 6 | high |
| 76 | the stage draws the code of a stop in a big file at once, though the count of the tour's calls never ends |  |  | 8 | some |
