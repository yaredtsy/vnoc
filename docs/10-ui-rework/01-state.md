# 10.01 State: one session in the extension

```
state
├── 1. three kinds of state, three owners
├── 2. the session: shape, intents, reducer
├── 3. intents up, session down
├── 4. the typed message map (shared/messages.ts)
├── 5. inside a webview: small stores, stable arrays
├── 6. what is saved, and where
└── 7. rules
```

## 1. Three kinds of state, three owners

The UI runs in three places that cannot share memory: the extension host (which also draws the native sidebar) and two webviews (the canvas and
the docs tab). Every bug in the README comes from one piece of state having two homes. So each piece gets exactly one.

| Kind | Examples | Owner (the only one who changes it) | Everyone else |
|---|---|---|---|
| **DATA** | the tree, doc lists, Found / Missing | the core (Rust) | holds a copy, replaced as a whole when `indexChanged` arrives |
| **SESSION** | focus stack · selected node · the canvas's open nodes · cursor node · highlight · reveal | **the extension host** | the sidebar reads it directly; a webview holds a read-only copy and asks for changes with an *intent* |
| **LOCAL VIEW** | zoom and pan · which cards show code · doc panel width and open/closed · the docs tab's pin · which rows of the native tree are open | the surface that draws it (VS Code itself for the tree's rows) | nobody else sees it |

The test for "session or local?": **does a second surface need it, or must it survive the surface being closed?** Focus and selection are drawn by
the sidebar and the canvas. The canvas's open nodes are drawn only by the canvas, but menu commands in the extension change them (Expand, Collapse,
Reveal), so they are session too. The zoom level is local.

> **🧩 VS Code note: why the extension host.** A webview only exists while its tab is open: close the canvas and its memory is gone. The extension
> host lives as long as the window does, it is where commands, menus and the native tree run, and it is the only process the webviews can talk to.
> Anything shared has to live there. With the native tree there is a bonus: the sidebar is not a copy at all, it reads the store.

## 2. The session

One object, pure functions around it, in `shared/` so the extension and the tests use the same code.

```ts
// shared/session.ts — no `vscode`, no React. Tested with `node --test`, like focusStack.ts today.

export type ContextId = string;                 // 0.1: always "main"

/** One context: what the sidebar and the canvas show for it. */
export interface ContextState {
  id: ContextId;
  title: string;                                // "Main"; 0.2: "Explore: checkout_order"
  parent: ContextId | null;                     // 0.2: the context it was opened from
  focus: readonly string[];                     // node keys; the last one is the root of what is drawn
  selected: string | null;
  expanded: readonly string[];                  // node keys open ON THE CANVAS (U5: the tree keeps its own)
}

export interface Session {
  contexts: Readonly<Record<ContextId, ContextState>>;
  order: readonly ContextId[];                  // top to bottom
  active: ContextId;                            // the one the canvas shows
  cursor: string | null;                        // the node the text cursor is in; never saved
  highlight: string | null;                     // "look here" without selecting (the search list's active item); never saved
  reveal: { context: ContextId; id: string; n: number } | null;   // a request for the canvas: "bring this into view"; never saved
  seq: number;                                  // grows on every change; a webview drops a session older than the one it shows
}

export type Intent =
  | { type: "select"; id: string | null }
  | { type: "focus"; id: string }               // push (or jump back to it) and select it
  | { type: "focusBack" }
  | { type: "focusJump"; depth: number }        // a crumb or a FOCUS-view row; 0 = clear
  | { type: "toggle"; id: string }              // open / close one card
  | { type: "expand"; ids: string[] }
  | { type: "collapseAll" }
  | { type: "reveal"; id: string; canvas: boolean }   // see below
  | { type: "revealed"; n: number }             // the canvas did it
  | { type: "cursor"; id: string | null }
  | { type: "highlight"; id: string | null }
  | { type: "treeChanged" };

/** Returns THE SAME object when nothing changed, so the caller can skip the save and the broadcast. */
export function reduce(s: Session, context: ContextId, intent: Intent, tree: TreeNode): Session;
```

