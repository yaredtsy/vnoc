import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

import { nodeContext, paneContext, panelContext } from "../../webview/src/node-card/nodeContext.ts";

// A menu that names a command that does not exist, or a `when` with a typo, fails silently in VS Code. This reads package.json and fails instead
// (docs/16-ui-redesign/08-menus.md §9). It is not type-checked with the extension: it reaches into the webview for `nodeContext`.

type Entry = { command?: string; submenu?: string; when?: string; group?: string };
const pkg = JSON.parse(readFileSync(new URL("../package.json", import.meta.url), "utf8"));
const menus: Record<string, Entry[]> = pkg.contributes.menus;
const declared = new Set<string>(pkg.contributes.commands.map((c: { command: string }) => c.command));
const submenus = new Set<string>(pkg.contributes.submenus.map((s: { id: string }) => s.id));
const webviewMenu = menus["webview/context"];
const whenOf = (e: Entry) => e.when ?? "";

/** The node menu (a structure node, on a card or a row) and the call menu (a call card), told apart as 08 §2 and §3 do: by `role`. */
const nodeMenu = webviewMenu.filter((e) => e.command && whenOf(e).includes("webviewSection == 'node'") && whenOf(e).includes("role == node"));
const callMenu = webviewMenu.filter((e) => e.command && whenOf(e).includes("webviewSection == 'node'") && whenOf(e).includes("role == call"));

// ── what a `when` clause says ───────────────────────────────────────────────────────────────────────────────────────────

type Keys = { "vnoc.hasMissing": boolean; "vnoc.callTreeExtensions": string[]; "vnoc.focused": boolean; "vnoc.hasSelection": boolean; "vnoc.activeIsExplore": boolean };
const noKeys: Keys = { "vnoc.hasMissing": false, "vnoc.callTreeExtensions": [], "vnoc.focused": false, "vnoc.hasSelection": false, "vnoc.activeIsExplore": false };

/**
 * The part of `when` clauses these menus use: `&&`, `||`, `!`, parentheses, `==`, `!=`, `=~` with a regular expression, `>` and `<` on numbers, and `key in list` (a context key that holds a list), and a bare key
 * (true when set). A bare word on the right of `==` is a string.
 */
function evalWhen(source: string, context: Record<string, unknown>): boolean {
  const regexes: string[] = [];
  const text = source.replace(/\/(?:\\.|[^/\\])+\//g, (re) => (regexes.push(re.slice(1, -1)), `__re${regexes.length - 1}__`));
  const tokens = text.match(/\(|\)|&&|\|\||==|!=|=~|!|[^\s()!&|=]+/g) ?? [];
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
    const op = tokens[at];
    if (op === "==" || op === "!=") {
      at++;
      const same = context[key] === literal(tokens[at++]);
      return op === "==" ? same : !same;
    }
    if (op === "in") {
      at++;
      const list = context[tokens[at++]];
      return Array.isArray(list) && list.includes(context[key]);
    }
    if (op === "=~") {
      at++;
      return new RegExp(regexes[Number(/__re(\d+)__/.exec(tokens[at++])![1])]).test(String(context[key] ?? ""));
    }
    if (op === ">" || op === "<") {
      at++;
      const number = Number(context[key]);
      const bound = Number(tokens[at++]);
      return op === ">" ? number > bound : number < bound;
    }
    return Boolean(context[key]);
  };
  const result = or();
  assert.equal(at, tokens.length, `could not read the whole clause: ${source}`);
  return result;
}

/** The commands a right-click would show: the entries of `menu` whose `when` is true for `context`, in menu order (group, then position). */
function shown(menu: Entry[], context: Record<string, unknown>): string[] {
  return [...menu]
    .filter((e) => e.command && evalWhen(whenOf(e), context))
    .sort((a, b) => (a.group ?? "").localeCompare(b.group ?? "", undefined, { numeric: true }))
    .map((e) => e.command!.replace("vnoc.", ""));
}

/** What a card or a row tells the menu: the facts `nodeContext()` reads. */
const facts = (kind: string, over: Record<string, unknown> = {}) =>
  ({ id: "a.py::f", key: "a.py::f", role: "node", kind, range: kind === "class" || kind === "function" ? { start: { line: 0, character: 0 }, end: { line: 1, character: 0 } } : null, codeOpen: false, docs: 0, link: null, childCount: 0, callsChevron: false, collapsed: true, call: null, ...over }) as Parameters<typeof nodeContext>[0]["node"];
