# 16. UI redesign: one bar, calls as nodes, a webview sidebar with contexts

Written 2026-11-12 on `0.2-call-trees` at `6041f56`, after the call tree started working (step 7.8). Read for it: the extension (`extension/src/`),
the canvas (`webview/src/`), `shared/session.ts`, [`../10-ui-rework/`](../10-ui-rework/), [`../14-call-trees.md`](../14-call-trees.md) §7, and the web
version (`ide/v-noc/src/frontend/src/features/Dashboard/`: `Sidebar/` with `TabContextStack` and `ContextPanel`, `store/useTabStore.ts`,
`store/slices/focusSlice.ts`, `hooks/useNodeHandlers.ts`, `Main/hooks/useWorkspaceActions.ts`, `Canvas/hooks/useEnhancedTreeLayout.tsx`).

**This folder wins over `10-ui-rework/` and over `14-call-trees.md` §7 where they differ.** Each file says what it replaces.

```
ui redesign
├── README (this file)       what is wrong · the decisions · what is replaced
├── 01-layout.md             the surfaces, the jobs a user comes with, the six verbs, where each thing lives
├── 02-top-bar.md            the canvas's one bar (was three)
├── 03-canvas.md             what is drawn, contexts on one canvas, expand one level at a time, keys, the four states
├── 04-node-cards.md         the structure card and the call card: anatomy, buttons, sizes
├── 05-calls-as-nodes.md     call ids, fetching, the "has calls?" chevron, cut / recursive / unknown, stale
├── 06-sidebar.md            the webview sidebar: search, context panels, the focus bar, keyboard, Needs Attention
├── 07-sidebar-rows.md       one row per kind: what it shows, hover buttons, click / double-click
├── 08-menus.md              every menu (card, row, canvas pane, editor, editor tab, Explorer, view titles), one use case per entry
├── 09-states.md             selected · highlighted · cursor · focused · active context · loading · stale, on every surface
└── 10-build-order.md        the steps, each with files, proof, and commit message
```

## 1. What is wrong today (the first screenshot)

| Problem | Where you see it | Cause |
|---|---|---|
| **three bars on the canvas** | `V-NOC minicart: 2 folders · …  Refresh ⓘ` · `← All code › app › api.py` · `ƒ checkout() app/api.py:11  Show Focus Code ✕` | `App.tsx` stacks the header, the crumbs and `SelectionStrip`. The strip repeats what the card's own buttons do; the stats are read once and then cost a line forever. |
| **the focus is drawn three times in the sidebar** | the FOCUS view (`All code / app / api.py ●`), then STRUCTURE's `Focus: app › api.py` row, then the crumbs on the canvas | 10-ui-rework U10 put a FOCUS view *and* a focus bar row on purpose. In use it reads as a duplicate. |
| **calls are a list inside one card** | the `checkout()` card is 520 px tall: code, then a "Calls" list with every level open, `• 1 site` on each row, `line 16` chips | 14 §7 drew the whole merged tree as indented rows in the entry's card. Nothing on the canvas is a call node, there is no edge, and nothing opens one level at a time. |
| **a call cannot be followed** | clicking `reserve_stock` in the list selects it, but nothing shows where it lives or what calls it | there is no EXPLORE context yet (10-ui-rework/03 §5 waited for call trees) |
| **Back means "the last thing I focused"**, not "one level up" | focus `checkout`, then `reserve_stock` from the editor: Back goes to `checkout`, never to `inventory.py` | the stack is a history (10/03 §2). You asked for the opposite: from a called node, go up to its parent, then its parent's parent. |
| **the card is small text and many chips** | `⌖ focus`, `+N`, `●`, `○`, link icon, docs button, `</>`, footer path, `+` | each feature added a chip to the header |
| **menus offer the same thing three ways** | Focus · Show on Canvas · Open Canvas · Reveal in Sidebar · Open Canvas to the Side, in one editor submenu | each was added for a case; the names do not say the difference |

## 2. The decisions

Proposed here. The ones marked **ask** change behaviour you already use; say yes or no before step 8.1. The others follow from what you asked.

