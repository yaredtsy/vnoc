import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

import { nodeContext } from "../../webview/src/node-card/nodeContext.ts";

// A menu that names a command that does not exist, or a `when` with a typo, fails silently in VS Code. This reads package.json and fails instead
// (docs/10-ui-rework/04-menus.md §6). It is not type-checked with the extension: it reaches into the webview for `nodeContext`.

type Entry = { command?: string; submenu?: string; when?: string; group?: string };
const pkg = JSON.parse(readFileSync(new URL("../package.json", import.meta.url), "utf8"));
const menus: Record<string, Entry[]> = pkg.contributes.menus;
const declared = new Set<string>(pkg.contributes.commands.map((c: { command: string }) => c.command));
const submenus = new Set<string>(pkg.contributes.submenus.map((s: { id: string }) => s.id));

/** A row's node menu (the right-click one, not the inline buttons) and a card's node menu. */
const rowMenu = menus["view/item/context"].filter((e) => e.command && !e.group?.startsWith("inline") && (e.when ?? "").startsWith("view == vnoc.structure") && !(e.when ?? "").includes("focusBar"));
const cardMenu = menus["webview/context"].filter((e) => e.command && (e.when ?? "").includes("webviewSection == 'node'"));

/** The entries that only make sense on one surface. */
const ROW_ONLY = ["vnoc.showOnCanvas", "vnoc.openCanvas"]; // a card is already on the canvas
const CARD_ONLY = ["vnoc.revealInSidebar", "vnoc.focusBack", "vnoc.expand", "vnoc.collapse", "vnoc.expandAll", "vnoc.showCodeInCard", "vnoc.hideCodeInCard"];

const groupOf = (e: Entry) => (e.group ?? "").split("@")[0];
const byGroup = (entries: Entry[], skip: string[]) => {
  const seen = new Map<string, string>(); // command -> group, in menu order
  [...entries].sort((a, b) => (a.group ?? "").localeCompare(b.group ?? "")).forEach((e) => skip.includes(e.command!) || seen.has(e.command!) || seen.set(e.command!, groupOf(e)));
  return [...seen];
};

test("every command a menu uses is declared, with the category V-NOC", () => {
  for (const [menu, entries] of Object.entries(menus)) {
    for (const e of entries.filter((e) => e.command)) {
      assert.ok(declared.has(e.command!), `${menu}: \`${e.command}\` is not in contributes.commands`);
    }
  }
  for (const c of pkg.contributes.commands as { command: string; title: string; category?: string }[]) {
    assert.equal(c.category, "V-NOC", `${c.command} has no category V-NOC`);
    assert.ok(!c.title.startsWith("V-NOC"), `${c.command}: the title repeats the category`);
  }
});

test("every submenu a menu uses is declared, and every declared one is used", () => {
  const used = new Set(Object.values(menus).flat().flatMap((e) => (e.submenu ? [e.submenu] : [])));
  for (const id of used) assert.ok(submenus.has(id), `submenu ${id} is not in contributes.submenus`);
  for (const id of submenus) assert.ok(used.has(id), `submenu ${id} is declared but no menu uses it`);
  for (const id of submenus) assert.ok(menus[id]?.length > 0, `submenu ${id} has no entries`);
});

test("a node has the same menu on a sidebar row and on its card, in the same groups and order, except the listed entries", () => {
  assert.deepEqual(byGroup(rowMenu, ROW_ONLY), byGroup(cardMenu, CARD_ONLY));
  // the exceptions really are exceptions
  for (const c of ROW_ONLY) assert.ok(rowMenu.some((e) => e.command === c) && !cardMenu.some((e) => e.command === c), `${c} is meant for the row only`);
  for (const c of CARD_ONLY) assert.ok(cardMenu.some((e) => e.command === c) && !rowMenu.some((e) => e.command === c), `${c} is meant for the card only`);
});

test("every key a card's `when` clause tests is a key nodeContext() writes", () => {
  const written = new Set(
    Object.keys(
      JSON.parse(
        nodeContext({
          node: { id: "a.py::f", kind: "function", name: "f", path: "a.py", line: 1, range: null, codeOpen: false, depth: 0, x: 0, y: 0, childCount: 0, collapsed: false, syntaxError: false, docs: 0, link: null },
          contextId: "main",
          isFocusRoot: false,
        }),
      ),
    ),
  );
  const VSCODE = new Set(["webviewId", "webviewSection"]); // VS Code's own keys, and V-NOC's global ones (`vnoc.…`)
  for (const e of cardMenu) {
    for (const term of (e.when ?? "").split(/&&|\|\||[()]/)) {
      const key = term.trim().replace(/^!/, "").split(/\s*(?:==|!=|=~| in )\s*/)[0].trim();
      if (!key || VSCODE.has(key) || key.startsWith("vnoc.")) continue;
      assert.ok(written.has(key), `\`${e.command}\`: the when clause tests \`${key}\`, which nodeContext() does not write`);
    }
  }
});
