import assert from "node:assert/strict";
import { test } from "node:test";

import type { Kind, TreeNode } from "@vnoc/shared/messages.ts";
import { nodeContext } from "../node-card/nodeContext.ts";
import type { FlatRow } from "./flatten.ts";
import { rowDescription, rowFacts, rowLabel } from "./rowFacts.ts";

const RANGE = { start: { line: 0, character: 0 }, end: { line: 3, character: 0 } };
const node = (kind: Kind, over: Partial<TreeNode> = {}): TreeNode => ({ id: `${kind}-id`, kind, name: kind, path: "p", qualname: null, range: null, syntax_error: false, node_id: null, docs: 0, link: null, children: [], ...over });
const row = (n: TreeNode, over: Partial<FlatRow> = {}): FlatRow => ({ id: n.id, node: n, depth: 0, hasChildren: n.children.length > 0, open: false, parent: null, focusRoot: false, inClass: false, posInSet: 1, setSize: 1, ...over });
const context = (r: FlatRow, contextId = "main") => JSON.parse(nodeContext({ node: rowFacts(r), contextId, isFocusRoot: r.focusRoot, surface: "sidebar" }));

// the row's nodeContext table: kind × docs × link → the object the menu's `when` clauses and the commands read (07 §6)
const kinds: { kind: Kind; range: typeof RANGE | null; children: number }[] = [
  { kind: "folder", range: null, children: 1 },
  { kind: "file", range: null, children: 1 },
  { kind: "class", range: RANGE, children: 1 },
  { kind: "function", range: RANGE, children: 0 },
];
const docs: { docs: number; link: TreeNode["link"] }[] = [{ docs: 0, link: null }, { docs: 2, link: "marker" }, { docs: 1, link: "name" }];
for (const k of kinds) {
  for (const d of docs) {
    test(`nodeContext of a ${k.kind} row with ${d.docs} docs, link ${d.link}`, () => {
      const n = node(k.kind, { range: k.range, docs: d.docs, link: d.link, children: Array.from({ length: k.children }, () => node("function", { id: "child" })) });
      assert.deepEqual(context(row(n)), {
        surface: "sidebar",
        webviewSection: "node",
        preventDefaultContextMenuItems: true,
        id: n.id,
        nodeKey: n.id,
        fileExtname: "",
        role: "node",
        calleeKnown: true,
        sites: 0,
        contextId: "main",
        kind: k.kind,
        hasChildren: k.children > 0,
        open: false,
        isFocusRoot: false,
        hasCode: k.range !== null,
        codeOpen: false,
        docs: d.docs,
        byName: d.link === "name",
        tours: 0,
      });
    });
  }
}

test("a row says it is open, and the focus root says so", () => {
  const n = node("file", { children: [node("function", { id: "f" })] });
  assert.equal(context(row(n, { open: true })).open, true);
  assert.equal(context(row(n, { open: true, focusRoot: true })).isFocusRoot, true);
  assert.equal(context(row(node("function"), { open: true })).open, false, "a leaf is never open");
});

test("a card and a row of the same node write the same keys, but for the surface", () => {
  const n = node("class", { range: RANGE, children: [node("function", { id: "m" })] });
  const card = { ...rowFacts(row(n)), codeOpen: false };
  const asCard = JSON.parse(nodeContext({ node: card, contextId: "main", isFocusRoot: false }));
  assert.deepEqual({ ...asCard, surface: "sidebar" }, context(row(n)));
  assert.equal(asCard.surface, "canvas");
});

test("the description: syntax error first, else the docs, else nothing", () => {
  assert.equal(rowDescription(node("file", { syntax_error: true, docs: 3 })), "syntax error");
  assert.equal(rowDescription(node("file", { docs: 3 })), "📄 3");
  assert.equal(rowDescription(node("file")), "");
});

// the table of 09 §6: what a screen reader says of a row
const said: { name: string; node: TreeNode; row?: Partial<FlatRow>; state?: { selected: boolean; cursor: boolean }; expect: string }[] = [
  { name: "the sentence of 09 §6", node: node("function", { name: "checkout", docs: 1 }), row: { depth: 2, posInSet: 2, setSize: 4 }, state: { selected: true, cursor: true }, expect: "function checkout, selected, 1 doc, cursor here, level 3, 2 of 4" },
  { name: "a top row", node: node("folder", { name: "app" }), expect: "folder app, level 1, 1 of 1" },
  { name: "two docs, not selected", node: node("file", { name: "api.py", docs: 2 }), row: { posInSet: 3, setSize: 3 }, expect: "file api.py, 2 docs, level 1, 3 of 3" },
  { name: "a file that did not parse", node: node("file", { name: "bad.py", syntax_error: true }), expect: "file bad.py, syntax error, level 1, 1 of 1" },
  { name: "only the cursor", node: node("class", { name: "Order" }), row: { depth: 1 }, state: { selected: false, cursor: true }, expect: "class Order, cursor here, level 2, 1 of 1" },
];
for (const s of said) test(`what a screen reader says of a row: ${s.name}`, () => assert.equal(rowLabel(row(s.node, s.row), s.state ?? { selected: false, cursor: false }), s.expect));
