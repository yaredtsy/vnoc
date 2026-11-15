import assert from "node:assert/strict";
import { test } from "node:test";

import type { Kind, TreeNode } from "@vnoc/shared/messages.ts";
import { flatten } from "./flatten.ts";
import { type KeyFacts, type SidebarAction, sidebarKey } from "./keys.ts";

const n = (id: string, kind: Kind, children: TreeNode[] = []): TreeNode => ({ id, kind, name: id, path: id, qualname: null, range: null, syntax_error: false, node_id: null, docs: 0, link: null, children });
// rows with pkg and a.py open:   pkg ▾ / a.py ▾ / C ▸ / b.py (leaf) / top.py (leaf)
const tree = n("/", "folder", [n("pkg", "folder", [n("a.py", "file", [n("C", "class", [n("m", "function")])]), n("b.py", "file")]), n("top.py", "file")]);
const rows = flatten(tree, new Set(["pkg", "a.py"]), null); // pkg, a.py, C, b.py, top.py
const focused = flatten(tree, new Set(), "pkg"); // pkg (the focus root), a.py, b.py

const press = (key: string, mods: Partial<KeyFacts> = {}): KeyFacts => ({ key, ctrlKey: false, metaKey: false, altKey: false, shiftKey: false, ...mods });
const select = (id: string): SidebarAction => ({ type: "intent", intent: { type: "select", id } });
const intent = (i: object): SidebarAction => ({ type: "intent", intent: i as never });
const PAGE = 2;