What each intent does:

| Intent | Rule |
|---|---|
| `select` | sets `selected` and clears `highlight`. **Never** touches `focus`, never opens anything. Opening code is a command's job ([`02-sidebar.md`](02-sidebar.md) §4), not the reducer's. |
| `focus` | `push(focus, id)` (exists: the same id on top does nothing; an id lower in the stack jumps back to it), then selects it (which clears `highlight`). |
| `highlight` | sets `highlight`. Changes nothing else, ever: not the selection, the focus or `expanded`. How it looks and when it is set: [`06-how-it-looks.md`](06-how-it-looks.md) §6. |
| `focusBack` / `focusJump` | `pop` / `jump`. Then: keep `selected` if it is still under the new top, else select the new top. |
| `toggle` / `expand` / `collapseAll` | change `expanded`. The focused node is always drawn open, whatever `expanded` says. |
| `reveal` | (1) step the focus back to the newest entry that contains `id`, or clear it; (2) add every ancestor of `id` to `expanded`; (3) select it; (4) if `canvas`, set `reveal = { context, id, n: n + 1 }`. "Reveal in Sidebar" passes `canvas: false`, so it leaves nothing waiting for the canvas. This is today's `webview/src/canvas/reveal.ts`, moved here. |
| `revealed` | the canvas centred the node: `reveal = null` if `n` matches. A canvas that opens later finds nothing to replay. |
| `cursor` | sets `cursor`. Changes nothing else, ever. |
| `treeChanged` | for **every** context: `prune` the focus; clear `selected` if its node is gone; drop `expanded` keys that are gone. Clear `cursor`, `highlight` and `reveal` if their node is gone. |

Two more rules for `reduce`:

- An intent for a context id that does not exist returns the same object (a late message from a closed context in 0.2).
- Arrays that did not change are **kept**, not copied, so `reduce(s, …).contexts.main.expanded === s.contexts.main.expanded` when only the
  selection moved. The webview relies on this (§5).

`shared/focusStack.ts` stays as it is, with its tests. `reduce` calls it.

### Which node the docs tab shows

The docs tab beside an editor follows the code, not the canvas. Today this is `docsTarget` and `docsPinned` in `extension.ts` and `docTarget` in
`nodeAt.ts`. The rule, written once:

