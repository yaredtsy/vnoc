# 07.01 UI state: who owns what, and how it flows

```
ui state
├── 1. three programs, and what each one may hold
├── 2. the flow: one direction for data, one for actions
├── 3. one typed message map (shared/messages.ts)
├── 4. inside the webview: data slice + view slice
├── 5. what is saved, and where
└── 6. rules to keep it from rotting
```

## 1. Three programs

The UI is spread over three processes that cannot share memory. Most UI bugs in an extension like this come from two of them each believing they hold the
truth.

| Program | Runs in | Owns (is the truth for) | Holds only a copy of |
|---|---|---|---|
| **Core** (Rust) | its own process | the index: tree, IDs, docs, Found / Missing | — |
| **Extension** (TypeScript) | VS Code's extension host | which panels are open · VS Code documents · commands and menus | — (it passes data through) |
| **Webview** (React) | a sandboxed browser frame | **view state**: what is collapsed, focused, selected, scrolled | a snapshot of the tree and docs |

> **🧩 VS Code note: why the webview is so isolated.** A webview is an `<iframe>` with no access to files, to the VS Code API, or to your core process.
> It can do exactly one thing: `postMessage` to the extension, and receive messages back. So every piece of data on the canvas made the trip
> core → extension → webview as JSON. `report/02-vscode-extension-basics.md` has the longer explanation.

## 2. The flow

```
                     DATA flows down                               ACTIONS flow up
core      index changes ──► vnoc/indexChanged {revision}        ◄── vnoc/attach, vnoc/relink, …
            │                                                          ▲
extension   ├─► sidebar.refresh()                                      │  commands, context menus
            └─► webview.postMessage({event: "indexChanged"})           │
                  │                                                    │
webview     fetch the new snapshot ──► draw                     user clicks ──► request(...)
```

Two rules make this predictable:

1. **The webview never changes its copy of the data.** After "Attach doc" it does not add a doc to its local list. It sends the action, the core changes the
   index, `indexChanged` arrives, the webview fetches again. One path for every change, including changes made by a teammate's `git pull`.
2. **Every snapshot carries a `revision` number.** If an answer arrives with a revision lower than the one already shown (two fetches crossed), drop it.

Is fetching the whole tree again wasteful? For a few thousand nodes it is a few hundred KB of JSON and a few milliseconds. Do the simple thing in 0.1.
When it is measurably slow, send only what changed. That is an optimisation you can add without touching rule 1.

## 3. One typed message map

Today (review E2, E4): message names are strings in an `if / else if` chain in `extension.ts`, payloads are `any`, and the `TreeNode` shape is typed twice
by hand.

Put every message in **one file** that both the extension and the webview import:

```ts
// shared/messages.ts

/** A node as the UI sees it. `key` is the location (changes on rename); `nodeId` exists once something is attached. */
export interface TreeNode {
  key: string;                       // today's `id`: renamed, because it is NOT the permanent ID
  nodeId: string | null;             // the 26-character ID, if the node has one
  kind: "folder" | "file" | "class" | "function";
  name: string;
  path: string;
  range: Range | null;
  docs: number;                      // how many docs are attached
  link: "marker" | "name" | null;    // solid icon, outlined icon, or none
  syntaxError: boolean;
  children: TreeNode[];
}

/** webview → extension. One line per request: its parameters and its answer. */
export interface Requests {
  "tree":         { params: void;                              result: { revision: number; root: TreeNode; missing: MissingNode[] } };
  "docs":         { params: { nodeId: string };                result: DocInfo[] };
  "attach":       { params: { key: string; title: string };    result: { nodeId: string } };
  "openDoc":      { params: { nodeId: string; file: string };  result: void };
  "code":         { params: { path: string; range: Range };    result: string };
  "reveal":       { params: { key: string };                   result: void };       // go to code
  "pendingFocus": { params: void;                              result: string | null };
}

/** extension → webview. No answer expected. */
export interface Events {
  "indexChanged": { revision: number };
  "focus":        { key: string };
}
```

> **🧩 TypeScript note: let the compiler check the wiring.** With the map above, both sides can be written so that a typo or a missing handler is a
> compile error:
> ```ts
> // webview/src/host.ts — the method name decides the parameter type and the result type
> export function request<M extends keyof Requests>(method: M, params: Requests[M]["params"]): Promise<Requests[M]["result"]> { … }
>
> const docs = await request("docs", { nodeId });     // docs is DocInfo[]; request("dcos", …) does not compile
> ```
> ```ts
> // extension/src/relay.ts — an object with exactly one handler per request
> const handlers: { [M in keyof Requests]: (p: Requests[M]["params"]) => Promise<Requests[M]["result"]> } = {
>   tree:    ()  => core.request("vnoc/tree"),
>   docs:    (p) => core.request("vnoc/docs", p),
>   code:    (p) => readSlice(p),
>   // … forget one and TypeScript reports: Property 'attach' is missing
> };
> ```
> `[M in keyof Requests]` reads "for every name in `Requests`". The `if` chain becomes a table, and adding a feature is: one line in `Requests`, one
> handler, one call.

