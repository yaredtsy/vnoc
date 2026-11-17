import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";
import * as vscode from "vscode";

import type { SearchResult, StructureResult, TreeNode } from "@vnoc/shared/messages";
import type { Session } from "@vnoc/shared/session.ts";
import { sleep, until, waitFor } from "./util";

// Search in a real window (docs/20-search/06-build-order.md S3): the STRUCTURE view's box asks the core and draws what it answers, and *V-NOC: Search…* puts the cursor in that box (a QuickPick re-sorted the core's order: S3b).
// The suite cannot type in a webview, so `setSearch` puts text in the box as typing would, and the page says what it drew (`searchDrawn`). What a query finds, and in what
// order, is the core's own test (`crates/vnoc/tests/lifecycle.rs`); here it is that the answer reaches the screen, and that only the newest one does.

interface Drawn {
  query: string;
  ids: string[];
  total: number;
  notes: string[];
  ms: number;
  trip: number;
}
interface Api {
  structure(): Promise<StructureResult>;
  request<T>(method: string, params?: object): Promise<T>;
  session(): Session;
  searchDrawn(): Drawn[];
  setSearch(text: string): void;
  searchFocused(): { focused: boolean; text: string }[];
  searchKey(key: string): void;
  sidebarOpen(): boolean;
  sidebarVisible(): boolean;
  webviewErrors(): string[];
}
let api: Api;
/** how many answers had been drawn before this suite began: the timing at its end counts only its own */
let drawnAtStart = 0;

const folder = () => vscode.workspace.workspaceFolders![0].uri.fsPath;
const nodesOf = async () => {
  const all: TreeNode[] = [];
  const walk = (n: TreeNode) => (all.push(n), n.children.forEach(walk));
  (await api.structure()).root.children.forEach(walk);
  return all;
};
/** Waits until an answer is drawn after the first `from` ones that satisfies `ok`, and gives it. */
async function drawn(what: string, from: number, ok: (d: Drawn) => boolean, ms = 20_000): Promise<Drawn> {
  let found: Drawn | undefined;
  await until(what, () => (found = api.searchDrawn().slice(from).find(ok)) !== undefined, ms);
  return found!;
}
const median = (values: number[]) => [...values].sort((a, b) => a - b)[Math.floor(values.length / 2)];

/**
 * The box can only be typed in, and its page only runs at full speed, while the view is on screen: a page VS Code calls hidden has its timers and drawing held back (answers came
 * a second late, and keys typed a moment apart drew as one). Earlier suites (`attention` makes the NEEDS ATTENTION view appear in the same container) can leave it hidden, so each test
 * shows it first, and says when it had to.
 */
async function showSidebar(): Promise<void> {
  if (!api.sidebarVisible()) console.log("[search] the STRUCTURE view was hidden: showing it");
  await vscode.commands.executeCommand("workbench.view.extension.vnoc");
  await vscode.commands.executeCommand("vnoc.structure.focus");
  await until("the STRUCTURE view to be on screen", () => api.sidebarVisible(), 30_000);
  await sleep(300);
}