| # | Decision | Why | Where |
|---|---|---|---|
| **R1** | The sidebar becomes a **webview view** (you suggested it). Needs Attention stays a native view under it. The native STRUCTURE and FOCUS views go. | Context panels, a search box, one menu list and shared open/closed state are all impossible or awkward in a native tree (10-ui-rework/02 §11 listed them). | [06](06-sidebar.md) |
| **R2** **ask** | **Inside one context the focus is a path**, root → node. Focus on a node = its lineage (`app › inventory.py › reserve_stock`). **Back = up one level.** A crumb = jump up to it. | This is the "pop to the parent, then the parent's parent" you asked for. A history inside a context is not needed any more: a jump across the code (following a call) opens its own context (R3), so the context you came from is still there. Replaces 10/03 §2 "the stack is a history, not a path". | [03](03-canvas.md) §3 |
| **R3** | **Following a call opens an EXPLORE context**, as the web does with tabs: its focus is the callee's lineage, its panel is stacked under its parent in the sidebar, and **one canvas shows the active context**. | One place per question: MAIN keeps what you were reading; EXPLORE shows where the callee lives, and Back walks up from there. | [03](03-canvas.md) §4 · [06](06-sidebar.md) §4 |
| **R4** **ask** | **An EXPLORE panel belongs to the selected call** (the web's rule, `handleNodeSelection`): selecting a call opens its panel **without activating it**; selecting anything else in the parent closes it. Double-click / Explore opens it **and** activates it. | No pile of panels; the panel is "the selected call, at home". The other choice is panels that stay until closed, which needs a close-all and a limit. | [03](03-canvas.md) §4 |
| **R5** | **Calls are nodes.** On the canvas a function's calls are cards after its nested defs, joined by **dashed** edges. **Not in the sidebar** (C1, §5). The "Calls" section in the card goes. | You asked for it; the web draws them so. | [04](04-node-cards.md) · [05](05-calls-as-nodes.md) |
| **R6** (decided 2026-11-13) | **Exact chevrons from a probe.** Each function card drawn is probed with a `depth: 1` call tree (`vnoc/hasCalls`): it shows a chevron only if opening shows at least one card, and its `+N`. The probe is never drawn and its `Cut` is never shown. Opening fetches **the whole merged tree once** and shows **one level per click**. Until the driver keeps the project open (8.2a), the chevron stays optimistic, with the same UI. | A probe on a kept project costs ~6 ms (Python, warm, debug); without one, 9.7 s for httpx's 1,134 functions (14 §11). A chevron that disappears on opening was the cost of the optimistic way. | [05](05-calls-as-nodes.md) §3.1 |
| **R7** | Only **functions** get calls. A class's own calls are under its `__init__` row; a class with no `__init__` of its own (none, or inherited) owns them itself, after its methods (05 §1, 2026-11-13). A *call* to a class (`Store()`) is a call node of kind class, whose children are what `__init__` calls (as the driver gives it). | A class row with both its methods and `__init__`'s calls would show the same calls twice. | [05](05-calls-as-nodes.md) §1 |
| **R8** **ask** | **Open/closed is shared** by the sidebar and the canvas, per context (the web's `expandedNodeIds`). | One state; "the canvas is the sidebar drawn as a graph". Possible now that the sidebar is a webview (U5 was only because of the native tree). | [03](03-canvas.md) §2 |
| **R9** | **One bar** on the canvas: Back · context chips · crumbs · reveal selection · docs · `⋯`. The stats line and the selection strip go. | | [02](02-top-bar.md) |
| **R10** | **Focus never opens anything.** It changes what the sidebar and the canvas show. Opening the canvas is its own entry, "Open in Canvas". | Answers "when I click Focus, should the canvas open or the code?": neither; the canvas follows if it is open. One verb, one effect. | [01](01-layout.md) §4 |
| **R11** | The editor's V-NOC submenu has **five entries** (Reveal in Sidebar · Open in Canvas · Focus · Show Calls · Show Docs / Add Doc…), down from nine. From an editor the canvas always opens **beside** it, so "Open Canvas to the Side" goes. | | [08](08-menus.md) §4 |
| **R12** | **One menu list** for a card and a sidebar row: both are webviews, so one `webview/context` list serves both. The drift test shrinks to two exceptions. **[check]** that `webview/context` applies to a webview *view* (8.6 checks it first; the fallback is in 08 §7). | | [08](08-menus.md) §1 |

## 3. What is replaced

| Was | Status |
|---|---|
| 10-ui-rework U1 (native tree), U5 (open/closed not shared), U6 (two menu lists), U10 (FOCUS view + focus bar row) | **replaced** by R1, R8, R12, and the panel's focus bar ([06](06-sidebar.md) §3) |
| 10/02 §4 the click rule (canvas open → reveal there; closed → preview the code) | **kept** ([07](07-sidebar-rows.md) §4) |
| 10/03 §1 the words (focus, selection, highlight, cursor, reveal) | **kept**, plus **explore** and **active context** ([01](01-layout.md) §3) |
| 10/03 §2 "the stack is a history", "focusing a node lower in the stack jumps back to it" | **replaced** by R2 |
| 10/03 §5 contexts, "built in 0.2" | **built here** (R3, R4), with the web's rules |
| 10/04 menus | **replaced** by [08](08-menus.md) |
| 10/06 how each state looks | **kept** for the card's selected / highlighted / cursor / revealed looks; [09](09-states.md) adds the rest and the sidebar's |
| 14 §7 the "Calls" section, the progress notification, "stale never re-runs by itself" | **replaced** by R5, R6 and [05](05-calls-as-nodes.md) §5–6 |
| `shared/session.ts`, `shared/focusStack.ts` | **changed** in step 8.1: focus as a path, contexts, call ids |

Nothing in the Rust core or the driver changes. `vnoc/callTree` is enough as it is (it returns the whole merged tree; the UI shows it one level at a time).

## 4. The idea in one picture

```
   VS CODE WINDOW
   ┌ activity bar ┐┌ side bar: V-NOC ────────────┐┌ editor group 1 ────────┐┌ editor group 2 ────────────────────────────────┐
   │              ││ STRUCTURE (webview)    ⊞ ⟳ ⋯ ││ api.py                 ││ V-NOC canvas                                    │
   │   ◉ V-NOC    ││ 🔍 Search nodes…              ││                        ││ ← [Main][◇ reserve_stock ✕] minicart›app›inv… ⌖ ⓘ ⋯│
   │              ││ ┌ MAIN ────────────────────┐ ││ def checkout(...):     ││                                                 │
   │              ││ │⌖ app › api.py       ← ✕  │ ││   completed = check…   ││  [📄 inventory.py]──[ƒ reserve_stock]┄┄[ƒ check_…]│
   │              ││ │▾ 📄 api.py               │ ││                        ││                                                 │
   │              ││ │    ƒ checkout            │ ││                        ││                                                 │
   │              ││ │    ƒ health              │ ││                        ││                                                 │
   │              ││ ├ ◇ EXPLORE: reserve_stock ✕┤ ││                        ││                                     ┌ docs ────┐│
   │              ││ │⌖ app › inventory.py › …  │ ││                        ││                                     │ ...      ││
   │              ││ │  ƒ reserve_stock         │ ││                        ││                                     └──────────┘│
   │              ││ └──────────────────────────┘ ││                        ││                                                 │
   │              ││ NEEDS ATTENTION (2)  native   ││                        ││                                                 │
   └──────────────┘└──────────────────────────────┘└────────────────────────┘└─────────────────────────────────────────────────┘
                                  │ intents ▲                    ▲ session, call trees (down)                  │ intents
                                  ▼         │                    │                                             ▼
                                ┌─────────────────────────────────────────────────────────────────────────────────┐
                                │ extension host: SESSION (contexts · focus paths · selected · expanded · active)   │
                                │                 CALL TREES (by entry key, fetched on first expand)                │
                                └─────────────────────────────────────────────────────────────────────────────────┘
```

## 5. Changed after your review of the mockup (2026-11-12)

| # | You said | Decision | Where |
|---|---|---|---|
| **C1** | showing calls in the sidebar may not be needed | **No call rows in the sidebar.** It shows where code lives; the canvas shows what it calls. Every sidebar chevron is then exact (R6's optimistic chevron is canvas-only), and a callee is not repeated as rows. Selecting a call card still opens its EXPLORE panel, which shows the callee at home. Show Calls on a function row opens the canvas. | [07](07-sidebar-rows.md) §2 |
| **C2** | Go to Call Site may not work when sibling calls are merged | It works: the merge keeps **every** site with its file (`calls.rs` `merge_group`). The fix is in the UI: **one card per callee per caller** (no duplicates), **each site its own link** on the card, `N more ▾` lists every site with its line's text, and no single "Go to Call Site" button on the action strip (it could not know which site). The menu and `Ctrl/Cmd+Enter` go straight there with one site, or show a QuickPick that previews each site. | [04](04-node-cards.md) §2.1 |

**The mockup (`mockup.html`) still shows both old things**: call rows under functions in the sidebar (`flattenPanel` / `rowHtml` with
`role === "call"`), and the `site` button in the call's action strip (`keys = ["explore", "site", "code", "card"]`), which goes to `sites[0]`.
Change it to match before you show it to anyone.

Next: [`01-layout.md`](01-layout.md)