1. Pinned (the tab's pin button, or the tab was opened by "Show Docs" on a specific row or card): that node, until unpinned.
2. Else the cursor node, widened by `docTarget` (the innermost node around the cursor that has docs).
3. `cursor` is updated only while a **code editor** is the active editor. Clicking into the docs tab or the canvas does not clear it, so the docs tab
   keeps showing what it showed. (In the first version it fell back to the selection, and jumped as soon as you clicked into it.)

`docTarget` and `chainAt` move to `shared/tree.ts`.

## 3. Intents up, session down

```
canvas card click ─────► post { intent: select } ───┐
canvas double-click ───► post { intent: focus } ────┤
tree row click ────────► command vnoc.open ─────────┼──► store.dispatch(context, intent)
menu "Focus" (any) ────► command vnoc.focus ────────┤         │ reduce(...)  → same object? stop.
editor cursor moves ───► (debounced) cursor ────────┤         │ save if the saved part changed · set context keys
core: indexChanged ────► treeChanged ───────────────┘         ▼
                                     ├─► the native tree redraws what changed and reveals the selection
                                     └─► post { event: "session", session } to every open webview
```

```ts
// extension/src/session.ts
export class SessionStore {
  private state: Session;                                  // restored from workspaceState at activation (§6)
  private readonly emitter = new vscode.EventEmitter<{ prev: Session; next: Session }>();
  readonly onChange = this.emitter.event;

  constructor(private readonly memento: vscode.Memento, private readonly tree: () => TreeNode | undefined) { … }

  get(): Session { return this.state; }

  dispatch(intent: Intent, context: ContextId = this.state.active): void {
    const tree = this.tree();
    if (!tree) return;                                     // nothing can be checked before the first structure arrives
    const prev = this.state;
    const next = reduce(prev, context, intent, tree);
    if (next === prev) return;
    this.state = next;
    // cursor, highlight and reveal are not saved: a cursor move must not write to disk
    if (next.contexts !== prev.contexts || next.order !== prev.order || next.active !== prev.active) {
      void this.memento.update(SAVED_KEY, toSaved(next));
    }
    this.emitter.fire({ prev, next });
  }
}
```

The `prev` in the event lets each listener do only what changed: the tree refreshes the old and the new selected rows, not the whole tree, and
reveals the new one; the context keys are set only when their value moves.

A webview never changes its copy. It draws the last session it was sent and posts intents. When it starts, it **asks** for the session
(`request("session")`); after that it only listens.

What this deletes from today's code:

| Goes away | Because |
|---|---|
| `FocusState` (`extension/src/focus.ts`), `setFocus(next, from)` and the `"host" \| "canvas"` argument | one `dispatch`; nobody needs to know who asked |
| `focusLoaded`, the `focusChanged` message, the second `prune` in `App.tsx` | the canvas has no copy to keep in step |
| `pendingReveal` and the `pending` request | a canvas that starts asks for the session and finds `reveal` in it. Two quick "Show on canvas" commands: the second one wins, none is lost. |
| `docsTarget`, `docsPinned` as module variables | §2: `cursor` is in the session; the pin is the docs tab's local state |
| `selected`, `expanded`, `revealId`, `centerOn` `useState`s in `App.tsx` | they are in the session |

> **🧩 VS Code note: is a round trip fast enough for a click?** A click on a card now goes webview → extension host → back before the ring moves.
> Both hops are `postMessage` inside one machine. **Measured in step R3** (a real VS Code window, the canvas visible, 30 selections that alternate
> between four cards, `performance.now()` at `dispatch` and a `MutationObserver` on the card): the `selected` class is on the card after a
> **median of 2.8 ms (p90 4.4 ms)**, and the frame that shows it is drawn after a median of 12.5 ms (the next display frame, which any approach
> waits for). Two runs agreed; a single outlier of 15 ms in 60 samples. A tree of four cards on a warm window: a large tree has more cards to
> re-render, but only the two whose `selected` changed do (step R5). Far below the ~30 ms where the plan would draw the ring locally first, so
> nothing is drawn locally: that would be a second writer, which is the bug this rework removes.

> **🧩 VS Code note: the session is sent whole.** Not "what changed", the whole object, every time: a focus stack, one id and a few hundred open
> keys is a few KB. Whole snapshots cannot get out of step; diffs can. `seq` protects against the one thing that can go wrong: two snapshots
> arriving in the wrong order.

## 4. The typed message map

Everything that crosses between the extension and a webview is declared once, in a file both sides import. A message that is not in this file
does not exist.

```ts
// shared/messages.ts
import type { ContextId, Intent, Session } from "./session.ts";

/** A node as the UI sees it. ONE definition: today it is written by hand in structureView.ts, types.ts and (as Placed) layout.ts. */
export interface TreeNode {
  id: string;                          // the location key (path, or path::qualname#n). Changes on rename.
  kind: Kind;
  name: string;
  path: string;
  qualname: string | null;
  range: Range | null;
  syntax_error: boolean;
  node_id: string | null;              // the permanent 26-character ID, once a doc is attached
  docs: number;
  link: "marker" | "name" | null;
  children: TreeNode[];
}

/** webview → extension, with an answer. One line per request: its parameters and its result. */
export interface Requests {
  structure: { params: void;                          result: StructureResult };
  session:   { params: void;                          result: Session };
  docs:      { params: { nodeId: string };            result: DocInfo[] };   // the permanent ID, not the location key
  docText:   { params: { path: string };              result: string };
  code:      { params: { path: string; range: Range }; result: string };
  addDoc:    { params: { key: string };               result: { added: boolean } };
  openDoc:   { params: { path: string };              result: void };
  openLink:  { params: { href: string; from: string }; result: void };
  goToCode:  { params: { key: string };               result: void };
}

/** extension → webview. No answer. */
export interface Events {
  session:      Session;
  indexChanged: { revision: number };
  docText:      { path: string; text: string };
  /** a menu entry that changes LOCAL view state of the canvas (show code in a card, fit the view) */
  viewCommand:  { name: "showCode" | "hideCode" | "fitView"; key?: string };
}

/** webview → extension, no answer. */
export type Notes =
  | { type: "intent"; context: ContextId; intent: Intent }
  | { type: "rendered"; visible: number; hidden: number }      // the test suite uses it
  | { type: "pin"; pinned: boolean }                           // the docs tab
  | { type: "close" };
```

The core's `vnoc/docs` takes `{ id }` (`crates/vnoc/src/protocol.rs:164`); the relay renames it. The webview side calls it `nodeId` because every
other `id` and `key` in this map is the location key.

> **🧩 TypeScript note: let the compiler check the wiring.**
> ```ts
> // webview/src/host.ts
> export function request<M extends keyof Requests>(method: M, ...params: Requests[M]["params"] extends void ? [] : [Requests[M]["params"]]): Promise<Requests[M]["result"]>;
>
> const docs = await request("docs", { nodeId });    // DocInfo[]
> await request("dcos", { nodeId });                 // error: "dcos" is not a key of Requests
> ```
> ```ts
> // extension/src/relay.ts — the switch in extension.ts:342 becomes a table
> const handlers: { [M in keyof Requests]: (p: Requests[M]["params"]) => Promise<Requests[M]["result"]> } = {
>   structure: () => loadStructure(),
>   session:   async () => store.get(),
>   docs:      (p) => core.request("vnoc/docs", { id: p.nodeId }),
>   // … leave one out and TypeScript says: Property 'goToCode' is missing
> };
> ```
> `[M in keyof Requests]` reads "for every name in `Requests`". `M extends keyof Requests` on a function reads "M is one of those names, and the
> rest of the signature depends on which".

The Rust side still declares `TreeNode` once more (`crates/vnoc/src/index/tree.rs`). Generating the TypeScript from Rust (`ts-rs`) stays "later", as
in `07-ui/01` §3.

## 5. Inside a webview: small stores, stable arrays

Add `zustand` (about 1 KB; the web version already uses it). One store per kind of state from §1.

```
webview/src/state/
├── data.ts       useData      status · revision · root · stats        refetched on indexChanged; never saved
├── session.ts    useSession   the last Session received               replaced as a whole; NO setters, only dispatch()
└── view.ts       useView      canvas: codeOpen · docsOpen · docWidth  owned here; saved with vscode.setState in ONE subscriber
                               canvas: hover · revealed                 owned here; NOT saved (06-how-it-looks.md §1)
                               docs tab: pinned
```

```ts
// webview/src/state/session.ts
export const useSession = create<Session>(() => EMPTY);

request("session").then(accept);
on("session", accept);

function accept(incoming: Session) {
  const current = useSession.getState();
  if (incoming.seq < current.seq) return;                                  // a late snapshot
  useSession.setState(keepEqual(current, incoming), true);                 // `true` = replace, do not merge
}

/** The ONLY way a component changes the session. */
export const dispatch = (intent: Intent, context = useSession.getState().active) => post({ type: "intent", context, intent });
```

**Why `keepEqual`.** A message is JSON, so every session that arrives has brand-new arrays, even when only the cursor moved. The canvas computes its
layout with `useMemo` from `focus` and `expanded`, and a new layout puts every dragged card back in its row (`Canvas.tsx:45`). Without this, moving
the text cursor in the editor would undo the user's dragging. `keepEqual` walks the contexts and reuses the current `focus` and `expanded` arrays
when their contents are equal:

```ts
// shared/session.ts, pure and tested
export function keepEqual(old: Session, next: Session): Session {
  const contexts = Object.fromEntries(Object.entries(next.contexts).map(([id, c]) => {
    const o = old.contexts[id];
    if (!o) return [id, c];
    return [id, { ...c, focus: sameList(o.focus, c.focus) ? o.focus : c.focus, expanded: sameList(o.expanded, c.expanded) ? o.expanded : c.expanded }];
  }));
  return { ...next, contexts };
}
```

**Selectors return a primitive, or use `useShallow`.** A card subscribes to exactly what it draws, so a click re-renders two cards, not all:

```tsx
const selected = useSession((s) => s.contexts[s.active].selected === node.id);   // boolean
const isCursor = useSession((s) => s.cursor === node.id);                         // boolean
const crumbs   = useSession(useShallow((s) => s.contexts[s.active].focus));        // the array, compared item by item
```

The canvas turns `expanded` into a `Set` once per change, in one `useMemo` beside the layout, rather than calling `includes` in every card.

**Measured in step R5** (a real window, 9 cards drawn, 6 selections): a click draws **2 cards**, the one that lost the selection and the one that got it.
It took two things beyond the selectors. React Flow re-draws every card when any prop of its renderer changes, so the click handlers are module-level
functions (an inline arrow is new on every render), and `NodeCard` is `memo`. Before those, a click drew all 9 cards and 2 of them twice (11 draws).
React Flow's own `selected` is not used (`elementsSelectable={false}`): the ring is the session's, and a second source of "selected" cost a second draw.

`CanvasActions` (a React context that only carries callbacks down) is deleted. So are the eight callback props on `<Canvas>`.

## 6. What is saved, and where

| State | Saved with | Survives |
|---|---|---|
| session: `contexts` (focus, selected, expanded), `order`, `active` | `context.workspaceState` in the extension, key `vnoc.session.v1` | closing the canvas, reloading the window, restarting VS Code |
| session: `cursor`, `highlight`, `reveal`, `seq` | not saved | — |
| local view of a webview | `vscode.setState()` in that webview | hiding and showing it, a window reload |
| which rows of the native tree are open | VS Code, by the item `id` | a refresh; a reload **[check]** |
| user preferences (`vnoc.sidebar.click`) | VS Code settings | synced by VS Code |
| what the team shares | `.vnoc/` | the repo |

**Restoring.** Read `vnoc.session.v1` and check its shape; anything that does not parse is dropped and the session starts empty. The old key
`vnoc.focus` (today's `FocusState`: a plain array) is read once as `contexts.main.focus` and then deleted. A later change of shape gets a new key
(`v2`) and a small converter from `v1`.

Never put session or view state in `.vnoc/`: it is per person and it would show up in `git status`.

**Keys after a rename.** The session stores node keys (`path::qualname#n`), which change when a function is renamed or moved. `treeChanged` drops
what no longer exists: one renamed node is closed again, or one focus entry is gone. That is one click. Do not build a key translation table for it
(same conclusion as `07-ui/01` §5).

## 7. Rules

1. The core is the only truth for data. The extension host is the only truth for the session. A surface is the only truth for its own local view.
2. A webview changes the session in one way: `dispatch(intent)`. It never sets a field of its copy.
3. A command (menu, key, palette, tree click) changes the session in the same way: `store.dispatch(intent)`. A command may also open an editor or a
   panel; the reducer never does.
4. Every surface has four drawn states: **loading, empty, ready, error**.
5. A message that is not in `shared/messages.ts` does not exist.
6. `retainContextWhenHidden` is a convenience. Each webview must come back correctly from `request("session")` + `request("structure")` +
   `vscode.getState()` alone. Test it with *Developer: Reload Window*.

Next: [`02-sidebar.md`](02-sidebar.md)
