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

/** A row's node menu (the sidebar is a webview: its rows say `webviewId == 'vnoc.structure'`) and a card's node menu (`vnoc.canvas`). The row's inline buttons are the strip's. */
const rowMenu = menus["webview/context"].filter((e) => e.command && (e.when ?? "").startsWith("webviewId == 'vnoc.structure' && webviewSection == 'node'"));
const cardMenu = menus["webview/context"].filter((e) => e.command && (e.when ?? "").includes("webviewId == 'vnoc.canvas'") && (e.when ?? "").includes("webviewSection == 'node'"));

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

/** What a card or a row tells the menu: the facts `nodeContext()` reads. */
const facts = (kind: string, over: Record<string, unknown> = {}) =>
  ({ id: "a.py::f", key: "a.py::f", role: "node", kind, range: kind === "class" || kind === "function" ? { start: { line: 0, character: 0 }, end: { line: 1, character: 0 } } : null, codeOpen: false, docs: 0, link: null, childCount: 0, callsChevron: false, collapsed: true, call: null, ...over }) as Parameters<typeof nodeContext>[0]["node"];

test("every key a card's or a row's `when` clause tests is a key nodeContext() writes", () => {
  const written = new Set(Object.keys(JSON.parse(nodeContext({ node: facts("function"), contextId: "main", isFocusRoot: false }))));
  const VSCODE = new Set(["webviewId", "webviewSection"]); // VS Code's own keys, and V-NOC's global ones (`vnoc.…`)
  for (const e of [...cardMenu, ...rowMenu]) {
    for (const term of (e.when ?? "").split(/&&|\|\||[()]/)) {
      const key = term.trim().replace(/^!/, "").split(/\s*(?:==|!=|=~| in )\s*/)[0].trim();
      if (!key || VSCODE.has(key) || key.startsWith("vnoc.")) continue;
      assert.ok(written.has(key), `\`${e.command}\`: the when clause tests \`${key}\`, which nodeContext() does not write`);
    }
  }
});

test("Write ID is on the row and on the card, and both only show it for a node found by name", () => {
  for (const [name, menu] of [["row", rowMenu], ["card", cardMenu]] as const) {
    const entry = menu.find((e) => e.command === "vnoc.writeId");
    assert.ok(entry, `Write ID is in the ${name} menu`);
    assert.match(entry.when ?? "", /byName/, `the ${name} entry tests byName`);
  }
});

// ── the sidebar's row menu moved from the native tree to `webview/context` (docs/16-ui-redesign/10-build-order.md §8.5, part 1) ──────────────────────────

/**
 * The entries `view/item/context` had for `vnoc.structure`, as they were in package.json before 8.5: the command, the group, and the condition on the row's
 * `contextValue` (the words `node file hasCode byName`, tested with `=~`) and on the global keys. Each one must have a twin on the new rows that is true
 * for the same rows.
 */
const FORMER: { command: string; group: string; old: (item: string, keys: Keys) => boolean }[] = [
  { command: "vnoc.focus", group: "1_show@1", old: (i) => /\bnode\b/.test(i) },
  { command: "vnoc.showOnCanvas", group: "1_show@2", old: (i) => /\bnode\b/.test(i) },
  { command: "vnoc.openCanvas", group: "1_show@3", old: (i) => /\bnode\b/.test(i) },
  { command: "vnoc.goToCode", group: "3_code@1", old: (i) => /\b(file|class|function)\b/.test(i) },
  { command: "vnoc.revealInExplorer", group: "3_code@3", old: (i) => /\b(file|folder)\b/.test(i) },
  { command: "vnoc.showDocs", group: "4_docs@1", old: (i) => /\bnode\b/.test(i) },
  { command: "vnoc.addDoc", group: "4_docs@2", old: (i) => /\bnode\b/.test(i) },
  { command: "vnoc.attachMissingDocHere", group: "4_docs@3", old: (i, k) => /\bnode\b/.test(i) && k["vnoc.hasMissing"] },
  { command: "vnoc.writeId", group: "4_docs@4", old: (i) => /\bnode\b/.test(i) && /\bbyName\b/.test(i) },
  { command: "vnoc.copyPath", group: "9_copy@1", old: (i) => /\bnode\b/.test(i) },
  { command: "vnoc.showCalls", group: "5_calls@1", old: (i, k) => /\b(class|function)\b/.test(i) && k["vnoc.callTree"] },
];
type Keys = { "vnoc.hasMissing": boolean; "vnoc.callTree": boolean };

