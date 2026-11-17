# 22.07 On screen

```
on screen
├── 1. where a playground shows       the panel's second tab · the card's footer · the menus
├── 2. the Playgrounds tab
├── 3. New Playground                 three native questions
├── 4. the recipe form (⚙)
├── 5. Edit Script · Remove
├── 6. Run, Stop, the output
├── 7. every state
├── 8. commands, menus, keys
└── 9. what the pages hold
```

Nothing here decides anything: every list, default and sentence comes from the core (06). The drawings are the intent, not pixels; the
panel's look follows `16-ui-redesign/` and the kit already in `webview/src/components`.

## 1. Where a playground shows

| Place | What | Today's code |
|---|---|---|
| **the doc panel** (beside the canvas, and the docs tab) | a second tab: **Docs** · **Playgrounds** | `webview/src/docs/DocPanel.tsx` |
| **a card's footer** | a chip `▶ 2` next to the docs chip; a click opens the panel on the Playgrounds tab | `node-card/footerChips.ts`: a new `kind: "plays"` |
| **a sidebar row** | the same count where the row shows its doc count | `16-ui-redesign/07-sidebar-rows.md` |
| **the node menu** | *Show Playgrounds* · *New Playground…* | `16-ui-redesign/08-menus.md` §2, a new group |
| **Needs Attention** | a Missing node's line says "2 docs, 1 playground" | `attentionView.ts` |

The panel keeps one rule it already has: it shows the **selected node's** things and follows the active context (`shared/docSubject.ts`). The
tab is per panel, remembered in the view state; a node with playgrounds and no docs opens on Playgrounds.

## 2. The Playgrounds tab

```
┌ parse_order ────────────────────────────────────────────── ✕ ┐
│ src/orders.py · OrderService                                 │
│ ┌──────────┬────────────────┐                                │
│ │ Docs  2  │ Playgrounds 2  │                  ＋ New        │
│ └──────────┴────────────────┘                                │
│                                                              │
│ ▾ Parse a bad order                               python     │
│   in .   $ .venv/bin/python vnoc_play_parse_a_bad_order.py   │
│   [▶ Run]  [Edit Script]  [⚙]  [⋯]                           │
│   ┌──────────────────────────────── exit 1 · 0.8 s ─ ⧉ ──┐   │
│   │ Traceback (most recent call last):                    │   │
│   │   File "vnoc_play_parse_a_bad_order.py", line 4, in … │   │
│   │ ValueError: qty: 'x' is not a number                  │   │
│   └───────────────────────────────────────────────────────┘   │
│                                                              │
│ ▸ Two currencies                                  python     │
└──────────────────────────────────────────────────────────────┘
```

- One row per playground, open or closed; the open one shows the plan line, the buttons and the last output. Opening is view state.
- **The plan line is always there** when the row is open (05 §3): `in <dir>   $ <the command's words>`. When the plan failed, the sentence
  is there in its place, in the warning colour, and Run is disabled.