const VIEW = { canvas: "vnoc.canvas", sidebar: "vnoc.structure" } as const;
const context = (surface: "canvas" | "sidebar", node: ReturnType<typeof facts>, keys: Keys = noKeys, isFocusRoot = false) => ({ ...JSON.parse(nodeContext({ node, contextId: "main", isFocusRoot, surface })), ...keys, webviewId: VIEW[surface] });

// ── 1. the declarations ─────────────────────────────────────────────────────────────────────────────────────────────────

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

test("08 §8: the new commands are declared, the removed ones are gone from every menu and from the declarations", () => {
  for (const c of ["vnoc.openInCanvas", "vnoc.explore", "vnoc.goToCallSite", "vnoc.closeContext", "vnoc.closeOtherContexts", "vnoc.openCanvas", "vnoc.showCalls"]) assert.ok(declared.has(c), `${c} is declared`);
  const used = new Set(Object.values(menus).flat().flatMap((e) => (e.command ? [e.command] : [])));
  for (const gone of ["vnoc.showOnCanvas", "vnoc.openCanvasBeside", "vnoc.showCallTree", "vnoc.focusDrop", "vnoc.focusSelection"]) {
    assert.ok(!declared.has(gone), `${gone} is still declared`);
    assert.ok(!used.has(gone), `${gone} is still in a menu`);
    assert.ok(!(pkg.contributes.keybindings as { command: string }[]).some((k) => k.command === gone), `${gone} still has a key binding`);
  }
  const changelog = readFileSync(new URL("../CHANGELOG.md", import.meta.url), "utf8");
  // vnoc.showCallTree was never released, so the CHANGELOG does not name it.
  for (const gone of ["vnoc.showOnCanvas", "vnoc.openCanvasBeside", "vnoc.focusDrop", "vnoc.focusSelection"]) assert.ok(changelog.includes(gone), `CHANGELOG.md does not say ${gone} is gone`);
});

// ── 2. one list for a card and a row ────────────────────────────────────────────────────────────────────────────────────

/** The entries that only make sense on one surface (08 §2): the five that test `surface` (Edit is a card's: a row has no code to edit). */
const SURFACE_ENTRIES = ["vnoc.openInCanvas", "vnoc.revealInSidebar", "vnoc.showCodeInCard", "vnoc.hideCodeInCard", "vnoc.editInCard"];

test("08 §9.2: every node entry names both surfaces, except the five that test `surface`; no other entry mentions it", () => {
  assert.ok(nodeMenu.length > 0);
  for (const e of nodeMenu) {
    const usesSurface = /\bsurface\b/.test(whenOf(e));
    if (SURFACE_ENTRIES.includes(e.command!)) assert.ok(usesSurface, `${e.command} is for one surface and must say which`);
    else assert.ok(!usesSurface, `${e.command} is for both surfaces and must not test \`surface\``);
    assert.ok(whenOf(e).startsWith("webviewId =~ /^vnoc\\.(canvas|structure)$/ && webviewSection == 'node' && role == node"), `${e.command}: ${whenOf(e)} does not name both surfaces`);
  }
  assert.deepEqual([...new Set(nodeMenu.filter((e) => /\bsurface\b/.test(whenOf(e))).map((e) => e.command))].sort(), [...SURFACE_ENTRIES].sort());
});

test("08 §9.2: every call entry tests `surface == canvas`, and names the canvas", () => {
  assert.ok(callMenu.length > 0);
  for (const e of callMenu) {
    assert.match(whenOf(e), /surface == canvas/, `${e.command} must test surface == canvas`);
    assert.match(whenOf(e), /webviewId == 'vnoc\.canvas'/, `${e.command} names the canvas`);
  }
});

