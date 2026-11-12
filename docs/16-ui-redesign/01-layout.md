# 16.01 Layout: the surfaces, the jobs, the verbs

```
layout
├── 1. the surfaces and what each one is for
├── 2. the jobs a user comes with
├── 3. the words (selection, focus, explore, active context, …)
├── 4. the six verbs, and what each one opens
├── 5. the web's tabs, and what they become in VS Code
├── 6. the default arrangement, and who opens what where
└── 7. what is not in this redesign
```

## 1. The surfaces

| Surface | Kind | Is for | Not for |
|---|---|---|---|
| **V-NOC side bar → STRUCTURE** | webview view (R1) | finding a node, the open/closed tree, the focus bar, the context panels. Always there while you work. | reading code or docs |
| **V-NOC side bar → NEEDS ATTENTION** | native tree (unchanged, `11-missing-relink-remove.md` §6) | missing docs and problems; shown only when there are some | |
| **canvas** | webview panel, **one** per window | seeing a part of the code as a graph: structure and calls, one level at a time; the docs panel on its right | browsing the whole project (the sidebar does that) |
| **code editor** | VS Code's | reading and writing code | |
| **docs tab** | webview panel beside the editor (unchanged) | reading the docs of the code under the cursor | |
| **editor / Explorer menus** | VS Code's, a V-NOC submenu | getting from the code to V-NOC | |

The status bar, the panel area (Problems, Output) and the palette are not V-NOC surfaces, except the commands in the palette.

## 2. The jobs a user comes with

Every entry of every menu and every button in this folder serves one of these. If a control serves none, it goes.

| # | Job | Example | Where it is done |
|---|---|---|---|
| **J1** | **Get the lay of the land** | "what is in `app/`?" | sidebar tree; canvas without a focus |
| **J2** | **Narrow down, and back out** | "only `api.py`", then "back to `app`" | Focus; Back / crumbs |
| **J3** | **See what a function calls, step by step** | "what does `checkout` call? and what does `checkout_order` call?" | on the canvas: expand a function, then each call, one level per click (the sidebar has no call rows, 07 §2) |
| **J4** | **Follow a call to where it lives, and climb from there** | "`reserve_stock`: what file is it in, what is next to it?" | Explore a call; Back walks up its parents |
| **J5** | **Read the code** | the body of `charge_payment`, the line where it is called | Go to Code; Go to Call Site; code in card |
| **J6** | **Read and write docs** | the docs of `checkout`; a new doc on `app/` | docs panel (ⓘ), docs tab, Add Doc… |
| **J7** | **From the code to V-NOC** | "I am in `api.py` at `checkout`: show it in V-NOC" | the editor's and Explorer's V-NOC submenu |
| **J8** | **Fix what broke** | a doc lost its function after a rename | NEEDS ATTENTION |

## 3. The words

Five are from 10-ui-rework/03 §1; **explore** and **active context** are new. How each looks: [`09-states.md`](09-states.md).

| Word | Meaning | Kept where | Changes what is drawn? |
|---|---|---|---|
| **selection** | "the node I am working with": the docs panel, the menus and the keys act on it. One per context, or none. **A call node can be selected** (05 §1). | `contexts[c].selected` | no; ring on the card, highlight on the row |
| **focus** | "show only this and what is under it", on both surfaces. **A path** from the root down to the node (R2). | `contexts[c].focus` | yes |
| **open** (expanded) | a card / row whose children are drawn. Shared by both surfaces (R8). | `contexts[c].expanded` | yes |
| **explore** | open the callee of a call in a **context of its own**, focused on its lineage | a new entry in `contexts` | a new panel; the canvas switches if it is activated |
| **active context** | the context the canvas draws and the docs panel follows | `session.active` | the canvas |
| **highlight** | "look here" without selecting (search list, hovered crumb) | `session.highlight` | no; dotted outline |
| **cursor node** | the node the text cursor is in | `session.cursor` | no; a mark |
| **reveal** | one-time: open what hides a node, bring its card into view | `session.reveal` | may open ancestors, may shorten the focus |

The rules from 10/03 §1 stay: focus selects; selection never focuses; the cursor never selects; selecting never opens anything by itself.

## 4. The six verbs

The question "when I click Focus, should the canvas open, or the code?" has one answer per verb. **Each verb does one thing, everywhere.**