suite("search", () => {
  suiteSetup(async () => {
    api = await vscode.extensions.getExtension("vnoc.vnoc")!.activate();
    // the box is in the STRUCTURE view: its page must exist and have drawn the tree before it can be typed in
    await showSidebar();
    await until("the STRUCTURE page to exist", () => api.sidebarOpen(), 30_000);
    await waitFor("the tree", async () => (await api.structure()).root.children.length > 0, 30_000);
    await sleep(1000); // the page asked for the structure and drew it
    drawnAtStart = api.searchDrawn().length;
  });

  suiteTeardown(() => api.setSearch("")); // the panels come back, and the highlight clears, for the suites that follow

  test("the box shows the core's rows, which the old ranking could not give: `outer.meth` is in no name and no path", async () => {
    await showSidebar();
    const methods = (await nodesOf()).filter((n) => n.qualname === "Outer.method").map((n) => n.id);
    assert.strictEqual(methods.length, 2, "the fixture defines Outer.method twice");
    const from = api.searchDrawn().length;
    api.setSearch("outer.meth");
    const got = await drawn("the answer for `outer.meth`", from, (d) => d.query === "outer.meth");
    assert.deepStrictEqual(got.ids, methods, "the two Outer.method nodes, in the core's order");
    assert.strictEqual(got.total, 2);
    const core = await api.request<SearchResult>("vnoc/search", { query: "outer.meth" });
    assert.deepStrictEqual(got.ids, core.nodes.map((n) => n.id), "what is drawn is what the core says");
    api.setSearch("");
  });

  test("typing fast: only the newest query's answer is drawn, and no other comes after it", async () => {
    await showSidebar();
    const from = api.searchDrawn().length;
    for (const text of ["o", "ou", "out", "oute", "outer"]) api.setSearch(text); // one after another, no waiting
    await drawn("the answer for `outer`", from, (d) => d.query === "outer");
    await sleep(1000);
    const after = api.searchDrawn().slice(from);
    assert.strictEqual(after.at(-1)!.query, "outer", `the last answer drawn is for the last text (drawn: ${after.map((d) => d.query).join(", ")})`);
    // a pause well over the debounce between keys, as a person types: every key is asked for and drawn, and the last answer is the newest's
    const slow = api.searchDrawn().length;
    for (const text of ["m", "me", "met", "meth", "metho", "method"]) {
      api.setSearch(text);
      await sleep(120);
    }
    await drawn("the answer for `method`", slow, (d) => d.query === "method");
    await sleep(1000);
    assert.strictEqual(api.searchDrawn().at(-1)!.query, "method", "nothing drawn after the newest query's answer");
    api.setSearch("");
  });

  test("the tree changes while the box has text: the rows are asked for again", async () => {
    await showSidebar();
    const probe = path.join(folder(), "zebra_probe.py");
    const from = api.searchDrawn().length;
    api.setSearch("zebra_search_probe");
    const empty = await drawn("an answer for the probe's name", from, (d) => d.query === "zebra_search_probe");
    assert.strictEqual(empty.total, 0, "no node has that name yet");
    try {
      fs.writeFileSync(probe, "def zebra_search_probe():\n    pass\n");
      const found = await drawn("the probe's def among the rows", from, (d) => d.query === "zebra_search_probe" && d.ids.some((id) => id.endsWith("zebra_probe.py::zebra_search_probe")));
      assert.strictEqual(found.total, 1);
    } finally {
      fs.rmSync(probe, { force: true });
    }
    // the later suites must see the fixture as it was: the core has read the deletion, and the box asked again
    await waitFor("the core to read the deletion", async () => !(await nodesOf()).some((n) => n.name === "zebra_probe.py"), 30_000);
    await until("the box to ask again after the deletion and draw no rows", () => api.searchDrawn().slice(from).filter((d) => d.query === "zebra_search_probe").at(-1)?.total === 0, 30_000);
    api.setSearch("");
  });

  test("*V-NOC: Search…* with a starting query: the cursor is in the sidebar's box with that text, its rows are the core's in the core's order, the first is pointed at, and Esc clears both", async () => {
    await showSidebar();
    api.setSearch(""); // the box starts empty: it must not point at a row
    await until("the box to point at nothing", () => api.session().highlight === null);
    const methods = (await nodesOf()).filter((n) => n.qualname === "Outer.method").map((n) => n.id);
    const focusedBefore = api.searchFocused().length;
    const from = api.searchDrawn().length;
    await vscode.commands.executeCommand("vnoc.search", "outer.meth");
    await until("the page to say it handled the command", () => api.searchFocused().length > focusedBefore, 30_000);
    assert.deepStrictEqual(api.searchFocused().at(-1), { focused: true, text: "outer.meth" }, "the cursor is in the box, and the box holds the starting query");
    const got = await drawn("the answer for the starting query", from, (d) => d.query === "outer.meth");
    assert.deepStrictEqual(got.ids, methods);
    await until("the first row to be pointed at", () => api.session().highlight === methods[0]);
    api.searchKey("Escape");
    await until("the pointer to clear", () => api.session().highlight === null);

    // 05 §2's fallback was taken because a QuickPick re-sorts what it is given (the check that failed: `e`, every row walked: VS Code showed `decorated deepest inner method …` where the core
    // says `inner Outer method …`). The box draws what the core ranked, so the same walk holds here: first that `e` can tell (the core's order is not the labels sorted), then every row in order.
    const rows = (await api.request<SearchResult>("vnoc/search", { query: "e" })).nodes;
    const labels = rows.map((n) => n.name);
    const sorted = [...labels].sort((a, b) => a.localeCompare(b));
    console.log(`[search] order on \`e\`, ${rows.length} rows\n  core:   ${labels.join(" ")}\n  sorted: ${sorted.join(" ")}`);
    assert.ok(rows.length >= 10, `enough rows to tell an order (${rows.length})`);
    assert.notDeepStrictEqual(labels, sorted, "the core's order is the labels' sorted order: this query cannot tell, pick another");
    const fromE = api.searchDrawn().length;
    await vscode.commands.executeCommand("vnoc.search", "e");
    const drawnE = await drawn("the answer for `e`", fromE, (d) => d.query === "e");
    assert.deepStrictEqual(drawnE.ids, rows.map((n) => n.id), "the rows are drawn in the core's order");
    await until(`the first row to be ${rows[0].name}`, () => api.session().highlight === rows[0].id);
    for (let i = 1; i < rows.length; i++) {
      api.searchKey("ArrowDown");
      await until(`row ${i} to be ${rows[i].id} (${rows[i].name})`, () => api.session().highlight === rows[i].id, 5_000);
    }
    api.searchKey("Escape");
    await until("the pointer to clear", () => api.session().highlight === null);
  });

  test("`kind:method` draws exactly the fixture's four methods, in reading order, and no note", async () => {
    await showSidebar();
    // a method is a function whose parent is a class; the tree's depth-first order is the reading order
    const methods: string[] = [];
    const walk = (n: TreeNode, parent: TreeNode | null) => {
      if (n.kind === "function" && parent?.kind === "class") methods.push(n.id);
      n.children.forEach((c) => walk(c, n));
    };
    (await api.structure()).root.children.forEach((c) => walk(c, null));
    assert.strictEqual(methods.length, 4, `the fixture defines four methods (${methods.join(", ")})`);
    const from = api.searchDrawn().length;
    api.setSearch("kind:method");
    const got = await drawn("the answer for `kind:method`", from, (d) => d.query === "kind:method");
    assert.deepStrictEqual(got.ids, methods);
    assert.strictEqual(got.total, 4);
    assert.deepStrictEqual(got.notes, []);
    api.setSearch("");
  });

  test("`kind:fnn` draws no rows, and its note", async () => {
    await showSidebar();
    const from = api.searchDrawn().length;
    api.setSearch("kind:fnn");
    const got = await drawn("the answer for `kind:fnn`", from, (d) => d.query === "kind:fnn");
    assert.deepStrictEqual(got.ids, []);
    assert.strictEqual(got.total, 0);
    assert.deepStrictEqual(got.notes, ["`kind:fnn` is not a kind (folder, file, class, function, method)"]);
    api.setSearch("");
  });

  test("[measure] the box: from the change of the text to the drawn rows, and from the request", () => {
    const own = api.searchDrawn().slice(drawnAtStart);
    console.log(`[search] view visible: ${api.sidebarVisible()}; answers: ${own.map((d) => `${d.query} ${d.ms.toFixed(0)}/${d.trip.toFixed(0)}`).join(", ")}`);
    assert.ok(own.length >= 8, `the suite drew ${own.length} answers`);
    const ms = own.map((d) => d.ms);
    const trip = own.map((d) => d.trip);
    const fixed = (v: number) => v.toFixed(1);
    console.log(`[measure] search, ${own.length} answers: key to drawn (ms) median ${fixed(median(ms))} worst ${fixed(Math.max(...ms))}; request to drawn (trip) median ${fixed(median(trip))} worst ${fixed(Math.max(...trip))}`);
    assert.deepStrictEqual(api.webviewErrors(), []);
  });
});