test("08 §1: the groups, in this order everywhere: 1_nav · 2_open · 3_calls · 4_code · 5_docs · 9_copy", () => {
  const ORDER = ["1_nav", "2_open", "3_calls", "4_code", "5_docs", "9_copy"];
  for (const [name, menu] of [["node", nodeMenu], ["call", callMenu]] as const) {
    for (const e of menu) assert.ok(ORDER.includes(e.group!.split("@")[0]), `${name} entry ${e.command}: group ${e.group}`);
  }
  for (const sub of ["vnoc.editorMenu", "vnoc.explorerMenu"]) for (const e of menus[sub]) assert.ok(ORDER.includes(e.group!.split("@")[0]), `${sub}: ${e.command}: group ${e.group}`);
  const first = (menu: Entry[]) => new Map(menu.map((e) => [e.command, e.group]));
  for (const e of menus["vnoc.editorMenu"]) if (first(nodeMenu).has(e.command)) assert.equal(e.group!.split("@")[0], first(nodeMenu).get(e.command)!.split("@")[0], `the editor submenu puts ${e.command} in the group the node menu does`);
});

// a node, in every state a menu's `when` reads: the row's menu and the card's menu are one list, but for the five
const kinds = ["folder", "file", "class", "function"] as const;
test("08 §1: a node has the same menu on a row and on its card, in every state, but for the five", () => {
  for (const kind of kinds) {
    for (const linked of [false, true]) {
      for (const hasMissing of [false, true]) {
        for (const callTree of [false, true]) {
          for (const open of [false, true]) {
            for (const isFocusRoot of [false, true]) {
              for (const codeOpen of [false, true]) {
                const node = facts(kind, { link: linked ? "name" : null, docs: linked ? 1 : 0, childCount: kind === "function" ? 0 : 2, collapsed: !open, codeOpen });
                const keys = { ...noKeys, "vnoc.hasMissing": hasMissing, "vnoc.callTreeExtensions": callTree ? [".py"] : [] };
                const row = shown(nodeMenu, context("sidebar", node, keys, isFocusRoot));
                const card = shown(nodeMenu, context("canvas", node, keys, isFocusRoot));
                const label = JSON.stringify({ kind, linked, hasMissing, callTree, open, isFocusRoot, codeOpen });
                assert.deepEqual(row.filter((c) => !card.includes(c)), ["openInCanvas"], `row only, ${label}`);
                const cardOnly = card.filter((c) => !row.includes(c));
                assert.ok(cardOnly.includes("revealInSidebar") && cardOnly.every((c) => ["revealInSidebar", "showCodeInCard", "hideCodeInCard", "editInCard"].includes(c)), `card only ${cardOnly}, ${label}`);
                // 17: Edit is for a function or a class, on a card; never a row, never a file or a folder, whatever the code's state
                assert.equal(card.includes("editInCard"), kind === "class" || kind === "function", `Edit in Card on a card ${label}`);
                assert.equal(row.includes("editInCard"), false, `Edit in Card on a row ${label}`);
                if (card.includes("editInCard")) assert.ok(card.indexOf("editInCard") > card.indexOf("goToCode"), `Edit comes after Go to Code ${label}`);
                // what a node's entries are about
                assert.equal(row.includes("goToCode"), kind !== "folder", `Go to Code ${label}`);
                assert.equal(row.includes("writeId"), linked, `Write ID ${label}`);
                assert.equal(row.includes("showCalls"), callTree && (kind === "class" || kind === "function"), `Show Calls ${label}`);
                assert.equal(row.includes(isFocusRoot ? "focusBack" : "focus"), true, `Focus or Back ${label}`);
                assert.equal(row.includes(isFocusRoot ? "focus" : "focusBack"), false, `Focus or Back ${label}`);
              }
            }
          }
        }
      }
    }
  }
});

test("08 §2: Write ID is on a card and on a row, and both only show it for a node found by name", () => {
  const entries = nodeMenu.filter((e) => e.command === "vnoc.writeId");
  assert.equal(entries.length, 1, "one entry serves both surfaces");
  assert.match(whenOf(entries[0]), /byName/);
});

// ── 3. the call menu ────────────────────────────────────────────────────────────────────────────────────────────────────

const callCard = (over: Record<string, unknown> = {}, sites = 1) =>
  facts("function", { id: 'call:["a.py::f","b.py::g"]', key: "b.py::g", role: "call", callsChevron: true, collapsed: true, call: { sites: Array.from({ length: sites }, () => ({})) }, ...over });
const NODE_ONLY = ["focus", "focusBack", "openInCanvas", "revealInSidebar", "expandAll", "showCalls", "revealInExplorer", "attachMissingDocHere", "writeId"];