The Rust side still declares the same shapes a third time (`protocol.rs`). Generate TypeScript from Rust (`ts-rs`) once the shapes stop changing weekly;
until then, the tests that send real messages through all three programs (you have them) catch a mismatch.

## 4. Inside the webview: two slices

`App.tsx` today holds `load`, `reloads`, `collapsed`, `codeOpen`, `focus` and six effects. 0.1 adds docs, a selected node, a doc list, and Missing.
Split the state by **where it comes from**, because that decides how it behaves:

```
webview state
├── DATA  (a copy; replaced as a whole; never saved)
│   ├── status      loading | ready | error
│   ├── revision
│   ├── root        the tree
│   ├── missing     nodes that could not be found
│   └── docs        nodeId → DocInfo[]      (fetched when a card opens its doc list; cleared on indexChanged)
│
└── VIEW  (owned here; changed by clicks; saved)
    ├── collapsed   set of keys
    ├── focus       stack of keys
    ├── codeOpen    set of keys
    ├── selected    key | null
    └── docsOpen    key | null              (which card shows its doc list)
```

**Recommendation: `zustand`, one small store per slice.** It is about 1 KB, has no boilerplate, and components subscribe to exactly the fields they use,
so toggling one card does not re-render the whole canvas.

```ts
// webview/src/state/view.ts
import { create } from "zustand";
import { vscode } from "../host.ts";

type View = {
  collapsed: Set<string>; focus: string[]; codeOpen: Set<string>; selected: string | null;
  toggle(key: string): void; focusOn(key: string): void; /* … */
};

export const useView = create<View>((set) => ({
  ...restore(),                                             // from vscode.getState()
  toggle: (key) => set((s) => ({ collapsed: flip(s.collapsed, key) })),
  focusOn: (key) => set((s) => ({ focus: push(s.focus, key) })),
}));

useView.subscribe((s) => vscode.setState(serialise(s)));    // ONE place that saves, instead of an effect in App
```

```tsx
// a card reads only what it needs; `toggle` no longer travels through three components as a prop
const collapsed = useView((s) => s.collapsed.has(node.key));
const toggle    = useView((s) => s.toggle);
```

Your pure helpers (`focusStack.ts`, `layout.ts`) stay as they are, with their tests; the store just calls them. `CanvasActions` (a React context that
exists only to pass `toggle` down) can go.

**Layout is derived, not stored:** `layout(root, collapsed, codeOpen)` is computed with `useMemo` from the two slices, as today.

## 5. What is saved, and where

| State | Saved with | Lifetime | Example |
|---|---|---|---|
| view state of one canvas | `vscode.setState()` in the webview | that panel, across hide / restart | collapsed, focus stack |
| small things the extension needs | `context.workspaceState` | this workspace on this machine | "the welcome hint was dismissed" |
| user preferences | `contributes.configuration` (VS Code settings) | the user's settings, synced by VS Code | call-tree depth (0.2) |
| project decisions the team shares | `.vnoc/vnoc.toml` | the repo | scope, `[ids] default` |

Never put view state in `.vnoc/`: it is per person, and it would show up in `git status`.

**Keys in saved view state:** use `nodeId` when the node has one (it survives renames), else `key`. After a refresh, anything that no longer exists is
dropped; `prune` already does this. Do not build the plan's `key_moves` translation table: losing "this function was collapsed" after a rename costs the
user one click.

## 6. Rules to keep it from rotting

1. The core is the only truth for data. The webview is the only truth for view state. The extension holds neither; it connects them.
2. One way in for data (fetch after `indexChanged`), one way out for actions (`request`).
3. Every surface has four explicit states: **loading, empty, ready, error**. Decide what each looks like when you build the surface, not after a bug report.
4. A message that is not in `shared/messages.ts` does not exist.
5. `retainContextWhenHidden: true` is a convenience, not a storage layer. The canvas must still come back correctly from `getState()` alone. Test it by
   reloading the window (*Developer: Reload Window*), which destroys every webview.

Next: [`02-design.md`](02-design.md)