/** The part of `when` clauses these menus use: `&&`, `||`, `!`, parentheses, `==`, `!=`, and a bare key (true when set). A bare word on the right is a string. */
function evalWhen(source: string, context: Record<string, unknown>): boolean {
  const tokens = source.match(/\(|\)|&&|\|\||==|!=|!|[^\s()!&|=]+/g) ?? [];
  let at = 0;
  const literal = (t: string) => (t.startsWith("'") ? t.slice(1, -1) : t === "true" ? true : t === "false" ? false : t);
  const or = (): boolean => {
    let v = and();
    while (tokens[at] === "||") (at++, (v = and() || v));
    return v;
  };
  const and = (): boolean => {
    let v = not();
    while (tokens[at] === "&&") (at++, (v = not() && v));
    return v;
  };
  const not = (): boolean => (tokens[at] === "!" ? (at++, !not()) : atom());
  const atom = (): boolean => {
    if (tokens[at] === "(") {
      at++;
      const v = or();
      assert.equal(tokens[at++], ")");
      return v;
    }
    const key = tokens[at++];
    if (tokens[at] === "==" || tokens[at] === "!=") {
      const eq = tokens[at++] === "==";
      return (context[key] === literal(tokens[at++])) === eq;
    }
    return Boolean(context[key]);
  };
  const result = or();
  assert.equal(at, tokens.length, `could not read the whole clause: ${source}`);
  return result;
}

test("every former `view/item/context` entry of the native tree has a twin on a row, in the same group, true for the same rows", () => {
  const twins = menus["webview/context"].filter((e) => e.command && (e.when ?? "").includes("webviewId == 'vnoc.structure'"));
  assert.equal(twins.length, FORMER.length, "no entry on a row that the native tree did not have");
  assert.ok(menus["view/item/context"].every((e) => !(e.when ?? "").includes("vnoc.structure")), "nothing is left for the native view");
  for (const former of FORMER) {
    const twin = twins.find((e) => e.command === former.command);
    assert.ok(twin, `${former.command} has no twin on a row`);
    assert.equal(twin.group, former.group, `${former.command}: the group moved`);
    for (const kind of ["folder", "file", "class", "function"]) {
      for (const byName of [false, true]) {
        for (const hasMissing of [false, true]) {
          for (const callTree of [false, true]) {
            const keys: Keys = { "vnoc.hasMissing": hasMissing, "vnoc.callTree": callTree };
            const contextValue = ["node", kind, "docs", "hasCode", byName ? "byName" : ""].filter(Boolean).join(" ");
            const row = JSON.parse(nodeContext({ node: facts(kind, byName ? { link: "name", docs: 1 } : {}), contextId: "main", isFocusRoot: false, surface: "sidebar" }));
            const now = evalWhen(twin.when!, { ...row, ...keys, webviewId: "vnoc.structure" });
            assert.equal(now, former.old(contextValue, keys), `${former.command} on a ${kind} row (byName ${byName}, hasMissing ${hasMissing}, callTree ${callTree})`);
          }
        }
      }
    }
  }
});

test("a canvas card never gets a row's entry, and a row never a card's", () => {
  for (const e of rowMenu) assert.ok(!(e.when ?? "").includes("vnoc.canvas"), e.command);
  for (const e of cardMenu) assert.ok(!(e.when ?? "").includes("vnoc.structure"), e.command);
  assert.ok(evalWhen("webviewId == 'vnoc.structure' && webviewSection == 'node'", { webviewId: "vnoc.structure", webviewSection: "node" }));
  assert.ok(!evalWhen("webviewId == 'vnoc.structure' && webviewSection == 'node'", { webviewId: "vnoc.canvas", webviewSection: "node" }));
});

test("the two key bindings that tested `focusedView` are gone: a webview view never sets it (06 §10)", () => {
  for (const k of pkg.contributes.keybindings as { when?: string }[]) assert.ok(!(k.when ?? "").includes("focusedView"), `a key binding tests focusedView: ${k.when}`);
  const commands = new Set((pkg.contributes.commands as { command: string }[]).map((c) => c.command));
  for (const gone of ["vnoc.focusSelection", "vnoc.focusDrop"]) assert.ok(!commands.has(gone), `${gone} is still declared`);
  const views = (pkg.contributes.views.vnoc as { id: string; type?: string }[]).map((v) => [v.id, v.type]);
  assert.deepEqual(views.find(([id]) => id === "vnoc.structure"), ["vnoc.structure", "webview"]);
  assert.ok(!views.some(([id]) => id === "vnoc.focusStack"), "the FOCUS view is gone");
  assert.equal(pkg.contributes.viewsWelcome, undefined, "the states are the page's now");
});

test("the view's title: Open Canvas and Collapse All are buttons; Refresh, Search and Restore are in the ⋯ menu (06 §9)", () => {
  const title = (menus["view/title"] as Entry[]).filter((e) => e.when === "view == vnoc.structure").map((e) => [e.command, e.group]);
  assert.deepEqual(title.sort(), [["vnoc.collapseAll", "navigation@2"], ["vnoc.openCanvas", "navigation@1"], ["vnoc.refreshStructure", "1_more@1"], ["vnoc.restoreRemoved", "1_more@3"], ["vnoc.search", "1_more@2"]].sort());
  for (const command of ["vnoc.openCanvas", "vnoc.collapseAll"]) assert.ok((pkg.contributes.commands as { command: string; icon?: string }[]).find((c) => c.command === command)?.icon, `${command} needs an icon to be a button`);
});