// the table of 08 §3: a call card's state → what its menu shows, in menu order
const callRows: { name: string; card: ReturnType<typeof callCard>; expect: string[] }[] = [
  { name: "a known callee, called once, closed", card: callCard(), expect: ["explore", "expand", "goToCallSite", "goToCode", "showCodeInCard", "editInCard", "showDocs", "addDoc", "copyPath"] },
  { name: "…called in several places: the same entry asks which", card: callCard({}, 3), expect: ["explore", "expand", "goToCallSite", "goToCode", "showCodeInCard", "editInCard", "showDocs", "addDoc", "copyPath"] },
  { name: "…open", card: callCard({ collapsed: false }), expect: ["explore", "collapse", "goToCallSite", "goToCode", "showCodeInCard", "editInCard", "showDocs", "addDoc", "copyPath"] },
  { name: "…its code in the card", card: callCard({ codeOpen: true }), expect: ["explore", "expand", "goToCallSite", "goToCode", "hideCodeInCard", "editInCard", "showDocs", "addDoc", "copyPath"] },
  { name: "…with nothing under it: no Expand", card: callCard({ callsChevron: false }), expect: ["explore", "goToCallSite", "goToCode", "showCodeInCard", "editInCard", "showDocs", "addDoc", "copyPath"] },
  { name: "a known callee with no range: no code in the card", card: callCard({ range: null }), expect: ["explore", "expand", "goToCallSite", "goToCode", "showDocs", "addDoc", "copyPath"] },
  { name: "a callee the project does not know: only what is about the call itself", card: callCard({ key: null, range: null }), expect: ["expand", "goToCallSite"] },
  { name: "…and with no site in a file of the project's list (none given): nothing to go to", card: callCard({ key: null, range: null, callsChevron: false }, 0), expect: [] },
];
for (const r of callRows) {
  test(`08 §3: a call card's menu: ${r.name}`, () => {
    const c = context("canvas", r.card);
    assert.deepEqual(shown(callMenu, c), r.expect);
    assert.deepEqual(shown(nodeMenu, c), [], "none of a node's entries shows on a call card");
    assert.deepEqual(shown(webviewMenu, c).filter((x) => NODE_ONLY.includes(x)), [], "…nor does any entry of the lists below it");
  });
}

test("08 §3: the sidebar has no call rows: a call context on the sidebar's surface shows nothing, and a node context never shows a call entry", () => {
  assert.deepEqual(shown(callMenu, context("sidebar", callCard())), []);
  assert.deepEqual(shown(callMenu, context("canvas", facts("function"))), []);
});

// ── 4. VS Code's menus ──────────────────────────────────────────────────────────────────────────────────────────────────

/** The commands that accept a `vscode.Uri` (an editor's, a tab's or the Explorer's right-click gives one): `Commands.nodeFor` reads it. */
const ACCEPT_URI = ["vnoc.revealInSidebar", "vnoc.openInCanvas", "vnoc.focus", "vnoc.showCalls", "vnoc.showDocs", "vnoc.addDoc", "vnoc.attachMissingDocHere", "vnoc.writeId"];

test("08 §4: the editor's and the Explorer's V-NOC submenus are the entries listed there, no more, and name only commands that accept a Uri", () => {
  assert.deepEqual(menus["vnoc.editorMenu"].map((e) => e.command), ["vnoc.revealInSidebar", "vnoc.openInCanvas", "vnoc.focus", "vnoc.showCalls", "vnoc.showDocs", "vnoc.addDoc", "vnoc.attachMissingDocHere", "vnoc.writeId"]);
  assert.deepEqual(menus["vnoc.explorerMenu"].map((e) => e.command), ["vnoc.revealInSidebar", "vnoc.openInCanvas", "vnoc.focus", "vnoc.showDocs", "vnoc.addDoc"]);
  for (const sub of ["vnoc.editorMenu", "vnoc.explorerMenu"]) for (const e of menus[sub]) assert.ok(ACCEPT_URI.includes(e.command!), `${sub}: ${e.command} is not known to accept a Uri`);
  // the conditional ones
  const byCommand = (name: string) => menus["vnoc.editorMenu"].find((e) => e.command === name)!;
  assert.equal(byCommand("vnoc.attachMissingDocHere").when, "vnoc.hasMissing");
  assert.equal(byCommand("vnoc.showCalls").when, "resourceExtname in vnoc.callTreeExtensions");
  // the editor and the tab share one submenu; the Explorer has its own, for files and folders
  assert.equal(menus["editor/context"][0].submenu, "vnoc.editorMenu");
  assert.equal(menus["editor/title/context"][0].submenu, "vnoc.editorMenu");
  assert.equal(menus["explorer/context"][0].submenu, "vnoc.explorerMenu");
  assert.ok(!menus["editor/context"].concat(menus["editor/title/context"], menus["explorer/context"]).some((e) => e.command), "each is one submenu, not loose entries");
});