// the table of 06 §7: each row of it
const table: { name: string; key: KeyFacts; rows?: typeof rows; selected: string | null; expect: SidebarAction | null }[] = [
  { name: "↓ moves down and selects", key: press("ArrowDown"), selected: "a.py", expect: select("C") },
  { name: "↓ on the last row stays", key: press("ArrowDown"), selected: "top.py", expect: select("top.py") },
  { name: "↓ with nothing selected: the first row", key: press("ArrowDown"), selected: null, expect: select("pkg") },
  { name: "↓ with a selection that has no row (a call, a node the focus hides): the first row", key: press("ArrowDown"), selected: "call:x", expect: select("pkg") },
  { name: "↑ moves up and selects", key: press("ArrowUp"), selected: "C", expect: select("a.py") },
  { name: "↑ on the first row stays", key: press("ArrowUp"), selected: "pkg", expect: select("pkg") },
  { name: "Home: the first row", key: press("Home"), selected: "b.py", expect: select("pkg") },
  { name: "End: the last row", key: press("End"), selected: "a.py", expect: select("top.py") },
  { name: "PageDown: a screen down, stopping at the end", key: press("PageDown"), selected: "pkg", expect: select("C") },
  { name: "PageDown near the end stops at the last row", key: press("PageDown"), selected: "b.py", expect: select("top.py") },
  { name: "PageUp: a screen up, stopping at the start", key: press("PageUp"), selected: "b.py", expect: select("a.py") },
  { name: "PageUp near the start stops at the first row", key: press("PageUp"), selected: "a.py", expect: select("pkg") },
  { name: "→ on a closed row with children opens it", key: press("ArrowRight"), selected: "C", expect: intent({ type: "expand", ids: ["C"] }) },
  { name: "→ on an open row goes to its first child", key: press("ArrowRight"), selected: "a.py", expect: select("C") },
  { name: "→ on a leaf does nothing", key: press("ArrowRight"), selected: "b.py", expect: null },
  { name: "← on an open row closes it", key: press("ArrowLeft"), selected: "a.py", expect: intent({ type: "toggle", id: "a.py" }) },
  { name: "← on a closed row goes to its parent", key: press("ArrowLeft"), selected: "C", expect: select("a.py") },
  { name: "← on a leaf goes to its parent", key: press("ArrowLeft"), selected: "b.py", expect: select("pkg") },
  { name: "← on a closed top row: it has no parent", key: press("ArrowLeft"), selected: "top.py", expect: null },
  { name: "Space opens or closes a row with children", key: press(" "), selected: "C", expect: intent({ type: "toggle", id: "C" }) },
  { name: "Space on a leaf does nothing", key: press(" "), selected: "b.py", expect: null },
  { name: "Enter is the click rule of the row", key: press("Enter"), selected: "b.py", expect: { type: "click", id: "b.py" } },
  { name: "Enter with nothing selected does nothing", key: press("Enter"), selected: null, expect: null },
  { name: "Ctrl+Enter is Focus on the selected row", key: press("Enter", { ctrlKey: true }), selected: "a.py", expect: intent({ type: "focus", id: "a.py" }) },
  { name: "Cmd+Enter is Focus too", key: press("Enter", { metaKey: true }), selected: "a.py", expect: intent({ type: "focus", id: "a.py" }) },
  { name: "Ctrl+Enter with nothing selected does nothing", key: press("Enter", { ctrlKey: true }), selected: null, expect: null },
  { name: "Alt+← is Back, in the selection's own panel", key: press("ArrowLeft", { altKey: true }), selected: "a.py", expect: intent({ type: "focusBack" }) },
  { name: "Alt+← is Back with nothing selected too", key: press("ArrowLeft", { altKey: true }), selected: null, expect: intent({ type: "focusBack" }) },
  { name: "Shift+F10 is left to the browser: it raises the row's menu", key: press("F10", { shiftKey: true }), selected: "a.py", expect: null },
  { name: "the context-menu key is left to the browser", key: press("ContextMenu"), selected: "a.py", expect: null },
  { name: "a letter is type-ahead, which is part 2: nothing yet", key: press("c"), selected: "a.py", expect: null },
  { name: "an arrow with Ctrl held is not a move", key: press("ArrowDown", { ctrlKey: true }), selected: "a.py", expect: null },
  { name: "an arrow with Shift held is not a move (no range selection here)", key: press("ArrowDown", { shiftKey: true }), selected: "a.py", expect: null },
  { name: "Tab is the browser's: from this tree to the next one", key: press("Tab"), selected: "a.py", expect: null },
  // the focus root is always open: it cannot be closed, and it has no parent row
  { name: "Space on the focused node does nothing", key: press(" "), rows: focused, selected: "pkg", expect: null },
  { name: "← on the focused node does nothing: it is open for good and has no parent", key: press("ArrowLeft"), rows: focused, selected: "pkg", expect: null },
  { name: "→ on the focused node goes to its first child", key: press("ArrowRight"), rows: focused, selected: "pkg", expect: select("a.py") },
  { name: "← on a child of the focused node goes to the focused node", key: press("ArrowLeft"), rows: focused, selected: "a.py", expect: select("pkg") },
];
for (const t of table) test(t.name, () => assert.deepEqual(sidebarKey(t.key, t.rows ?? rows, t.selected, PAGE), t.expect));

test("no rows: no key means anything but Back", () => {
  assert.equal(sidebarKey(press("ArrowDown"), [], null, PAGE), null);
  assert.equal(sidebarKey(press("Enter"), [], null, PAGE), null);
  assert.deepEqual(sidebarKey(press("ArrowLeft", { altKey: true }), [], null, PAGE), intent({ type: "focusBack" }));
});

// the keys that put the cursor in the search box (06 §7)
import { isSearchKey } from "./keys.ts";
const searchTable: { name: string; key: KeyFacts; expect: boolean }[] = [
  { name: "/", key: press("/"), expect: true },
  { name: "Ctrl+F", key: press("f", { ctrlKey: true }), expect: true },
  { name: "Cmd+F", key: press("f", { metaKey: true }), expect: true },
  { name: "Ctrl+Shift+F (VS Code's search in files) is not ours", key: press("F", { ctrlKey: true, shiftKey: true }), expect: false },
  { name: "Alt+F is not ours", key: press("f", { altKey: true }), expect: false },
  { name: "a plain f is type-ahead", key: press("f"), expect: false },
  { name: "? (Shift+/) is not /", key: press("?", { shiftKey: true }), expect: false },
];
for (const t of searchTable) test(`search key: ${t.name}`, () => assert.equal(isSearchKey(t.key), t.expect));