- `⋯`: *Rename…* · *Copy Command* · *Reveal Folder* (the playground's folder in the Explorer) · *Remove Playground*.
- `⧉` copies the output. The output box is plain monospace text, grows to a limit and then scrolls, and follows the end while it runs.
- A node with none: "No playgrounds on `parse_order` yet." and **New Playground**. In a project with no preset for anything (never: `custom`
  exists) the button is still there.

## 3. New Playground

Three native questions, in the style of *Add Doc* (`extension/src/addDoc.ts`); Esc at any of them creates nothing.

```
1. Title                 an input box            "Parse a bad order"
2. Language              a QuickPick             Python (proposed: the node's language) · TypeScript · JavaScript · Rust · Custom…
                                                 + any language the project's vnoc.toml defines          ← vnoc/playPresets
3. Run in                a QuickPick             . (project root)      recommended                        ← vnoc/playSuggest
                                                 src/                   the node's folder
                                                 Choose a folder…       (the native folder dialog, kept inside the project)
```

- *Custom…* asks two more: the file name (`{name}.sh`) and the command (`sh {file}`).
- If the project has no `.vnoc/` yet, the first-attach question of `addDoc.ts` comes first, unchanged (06 §6).
- Then: `vnoc/playNew`, the panel opens on the new row, and the script opens in an editor tab beside the canvas with the starter text.
- The command is **not** asked. The preset's is right most of the time, and it is one click away (⚙).

## 4. The recipe form (⚙)

Inside the row, in the panel:

```
Title        [ Parse a bad order                         ]
Run in       [ .                                    ▾ ]     the folders of vnoc/playSuggest, and Choose…
File name    [ {name}.py                              ]     default shown as the placeholder text when unset
Command      [ {python} {file}                        ]     ditto
Timeout      [ 60 ] s
             reaches pub items only                         ← the preset's note, when it has one
             ☐ Use this command as the project's default for Python     → writes vnoc.toml [play.language.python] run
[Save]  [Reset to defaults]                                  Committed with the project. No passwords or keys.
```

- A field left as the default is **not written** to `play.toml` (02 §2): the form sends `null` for it.
- Save → `vnoc/playUpdate`; the answer is the new `PlayInfo`, whose plan line replaces the old one. A refusal is the sentence under the
  field it names.
- An absolute path typed into Command gets the one-time warning of 02 §6.
- Language is not in the form: changing a playground's language is a new playground.

## 5. Edit Script · Remove

**Edit Script** opens the script file in a normal editor tab (`vscode.window.showTextDocument`, beside the canvas, as *Go to Code* does).
It is the user's editor: their keys, their formatter, undo, the usual dirty dot. What language help it has is in 04 §5.

**Remove Playground** asks ("Remove the playground *Parse a bad order*? Its folder goes to the trash."), then the extension asks the core
that the path is a playground of this node (`is_play`), and deletes the folder with `workspace.fs.delete(…, { recursive: true, useTrash:
true })`. It is refused while that playground runs.

## 6. Run, Stop, the output

1. **Run** saves the script's tab first if it is open and dirty. Only that file: it is V-NOC's, and the user pressed Run on it. (VS Code's
   own *Run Python File* does the same.)
2. The extension sends `vnoc/playRun`. The button becomes **■ Stop**; the header of the output box says `running… 2 s`.
3. `vnoc/playOutput` chunks are appended; `err` text is drawn in the error colour only when the run ends with a non-zero code (many tools
   write progress to stderr; colouring it red while it runs cries wolf).
4. `vnoc/playEnded` sets the header: `exit 0 · 0.8 s`.

A second Run of the same playground while one runs is not offered (the button is Stop). Other playgrounds run beside it.

The canvas and the tree do not move during a run (G15). If the script changed the user's code on disk (it can), the watcher reports that as
any change.

## 7. Every state

| State | The row shows | Run |
|---|---|---|
| never run (in this core's life) | the plan line; no output box | ▶ |
| running | `running… 12 s`, output growing | ■ Stop |
| ended, code 0 | `exit 0 · 0.8 s` | ▶ |
| ended, another code | `exit 1 · 0.8 s`, in the error colour | ▶ |
| stopped | `stopped · 3.1 s` | ▶ |
| timed out | `timed out after 60 s` and a link *Change timeout* (opens ⚙) | ▶ |
| output cut | under the box: "the output was cut at 1 MB" | |
| could not start | "`python3` was not found. Set it in `.vnoc/local/play.toml` ([tools] python), or change the command (⚙)." with *Open play.toml* (creates the file with the `[tools]` table commented if it is not there) | ▶ |
| the name is taken | "`vnoc_play_parse_a_bad_order.py` already exists in `.` and V-NOC did not create it. Rename the file, or change File name (⚙)." | ▶ |
| already running elsewhere | "Running in another window or from the command line." | disabled until `indexChanged` or a retry |
| the recipe is refused | the sentence in place of the plan line; *Open play.toml* | disabled |
| Restricted Mode | "Trust this folder to run playgrounds." *Manage Workspace Trust* | disabled |
| the node is Missing | the playgrounds are listed under the Missing node in Needs Attention; Run is offered (the script may still import fine) | ▶ |

## 8. Commands, menus, keys

| Command id | Title | Where |
|---|---|---|
| `vnoc.newPlayground` | V-NOC: New Playground… | the node menu `6_play@2`; the panel's ＋; the palette, on the selected node, with the rule `vnoc.addDoc` has when nothing is selected **[check]** PG4 |
| `vnoc.showPlaygrounds` | Show Playgrounds | the node menu `6_play@1`; the footer chip |
| `vnoc.runPlayground` | V-NOC: Run Playground… | the palette: a QuickPick of every playground of the project (title, node, language) |
| `vnoc.stopPlaygrounds` | V-NOC: Stop All Playgrounds | the palette |

The node menu's groups become `1_nav · 2_open · 3_calls · 4_code · 5_docs · 6_play · 9_copy`; the menu test of `16-ui-redesign/08` §9
(`menus.test.ts`) gets the two rows. No key bindings first: Run is a button on a row that is already focused.

## 9. What the pages hold

| State | Lives in | Why |
|---|---|---|
| which playgrounds a node has, their plans | the core; asked with `vnoc/plays` on select and on `indexChanged` | as docs |
| a run's output and end | the core (`vnoc/playRuns`), mirrored in the extension's session for the pages | the panel can be closed and reopened mid-run; the docs tab and the canvas's panel show the same run |
| which row is open, which tab | the webview's view state (`state/view.ts` `Saved`) | how *you* look at it (place D, `04-storage` §1) |
| the form's unsaved fields | the component | a form nobody saved is not worth keeping |

The session gets one new slice (`plays: { runs }`), fed by the two notifications; `shared/messages.ts` gets the request and note types;
`relay.ts` forwards `plays`, `playPresets`, `playSuggest`, `playUpdate`, `playRun`, `playStop`, `playRuns` to the core unchanged.

Next: [`08-agents.md`](08-agents.md)