// ── 5. the other menus ──────────────────────────────────────────────────────────────────────────────────────────────────

const pane = (surface: "canvas" | "sidebar", keys: Partial<Keys> = {}) => ({ ...JSON.parse(paneContext(surface)), ...noKeys, ...keys, webviewId: VIEW[surface] });
const header = (kind: "main" | "explore") => ({ ...JSON.parse(panelContext({ id: kind === "main" ? "main" : "explore:x", kind })), ...noKeys, webviewId: VIEW.sidebar });

test("08 §5: the canvas background", () => {
  assert.deepEqual(shown(webviewMenu, pane("canvas")), ["fitView", "collapseAll"]);
  assert.deepEqual(shown(webviewMenu, pane("canvas", { "vnoc.focused": true, "vnoc.hasSelection": true, "vnoc.activeIsExplore": true })), ["fitView", "collapseAll", "focusBack", "focusClear", "clearSelection", "closeContext"]);
  assert.deepEqual(shown(webviewMenu, pane("canvas", { "vnoc.activeIsExplore": true })), ["fitView", "collapseAll", "closeContext"], "Close Explore only when the active context is an EXPLORE");
});

test("08 §5: the sidebar's empty space", () => {
  assert.deepEqual(shown(webviewMenu, pane("sidebar")), ["collapseAll", "refreshStructure"]);
  assert.deepEqual(shown(webviewMenu, pane("sidebar", { "vnoc.focused": true })), ["collapseAll", "focusClear", "refreshStructure"]);
});

test("08 §5: a sidebar panel's header: Close for an EXPLORE, Close Others for any", () => {
  assert.deepEqual(shown(webviewMenu, header("explore")), ["closeContext", "closeOtherContexts"]);
  assert.deepEqual(shown(webviewMenu, header("main")), ["closeOtherContexts"], "MAIN has no Close");
});

test("08 §5: a section shows only its own menu: a node never shows a pane's entries, nor a header a node's", () => {
  assert.deepEqual(shown(webviewMenu, context("canvas", facts("file"))).filter((c) => ["fitView", "closeContext", "refreshStructure", "closeOtherContexts"].includes(c)), []);
  assert.deepEqual(shown(webviewMenu, header("explore")).filter((c) => NODE_ONLY.concat(["copyPath", "showDocs"]).includes(c)), []);
  assert.deepEqual(shown(webviewMenu, pane("canvas")).filter((c) => c === "collapseAll"), ["collapseAll"]);
  assert.ok(!shown(webviewMenu, { ...pane("canvas"), webviewId: "vnoc.docs" }).length, "another webview gets none of V-NOC's entries");
});

