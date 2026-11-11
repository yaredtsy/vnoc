# 12. Fix: the tree and the session chase each other

Written 2026-11-11, after the selection was seen jumping between nodes by itself (in focus mode, on a slow computer). It is a build plan for one
fix, outside the Step 5 numbering. The state rules it builds on are in [`10-ui-rework/01-state.md`](10-ui-rework/01-state.md). Where this file
is more exact, it wins.

```
the fix
├── 1. what goes wrong
├── 2. the rule
├── 3. the module: treeMirror
├── 4. wiring it in
├── 5. tests
├── 6. order of work
└── 7. what is not part of this
```

## 1. What goes wrong

The core and the reducer (`shared/session.ts`) are sound. The fault is in `extension/src/extension.ts`, where the native tree is bound to the
session both ways:

| Direction | Code today |
|---|---|
| session → tree | `revealSelection`: `sidebarView.reveal(row, { select: true })` when `selected` changes |
| tree → session | `sidebarView.onDidChangeSelection` → `store.dispatch({ type: "select" })` |

VS Code fires `onDidChangeSelection` for our own `reveal`, exactly as for a click, and `reveal` answers later. The only guard is that `reduce`
returns the same session for an equal selection. That holds for one selection at a time. With two close together:

```
select A      session = A     reveal(A) sent
select B      session = B     reveal(B) sent        (before the tree answered A)
echo of A     session is B, so A looks like a click → session = A → reveal(A) sent
echo of B     session is A, so B looks like a click → session = B → reveal(B) sent
…             for ever
```

**Reproduced** in the window suite with the test of §5: after a burst of selections the session ended on the wrong node. A focus change redraws
the whole tree, which slows the reveals, so focus mode and a slow computer make it easy to hit. Quick clicks or held arrow keys on the canvas do
it too.

## 2. The rule

> A surface is a mirror. What the session writes to a surface is never read back as the user's input.

The webviews already keep it: they post intents only from clicks and keys. The native tree is the one surface that breaks it. An echo cannot be
told from a click by its **value**, so it is told by its **cause**:

- one `reveal` at a time; a selection that arrives meanwhile waits, and only the newest one waits;
- every selection event that arrives while a reveal is running is the tree answering us, and is dropped;
- an event with no reveal running is the user.

A click on a row also runs the row's command (`vnoc.open`), which dispatches `select` by itself (`01-state.md` §3). So a click that lands while
a reveal is running is not lost.

**[check]** VS Code delivers the selection event for a `reveal` before the `reveal` promise resolves. The design does not break if it is the
other way round (a late echo of the newest selection is an equal selection, which `reduce` ignores; a late echo of an older one arrives while the
next reveal runs, and is dropped), but verify it with the window test and write the answer here.

## 3. The module: `extension/src/treeMirror.ts`

New file. **No `vscode` import**, so `node --test` can run it. A function, not a class with constructor parameter properties
(`--experimental-strip-types` cannot run those).

```ts
export interface TreeMirror {
  /** Session → tree: select this node's row. Resolves when the tree shows the newest selection asked for. `focus` also moves the keyboard there. */
  show(id: string, focus?: boolean): Promise<void>;
  /** Tree → session: the tree's selection event. Calls `onUser` only when the user made it. */
  selectionChanged(id: string): void;
}

/** `reveal` selects the row in the tree and resolves when the tree has done it. `onUser`: the user selected this node. */
export function treeMirror(reveal: (id: string, focus: boolean) => Promise<void>, onUser: (id: string) => void): TreeMirror;
```

Behaviour:

| Call | What it does |
|---|---|
| `show(id, focus)` | Stores `{ id, focus }` as the one waiting request, replacing an older waiting one. If the older one asked for `focus`, the new one keeps `focus: true`. If no reveal is running, starts the loop. Returns the loop's promise. |
| the loop | While a request waits: take it, `await reveal(id, focus)`, ignore a rejection, look again. When none waits, the mirror is idle. |
| `selectionChanged(id)` | Idle → `onUser(id)`. A reveal running → nothing. |

## 4. Wiring it in (`extension/src/extension.ts`)

| Today | Becomes |
|---|---|
| `revealSelection` calls `sidebarView.reveal` | `revealSelection` reads the selected id and calls `mirror.show(id)` |
| the `Commands` callback (`revealRow`) calls `sidebarView.reveal(row, { select: true, focus: true, expand: false })` | `(id) => mirror.show(id, true)` |
| `onDidChangeSelection` dispatches `select` | it calls `mirror.selectionChanged(row.node.id)` for a node row |
| the comment "`reduce` returns the same session for it, so nothing loops" | deleted: it is the wrong guard |

The mirror is made with:

- `reveal`: find the row (`sidebar.rowForId(id)`); if there is one and (`focus` or `sidebarView.visible`), `await sidebarView.reveal(row, { select: true, focus, expand: false })`.
- `onUser`: `store.dispatch({ type: "select", id })`.

After this, **`sidebarView.reveal` is called in two places only**: inside the mirror's `reveal`, and in the test hook `selectInTree`, which stays
as it is (it stands for a click: no reveal is running, so the mirror reads its event as the user).

The test API returned by `activate` gains one entry, so the suite can do what a canvas click does:

```ts
/** an intent, as a webview posts one (a click on a card) */
dispatch: (intent: Intent) => store.dispatch(intent),
```

## 5. Tests

**Unit**, `extension/src/treeMirror.test.ts`, added to the `unit` script in `extension/package.json`. Use a fake tree whose `reveal` answers
only when the test says so, and which calls `selectionChanged(id)` just before it answers (the echo):

1. the echo of a reveal is not the user;
2. `show(a)`, `show(b)`, `show(c)` before any answer: one reveal at a time, `b` is never revealed, `onUser` is never called;
3. `show` resolves when the newest selection is shown, not the first;
4. with no reveal running, `selectionChanged` calls `onUser`; during a reveal it does not;
5. a `reveal` that rejects does not stop the next one, and `focus: true` survives being replaced by a plain `show`.

**Window**, in `extension/src/test/suite/sidebar.test.ts`, suite "the sidebar":

```
two selections close together settle on the last one
  close the canvas · run workbench.view.extension.vnoc (the tree follows only while visible)
  selectInTree(first) · wait 600 ms
  5 times: dispatch select other, dispatch select first   (no await between them)
  dispatch select other
  wait 1500 ms → selected is other · remember seq
  wait 1500 ms → seq is the same
```

It must **fail before the fix** (run it first, and say what it printed) and pass after. The existing test "a selection from the tree reaches the
session once" must still pass unchanged.

## 6. Order of work

One commit, message `fix: the tree's echo of a reveal is not read as a selection`.

| # | Do | Proof |
|---|---|---|
| 1 | the `dispatch` test hook and the window test | the window test fails with the loop |
| 2 | `treeMirror.ts` and its unit tests | `pnpm --filter vnoc unit` |
| 3 | wire it in (§4) | `pnpm -r typecheck` · `pnpm -r build` · the window suite: the new test and the old one pass |
| 4 | write the answer to the **[check]** of §2 into this file | — |

Then the full test list, and ask the user to check by eye: focus a node, then click several rows and cards quickly; the selection must stay where
the last click put it.

## 7. Not part of this

- The reducer, the session's shape and the messages do not change.
- The webviews do not change: they already keep the rule.
- Arrow keys in the tree keep their behaviour (`02-sidebar.md`): they move VS Code's focus row only.