| Verb | What it does | Opens the canvas? | Opens code? | Gesture |
|---|---|---|---|---|
| **Select** | sets the selection in the context it was done in | no | no | click a card or a row |
| **Focus** | the context's focus becomes the node's lineage; both surfaces show only that | **no** (R10). If the canvas is open it follows. | no | `⌖` · double-click a **card** · `Ctrl/Cmd+Enter` |
| **Explore** (a call) | opens / replaces the call's EXPLORE context, **and activates it** | no; if open, it switches to that context | no | double-click a **call** card · `◇` · menu · `Enter` on a call card |
| **Open in Canvas** | reveal the node on the canvas, opening the canvas if it is closed (beside the editor when run from one) | **yes** | no | menu · the sidebar's `⊞` title button (no node: just open) |
| **Go to Code** | the definition in an editor tab; beside the canvas if the canvas is in front | no | **yes** | `↗` · double-click a **row** · menu |
| **Show Docs** | the docs panel on the canvas if the canvas is visible, else the docs tab beside the editor | no | no | `ⓘ` · docs count chip · menu |

Two shortcuts combine verbs, because the combination is the job:

| Shortcut | = | Job |
|---|---|---|
| **a sidebar click** | Select, then: canvas open → reveal on it; canvas closed → Go to Code as a **preview** tab (kept from 10/02 §4, setting `vnoc.sidebar.click`) | J1, J5 |
| **Show Calls** (editor menu, function rows/cards) | Open in Canvas, then expand the function's calls one level | J3 from the code |

Why a row's double-click is Go to Code and a card's is Focus: each follows its host. In VS Code's Explorer a double-click opens the file for good (a pinned tab); on a canvas a double-click zooms in. The `⌖` button is on both.

## 5. The web's tabs, in VS Code

The web has two kinds of tab. They become different things here.

| Web | What it is there | Here | Why |
|---|---|---|---|
| **Code · Docs · Canvas · Tasks** (the workspace tabs) | what the main area shows for the selected node | **VS Code's editor tabs**: code tabs, the canvas tab, the docs tab | VS Code already has tabs, split groups, and keyboard for them. A second tab bar inside the canvas would fight them. |
| **MAIN · EXPLORE: x** (`useTabStore`, stacked panels in the sidebar) | a context: its own focus, selection, open nodes | **contexts**: stacked panels in the sidebar, chips in the canvas's bar | the same idea, the same rules |
| switching a tab changes the sidebar and the main area | | **the sidebar never swaps**: it shows every context's panel at once. Activating a context changes **only** the canvas (and the docs panel). | A sidebar that rebuilds itself on every switch is what you called "too much" for the extension. The stacked panels already show everything. |

There is **one canvas tab**, drawing `session.active`. Each context keeps its own pan and zoom inside the page (not saved), so switching back lands where you were. One canvas tab per context was the other option (10/03 §5); it is rejected: two canvases with the same cards side by side, two docs panels, and a tab bar full of "V-NOC canvas".

## 6. The default arrangement

```
┌ side bar ──────────┐┌ group 1: code ─────────┐┌ group 2: canvas ─────────────┐
│ V-NOC: STRUCTURE   ││ api.py                 ││ V-NOC canvas                 │
│ NEEDS ATTENTION    ││                        ││                     [docs ⓘ]│
└────────────────────┘└────────────────────────┘└──────────────────────────────┘
```

| Who opens | What | Where |
|---|---|---|
| sidebar `⊞` Open Canvas · the palette | the canvas | beside the active code editor if there is one, else the active group |
| editor / Explorer menu → Open in Canvas, Show Calls | the canvas | **beside** the editor, always (R11) |
| Go to Code, Go to Call Site, a sidebar click with the canvas closed | a code tab | if the canvas is in front: the group beside it (created once, reused); else the active group |
| Show Docs with the canvas hidden | the docs tab | beside the editor (unchanged) |

With code on the left and the canvas on the right, nothing ever covers anything: a sidebar click moves the canvas, and moves the code too when a code editor is visible in the other group (10/02 §4, kept).

## 7. Not in this redesign

- Editing on the canvas (0.3), tasks, prompts, groups, "Add Call" by hand (web only for now).
- A docstring summary on the card: it needs a `summary` field in `vnoc/structure` (core and driver). Listed as a later step in [`10-build-order.md`](10-build-order.md) §4.
- Callers ("who calls this").
- The docs tab and the doc panel's insides: unchanged, except that the panel follows the active context.

Next: [`02-top-bar.md`](02-top-bar.md)