test("every key a card's, a row's, a pane's or a header's `when` clause tests is a key those contexts write", () => {
  const written = new Set([
    ...Object.keys(JSON.parse(nodeContext({ node: facts("function"), contextId: "main", isFocusRoot: false }))),
    ...Object.keys(JSON.parse(nodeContext({ node: callCard(), contextId: "main", isFocusRoot: false }))),
    ...Object.keys(JSON.parse(paneContext("canvas"))),
    ...Object.keys(JSON.parse(panelContext({ id: "main", kind: "main" }))),
  ]);
  const VSCODE = new Set(["webviewId", "webviewSection"]); // VS Code's own keys, and V-NOC's global ones (`vnoc.…`)
  for (const e of webviewMenu) {
    for (const term of whenOf(e).replace(/\/(?:\\.|[^/\\])+\//g, "").split(/&&|\|\||[()]/)) {
      const key = term.trim().replace(/^!/, "").split(/\s*(?:==|!=|=~| in |>|<)\s*/)[0].trim();
      if (!key || VSCODE.has(key) || key.startsWith("vnoc.")) continue;
      assert.ok(written.has(key), `\`${e.command}\`: the when clause tests \`${key}\`, which nothing writes`);
    }
  }
});

test("the global keys a `when` tests are ones V-NOC sets", () => {
  const known = new Set(["vnoc.extensions", "vnoc.callTreeExtensions", "vnoc.hasMissing", "vnoc.canvasOpen", "vnoc.focused", "vnoc.hasSelection", "vnoc.activeIsExplore", "vnoc.hasAttention"]);
  for (const e of Object.values(menus).flat()) for (const key of whenOf(e).match(/vnoc\.[A-Za-z]+/g) ?? []) assert.ok(known.has(key) || /^vnoc\.(canvas|structure|attention|editorMenu|explorerMenu)$/.test(key), `${e.command ?? e.submenu}: ${key}`);
});

// ── the move from the native tree (8.5), and what 8.7 changed on purpose ──────────────────────────────────────────────

/**
 * The entries `view/item/context` had for `vnoc.structure` before 8.5, with the group each has now and the condition on the row's `contextValue` (tested with
 * `=~`). Each must still be on a row, true for the same rows. **Changed on purpose in 8.7** (08 §2): `Show on Canvas` and `Open Canvas` (on a node) are gone, and
 * `Open in Canvas` is the one entry for both; the groups were renamed (`1_show` → `1_nav` …, `5_calls` → `3_calls`, `3_code` → `4_code`, `4_docs` → `5_docs`).
 */
const FORMER: { command: string; group: string; old: (item: string, keys: Keys) => boolean }[] = [
  { command: "vnoc.focus", group: "1_nav@1", old: (i) => /\bnode\b/.test(i) },
  { command: "vnoc.goToCode", group: "4_code@1", old: (i) => /\b(file|class|function)\b/.test(i) },
  { command: "vnoc.revealInExplorer", group: "4_code@3", old: (i) => /\b(file|folder)\b/.test(i) },
  { command: "vnoc.showDocs", group: "5_docs@1", old: (i) => /\bnode\b/.test(i) },
  { command: "vnoc.addDoc", group: "5_docs@2", old: (i) => /\bnode\b/.test(i) },
  { command: "vnoc.attachMissingDocHere", group: "5_docs@3", old: (i, k) => /\bnode\b/.test(i) && k["vnoc.hasMissing"] },
  { command: "vnoc.writeId", group: "5_docs@4", old: (i) => /\bnode\b/.test(i) && /\bbyName\b/.test(i) },
  { command: "vnoc.copyPath", group: "9_copy@1", old: (i) => /\bnode\b/.test(i) },
  { command: "vnoc.showCalls", group: "3_calls@1", old: (i, k) => /\b(class|function)\b/.test(i) && k["vnoc.callTreeExtensions"].length > 0 },
  // the two that became one: both were shown on every node
  { command: "vnoc.openInCanvas", group: "1_nav@2", old: (i) => /\bnode\b/.test(i) },
];

test("every former `view/item/context` entry of the native tree is still on a row, in its (renamed) group, true for the same rows", () => {
  assert.ok(menus["view/item/context"].every((e) => !(whenOf(e)).includes("vnoc.structure")), "nothing is left for the native view");
  for (const former of FORMER) {
    const twin = nodeMenu.find((e) => e.command === former.command);
    assert.ok(twin, `${former.command} is not on a row`);
    assert.equal(twin.group, former.group, `${former.command}: the group`);
    for (const kind of kinds) {
      for (const byName of [false, true]) {
        for (const hasMissing of [false, true]) {
          for (const callTree of [false, true]) {
            const keys = { ...noKeys, "vnoc.hasMissing": hasMissing, "vnoc.callTreeExtensions": callTree ? [".py"] : [] };
            const contextValue = ["node", kind, "docs", "hasCode", byName ? "byName" : ""].filter(Boolean).join(" ");
            const row = context("sidebar", facts(kind, byName ? { link: "name", docs: 1 } : {}), keys);
            assert.equal(evalWhen(whenOf(twin), row), former.old(contextValue, keys), `${former.command} on a ${kind} row (byName ${byName}, hasMissing ${hasMissing}, callTree ${callTree})`);
          }
        }
      }
    }
  }
});

test("what 8.7 changed on a row on purpose: Show on Canvas and Open Canvas are gone, and one Open in Canvas is there", () => {
  const onRow = shown(nodeMenu, context("sidebar", facts("function", { childCount: 0 })));
  assert.ok(onRow.includes("openInCanvas") && !onRow.includes("showOnCanvas") && !onRow.includes("openCanvas"));
  // `Expand` / `Collapse` and `Expand All Below` are new on a row (the card had them; 08 §2 gives them to both)
  const folder = shown(nodeMenu, context("sidebar", facts("folder", { childCount: 2 })));
  assert.ok(folder.includes("expand") && folder.includes("expandAll"));
  assert.ok(!shown(nodeMenu, context("sidebar", facts("function", { childCount: 2 }))).includes("expandAll"), "Expand All Below is not for a function (05 §4)");
});

test("the two key bindings that tested `focusedView` are gone: a webview view never sets it (06 §10)", () => {
  for (const k of pkg.contributes.keybindings as { when?: string }[]) assert.ok(!(k.when ?? "").includes("focusedView"), `a key binding tests focusedView: ${k.when}`);
  const views = (pkg.contributes.views.vnoc as { id: string; type?: string }[]).map((v) => [v.id, v.type]);
  assert.deepEqual(views.find(([id]) => id === "vnoc.structure"), ["vnoc.structure", "webview"]);
  assert.ok(!views.some(([id]) => id === "vnoc.focusStack"), "the FOCUS view is gone");
  assert.equal(pkg.contributes.viewsWelcome, undefined, "the states are the page's now");
});

test("the view's title: Open Canvas and Collapse All are buttons; Refresh, Search and Restore are in the ⋯ menu, Help and Feedback in its last group (06 §9, 18.03 §5)", () => {
  const title = (menus["view/title"] as Entry[]).filter((e) => e.when === "view == vnoc.structure").map((e) => [e.command, e.group]);
  assert.deepEqual(title.sort(), [["vnoc.collapseAll", "navigation@2"], ["vnoc.openCanvas", "navigation@1"], ["vnoc.help", "9_help@1"], ["vnoc.refreshStructure", "1_more@1"], ["vnoc.restoreRemoved", "1_more@3"], ["vnoc.search", "1_more@2"]].sort());
  for (const command of ["vnoc.openCanvas", "vnoc.collapseAll"]) assert.ok((pkg.contributes.commands as { command: string; icon?: string }[]).find((c) => c.command === command)?.icon, `${command} needs an icon to be a button`);
});

test("the evaluator reads the clauses it is asked to (so a test above cannot pass by reading nothing)", () => {
  const c = { webviewId: "vnoc.structure", webviewSection: "node", kind: "file", "vnoc.callTreeExtensions": [".py"], ext: ".py", open: false };
  assert.equal(evalWhen("webviewId =~ /^vnoc\\.(canvas|structure)$/ && webviewSection == 'node'", c), true);
  assert.equal(evalWhen("webviewId =~ /^vnoc\\.(canvas|structure)$/", { ...c, webviewId: "vnoc.docs" }), false);
  assert.equal(evalWhen("(kind == file || kind == folder) && !open", c), true);
  assert.equal(evalWhen("kind != file", c), false);
  assert.equal(evalWhen("vnoc.callTreeExtensions && open", c), false);
  assert.equal(evalWhen("ext in vnoc.callTreeExtensions", c), true);
  assert.equal(evalWhen("ext in vnoc.callTreeExtensions", { ...c, ext: ".bend" }), false);
  assert.equal(evalWhen("kind == file && ext in vnoc.callTreeExtensions", { ...c, "vnoc.callTreeExtensions": undefined }), false);
});

test("Show Calls on a node is offered by the extension of its file, whatever the language", () => {
  const onKey = (key: string, extensions: string[]) => shown(nodeMenu, context("sidebar", facts("function", { key, id: key }), { ...noKeys, "vnoc.callTreeExtensions": extensions })).includes("showCalls");
  assert.equal(onKey("lib.bend::Nat.double", [".bend"]), true);
  assert.equal(onKey("lib.bend::Nat.double", [".py"]), false);
  assert.equal(onKey("pkg/a.py::C.f", [".bend", ".py"]), true);
  assert.equal(onKey("pkg.d/a::f", [".py"]), false, "a dot in a folder is not an extension");
  assert.equal(onKey("a.py::f", []), false);
});
