import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";
import * as vscode from "vscode";

import type { SearchResult, StructureResult, TreeNode } from "@vnoc/shared/messages";
import type { Session } from "@vnoc/shared/session.ts";
import { sleep, until, waitFor } from "./util";

// Search in a real window (docs/20-search/06-build-order.md S3): the STRUCTURE view's box and the palette's *V-NOC: Search…* both ask the core, and draw what it answers.
// The suite cannot type in a webview, so `setSearch` puts text in the box as typing would, and the page says what it drew (`searchDrawn`). What a query finds, and in what
// order, is the core's own test (`crates/vnoc/tests/lifecycle.rs`); here it is that the answer reaches the screen, and that only the newest one does.

interface Drawn {
  query: string;
  ids: string[];
  total: number;
  ms: number;
  trip: number;
}
interface Api {
  structure(): Promise<StructureResult>;
  request<T>(method: string, params?: object): Promise<T>;
  session(): Session;
  searchDrawn(): Drawn[];
  setSearch(text: string): void;
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

suite("search", () => {
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

  test("the palette: *V-NOC: Search…* with a starting query points at the core's first item, in the core's order, and closing it clears the pointer", async () => {
    await showSidebar();
    api.setSearch(""); // the box has no text: it must not point at a row itself
    await until("the box to point at nothing", () => api.session().highlight === null);
    const methods = (await nodesOf()).filter((n) => n.qualname === "Outer.method").map((n) => n.id);
    await vscode.commands.executeCommand("vnoc.search", "outer.meth");
    await until("the first item to be pointed at", () => api.session().highlight !== null);
    assert.ok(methods.includes(api.session().highlight!), `the first item is an Outer.method (${api.session().highlight})`);
    await vscode.commands.executeCommand("workbench.action.closeQuickOpen");
    await until("the pointer to clear", () => api.session().highlight === null);

    // [check] 05 §2: VS Code keeps the order it was given. The query is `e`: every row's label holds it, so VS Code has a match to score on every item and a re-sort would show.
    // (`outer` could not tell: one label matched, the rest were found by their qualified name.) First, that this query *can* tell: the core's order is not the labels sorted.
    const rows = (await api.request<SearchResult>("vnoc/search", { query: "e" })).nodes;
    const labels = rows.map((n) => n.name);
    const sorted = [...labels].sort((a, b) => a.localeCompare(b));
    console.log(`[search] order check on \`e\`, ${rows.length} rows\n  core:   ${labels.join(" ")}\n  sorted: ${sorted.join(" ")}`);
    assert.ok(rows.length >= 10, `enough rows to tell an order (${rows.length})`);
    assert.notDeepStrictEqual(labels, sorted, "the core's order is the labels' sorted order: this query cannot tell a re-sort from the core's order, pick another");
    await vscode.commands.executeCommand("vnoc.search", "e");
    const nameOf = new Map(rows.map((n) => [n.id, n.name]));
    try {
      await until(`the first item to be ${rows[0].id} (${rows[0].name})`, () => api.session().highlight === rows[0].id, 5_000);
      for (let i = 1; i < rows.length; i++) {
        await vscode.commands.executeCommand("workbench.action.quickOpenSelectNext");
        await until(`item ${i} to be ${rows[i].id} (${rows[i].name})`, () => api.session().highlight === rows[i].id, 5_000);
      }
    } catch (e) {
      // the walk failed: say what VS Code shows (not part of the check, which has failed): the highlight now, then the next items as it walks them
      const shown: string[] = [];
      for (let i = 0; i < rows.length; i++) {
        await sleep(200);
        const id = api.session().highlight;
        shown.push(id === null ? "(none)" : (nameOf.get(id) ?? id));
        await vscode.commands.executeCommand("workbench.action.quickOpenSelectNext");
      }
      throw new Error(`${(e as Error).message}. The core's order: ${labels.join(" ")}. What VS Code highlights, walking from the top: ${shown.join(" ")}`);
    } finally {
      await vscode.commands.executeCommand("workbench.action.closeQuickOpen");
    }
    await until("the pointer to clear", () => api.session().highlight === null);
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
