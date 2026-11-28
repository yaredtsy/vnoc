import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";
import * as vscode from "vscode";

import { callNodesIn } from "@vnoc/shared/calls.ts";
import type { CallTreeResult, SearchResult, StructureResult, TreeNode } from "@vnoc/shared/messages";
import type { Session } from "@vnoc/shared/session.ts";
import { MISSING_PIN } from "@vnoc/shared/tree";
import { canvasTabs, closeCanvas, guarded, sleep, soft, until, waitFor } from "./util";

// Search in a real window (docs/20-search/06-build-order.md S3): the STRUCTURE view's box asks the core and draws what it answers, and *V-NOC: Search…* puts the cursor in that box (a QuickPick re-sorted the core's order: S3b).
// The suite cannot type in a webview, so `setSearch` puts text in the box as typing would, and the page says what it drew (`searchDrawn`). What a query finds, and in what
// order, is the core's own test (`crates/vnoc/tests/lifecycle.rs`); here it is that the answer reaches the screen, and that only the newest one does.

interface Drawn {
  query: string;
  ids: string[];
  docs: string[];
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
  searchDim(): { query: string; dimmed: string[] }[];
  renderedPosts(): { context: string; visible: number; callCards: number }[];
  dispatch(intent: { type: "expand"; ids: string[] } | { type: "select"; id: string | null } | { type: "focus"; id: string }): void;
  callTrees(): Record<string, CallTreeResult>;
  docsShown(): { pinned: string | null; node: string | null; doc: string | null }[];
  sidebarOpen(): boolean;
  sidebarVisible(): boolean;
  webviewErrors(): string[];
}
// the hooks of this file wait for the core: when it has stopped answering they stand aside (`soft`), and each test fails at once (nothing is skipped)
const suiteSetup = (fn: (this: Mocha.Context) => unknown) => globalThis.suiteSetup(soft(fn));

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
    api = guarded(await vscode.extensions.getExtension("vnoc.vnoc")!.activate());
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

  test("typing fast, and at a person's pace: every key is asked for, and the last answer drawn is the last text's", async () => {
    await showSidebar();
    const from = api.searchDrawn().length;
    for (const text of ["o", "ou", "out", "oute", "outer"]) api.setSearch(text); // one after another, no waiting
    await drawn("the answer for `outer`", from, (d) => d.query === "outer");
    await sleep(1000);
    const after = api.searchDrawn().slice(from);
    assert.strictEqual(after.at(-1)!.query, "outer", `the last answer drawn is for the last text (drawn: ${after.map((d) => d.query).join(", ")})`);
    // a pause of 120 ms between keys, as a person types: each key is asked for at once, and the last answer drawn is the newest's (that an older answer is dropped is `newest`'s unit test)
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

  test("`kind:method` draws exactly the fixture's methods, in reading order, and no note", async () => {
    await showSidebar();
    // a method is a function whose parent is a class; the tree's depth-first order is the reading order
    const methods: string[] = [];
    const walk = (n: TreeNode, parent: TreeNode | null) => {
      if (n.kind === "function" && parent?.kind === "class") methods.push(n.id);
      n.children.forEach((c) => walk(c, n));
    };
    (await api.structure()).root.children.forEach((c) => walk(c, null));
    // the `calls` suite leaves `calls_cls.py` (`Widget.__init__`) in the fixture, so a full run has five: the rows are checked against the structure's own methods
    assert.ok(methods.length >= 4, `the fixture defines at least four methods (${methods.join(", ")})`);
    const from = api.searchDrawn().length;
    api.setSearch("kind:method");
    const got = await drawn("the answer for `kind:method`", from, (d) => d.query === "kind:method");
    assert.deepStrictEqual(got.ids, methods);
    assert.strictEqual(got.total, methods.length);
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
  // Docs in the results (docs/20-search/06-build-order.md S7). The fixture gets three docs on `nested.py::decorated` (one attached, two written beside it) and a node that is Missing with one
  // doc; the words in them are found nowhere in the code, so a query for one draws no node row.
  suite("docs", () => {
    const NODE = "nested.py::decorated";
    const GONE = "quokka_gone.py::quokka_gone";
    let nodeId = "";
    let goneId = "";
    let docDir = "";
    let goneDir = "";
    let madeVnoc = false;
    let vnoc = "";
    let docSecond = "";
    let docThird = "";
    let docGone = "";

    const docsTabs = () => vscode.window.tabGroups.all.flatMap((g) => g.tabs).filter((t) => t.input instanceof vscode.TabInputWebview && t.input.viewType.endsWith("vnoc.docs"));
    const closeDocsTab = async () => {
      await vscode.window.tabGroups.close(docsTabs());
      await until("the docs tab to close", () => docsTabs().length === 0);
    };
    const write = (file: string, id: string, title: string, body: string) => fs.writeFileSync(file, `---\nnode: ${id}\nhint: x\n---\n# ${title}\n\n${body}\n`);
    /** the core's own answer, for what the page drew */
    const asked = (query: string) => api.request<SearchResult>("vnoc/search", { query });
    /** waits until the core finds a doc for `query` (a new file is read a moment after it is written) */
    const docFound = (query: string, doc: string) => waitFor(`the core to find ${doc} for ${query}`, async () => (await asked(query)).docs.some((d) => d.doc_path === doc), 30_000);

    suiteSetup(async function () {
      this.timeout(120_000);
      const structure = await api.structure();
      vnoc = path.join(structure.git_root, ".vnoc");
      madeVnoc = !fs.existsSync(vnoc);
      const attach = async (key: string, file: string, title: string) => {
        const r = await api.request<{ id: string }>("vnoc/attach", { key, title, markers: false, document: { uri: vscode.Uri.file(file).toString(), version: 1, text: fs.readFileSync(file, "utf8") } });
        const docs = await api.request<{ path: string }[]>("vnoc/docs", { id: r.id });
        return { id: r.id, dir: path.dirname(docs[0].path), doc: docs[0].path };
      };
      const nested = path.join(folder(), "nested.py");
      const first = await attach(NODE, nested, "Why decorated");
      nodeId = first.id;
      docDir = first.dir;
      // two more docs on the node, named to come after the attached one: a doc chosen by the pick is not the first
      docThird = path.join(docDir, "yyy-third.md");
      docSecond = path.join(docDir, "zzz-second.md");
      write(first.doc, nodeId, "Why decorated", "Nothing special here.");
      write(docThird, nodeId, "Third notes", "A word only here: quokkayak.");
      write(docSecond, nodeId, "Second notes", "A word only here: quokkazebra, and the outer shell.");
      // a node whose code is deleted after its doc is attached: Missing, with its doc kept
      const gone = path.join(folder(), "quokka_gone.py");
      fs.writeFileSync(gone, "def quokka_gone():\n    pass\n");
      await waitFor("quokka_gone.py in the structure", async () => (await nodesOf()).some((n) => n.id === GONE), 30_000);
      const g = await attach(GONE, gone, "Where it went");
      goneId = g.id;
      goneDir = g.dir;
      docGone = g.doc;
      write(docGone, goneId, "Where it went", "The wombatzebra left no code.");
      fs.rmSync(gone);
      await waitFor("the node to be Missing", async () => (await api.structure()).missing.some((m: { id: string }) => m.id === goneId), 30_000);
      await docFound("quokkazebra", docSecond);
      await docFound("quokkayak", docThird);
      await docFound("wombatzebra", docGone);
    });

    suiteTeardown(async function () {
      this.timeout(60_000);
      api.setSearch("");
      await closeDocsTab();
      await vscode.commands.executeCommand("workbench.action.closeAllEditors");
      fs.rmSync(path.join(folder(), "quokka_gone.py"), { force: true });
      fs.rmSync(madeVnoc ? vnoc : goneDir, { recursive: true, force: true });
      if (!madeVnoc) fs.rmSync(docDir, { recursive: true, force: true });
      // the core has read it, as `indexChanged` says: the node is not Missing, and the fixture's node has no docs, as the later suites expect
      await waitFor("the core to read the removal", async () => {
        const s = await api.structure();
        return !s.missing.some((m: { id: string }) => m.id === goneId) && (await nodesOf()).find((n) => n.id === NODE)?.docs === 0;
      }, 30_000);
    });

    /** the answer for `text`, drawn: it has been asked for in the box */
    async function searched(text: string): Promise<Drawn> {
      await showSidebar();
      const from = api.searchDrawn().length;
      api.setSearch(text);
      return drawn(`the answer for ${text}`, from, (d) => d.query === text);
    }

    test("a word only a doc holds finds the doc: no node row, one doc row, and the row's node is pointed at", async () => {
      const got = await searched("quokkazebra");
      assert.deepStrictEqual(got.ids, [], "the word is in no name");
      assert.deepStrictEqual(got.docs, [docSecond]);
      const core = await asked("quokkazebra");
      assert.deepStrictEqual(got.docs, core.docs.map((d) => d.doc_path), "what is drawn is what the core says");
      assert.strictEqual(core.docs[0].node, NODE);
      assert.strictEqual(core.docs[0].missing, false);
      await until("the doc's node to be pointed at", () => api.session().highlight === NODE);
      api.searchKey("Escape");
      await until("the pointer to clear", () => api.session().highlight === null);
    });

    test("picking a doc reveals its node and shows that doc in the docs tab, pinned: in a tab the pick opens, and in one that is open", async () => {
      await closeDocsTab();
      const from = api.docsShown().length;
      await searched("quokkazebra");
      await until("the doc's node to be pointed at", () => api.session().highlight === NODE);
      api.searchKey("Enter");
      await until("the node to be selected", () => api.session().contexts.main.selected === NODE);
      await until("the docs tab", () => docsTabs().length === 1);
      await until("the tab to show the picked doc, not the node's first", () => api.docsShown().slice(from).some((d) => d.doc === docSecond));
      assert.deepStrictEqual(api.docsShown().at(-1), { pinned: NODE, node: NODE, doc: docSecond }, "pinned to the node, showing the doc the pick chose");
      await until("the box to clear", () => api.session().highlight === null);

      // the tab is open now: another doc of the same node reaches it as an event
      const open = api.docsShown().length;
      await searched("quokkayak");
      await until("the doc's node to be pointed at", () => api.session().highlight === NODE);
      api.searchKey("Enter");
      await until("the open tab to show the other doc", () => api.docsShown().slice(open).some((d) => d.doc === docThird));
      assert.deepStrictEqual(api.docsShown().at(-1), { pinned: NODE, node: NODE, doc: docThird });
      assert.strictEqual(docsTabs().length, 1, "the same tab");
      await sleep(500);
      assert.deepStrictEqual(api.webviewErrors(), []);
      await closeDocsTab();
    });

    test("a Missing node's doc is marked, points at nothing, and its pick pins the tab to the Missing node with that doc", async () => {
      await closeDocsTab();
      const from = api.docsShown().length;
      const got = await searched("wombatzebra");
      assert.deepStrictEqual(got.docs, [docGone]);
      const row = (await asked("wombatzebra")).docs[0];
      assert.deepStrictEqual([row.missing, row.node, row.node_id], [true, null, goneId]);
      assert.strictEqual(api.session().highlight, null, "a node that is not in the tree is not pointed at");
      api.searchKey("Enter");
      await until("the docs tab", () => docsTabs().length === 1);
      await until("the tab to show the doc of the Missing node", () => api.docsShown().slice(from).some((d) => d.doc === docGone));
      assert.deepStrictEqual(api.docsShown().at(-1), { pinned: MISSING_PIN + goneId, node: MISSING_PIN + goneId, doc: docGone });
      await sleep(500);
      assert.deepStrictEqual(api.webviewErrors(), []);
      await closeDocsTab();
    });

    test("a doc whose node is neither in the tree nor Missing: its pick opens the .md in an editor tab, with no docs tab", async () => {
      await closeDocsTab();
      await vscode.commands.executeCommand("workbench.action.closeAllEditors");
      // a record for a file that exists but that no driver reads (a .txt): its code is not read, and it is not gone, so the node is in neither list
      const unreadId = "0000000000000000000000000V";
      const unreadDir = path.join(vnoc, "nodes", unreadId.slice(24), unreadId);
      const unreadFile = path.join(folder(), "quokka_unread.txt");
      const unreadDoc = path.join(unreadDir, "unread.md");
      try {
        fs.writeFileSync(unreadFile, "not code\n");
        fs.mkdirSync(unreadDir, { recursive: true });
        fs.writeFileSync(path.join(unreadDir, "node.toml"), 'path = "quokka_unread.txt"\nqualname = ""\nid_in_code = "no"\nkind = "file"\n');
        write(unreadDoc, unreadId, "Unread notes", "The word quokkaunread.");
        await docFound("quokkaunread", unreadDoc);
        const row = (await asked("quokkaunread")).docs[0];
        assert.deepStrictEqual([row.missing, row.node, row.node_id], [false, null, unreadId], "neither in the tree nor Missing");
        const got = await searched("quokkaunread");
        assert.deepStrictEqual(got.docs, [unreadDoc]);
        api.searchKey("Enter");
        await until("the .md to open in an editor", () => vscode.window.activeTextEditor?.document.uri.fsPath === unreadDoc, 20_000);
        assert.strictEqual(docsTabs().length, 0, "no docs tab: there is no node to pin it to");
        await sleep(500);
        assert.deepStrictEqual(api.webviewErrors(), []);
      } finally {
        await vscode.commands.executeCommand("workbench.action.closeAllEditors");
        fs.rmSync(unreadDir, { recursive: true, force: true });
        fs.rmSync(unreadFile, { force: true });
      }
    });

    test("↓ walks from the last node row to the first doc row", async () => {
      const got = await searched("outer");
      const core = await asked("outer");
      assert.ok(core.nodes.length >= 2 && core.docs.length >= 1, `a query that finds both (${core.nodes.length} nodes, ${core.docs.length} docs)`);
      assert.deepStrictEqual(got.ids, core.nodes.map((n) => n.id));
      assert.deepStrictEqual(got.docs, core.docs.map((d) => d.doc_path));
      const last = core.nodes.length - 1;
      assert.ok(core.docs[0].node !== null && core.docs[0].node !== core.nodes[last].id, "the doc's node is not the last node row's, so the pointer tells them apart");
      await until("the first row to be pointed at", () => api.session().highlight === core.nodes[0].id);
      for (let i = 1; i <= last; i++) {
        api.searchKey("ArrowDown");
        await until(`node row ${i}`, () => api.session().highlight === core.nodes[i].id, 5_000);
      }
      api.searchKey("ArrowDown");
      await until("the first doc row to be active", () => api.session().highlight === core.docs[0].node, 5_000);
      api.searchKey("Escape");
      await until("the pointer to clear", () => api.session().highlight === null);
    });
    test("the canvas dims the cards the query does not find, never the selected one, and not a card whose doc is the hit; an empty box undims at once", async () => {
      await showSidebar();
      api.setSearch("");
      await vscode.commands.executeCommand("vnoc.openCanvas");
      await until("the canvas tab", () => canvasTabs().length === 1);
      const drawnBefore = api.renderedPosts().length;
      api.dispatch({ type: "expand", ids: ["nested.py", "nested.py::Outer"] });
      await until("the canvas to draw the open file and class", () => api.renderedPosts().length > drawnBefore && (api.renderedPosts().at(-1)?.visible ?? 0) >= 8, 30_000);
      const methods = (await nodesOf()).filter((n) => n.qualname === "Outer.method").map((n) => n.id);
      const OUTER = "nested.py::Outer";
      const lastFor = (query: string) => api.searchDim().filter((d) => d.query === query).at(-1);
      try {
        // a query whose hits are two cards: they are bright, the others dim, and the class that holds them is not a hit
        api.setSearch("outer.meth");
        await until("the dim for `outer.meth`", () => lastFor("outer.meth") !== undefined, 20_000);
        const core = await api.request<{ hits: string[] }>("vnoc/searchCards", { query: "outer.meth", ids: (await nodesOf()).map((n) => n.id) });
        assert.deepStrictEqual(core.hits, methods, "the core's own answer");
        let dimmed = lastFor("outer.meth")!.dimmed;
        assert.ok(dimmed.length >= 4, `the cards that are not hits are dimmed (${dimmed.join(", ")})`);
        assert.ok(dimmed.includes(OUTER), "the class is not a hit");
        assert.deepStrictEqual(dimmed.filter((id) => core.hits.includes(id)), [], "no hit card is dimmed");

        // the selected card is never dimmed, hit or not
        api.dispatch({ type: "select", id: OUTER });
        await until("the selected class to leave the dimmed list", () => !lastFor("outer.meth")!.dimmed.includes(OUTER), 20_000);
        dimmed = lastFor("outer.meth")!.dimmed;
        assert.ok(dimmed.length >= 3 && !dimmed.includes(OUTER), "the others stay dimmed");

        // a card whose doc is the hit: `decorated` holds the word only in a doc
        api.setSearch("quokkazebra");
        await until("the dim for the doc's word", () => lastFor("quokkazebra") !== undefined, 20_000);
        dimmed = lastFor("quokkazebra")!.dimmed;
        assert.ok(!dimmed.includes(NODE), "the card of the doc that holds the word is bright");
        assert.ok(dimmed.includes("nested.py"), `the file card is dimmed (${dimmed.join(", ")})`);

        // an empty box: everything is bright at once, said once
        const before = api.searchDim().length;
        api.setSearch("");
        await until("the dim to end", () => api.searchDim().length > before, 20_000);
        assert.deepStrictEqual(api.searchDim().at(-1), { query: "", dimmed: [] });
        assert.deepStrictEqual(api.webviewErrors(), []);
      } finally {
        api.setSearch("");
        api.dispatch({ type: "select", id: null });
        await closeCanvas();
      }
    });
  });

  suite("a call view", () => {
    // `entry` calls `pick_wrap_fn(pick_helper_fn(1))` and then `pick_helper_fn(2)`: the callee is called twice, one call inside another call's arguments
    const LIB = "def pick_helper_fn(x):\n    return x\n\n\ndef pick_wrap_fn(x):\n    return x\n\n\ndef pick_unused_fn():\n    return 0\n";
    const APP = "from pick_lib import pick_helper_fn, pick_wrap_fn\n\n\ndef pick_entry_fn():\n    pick_wrap_fn(pick_helper_fn(1))\n    pick_helper_fn(2)\n";
    const ENTRY = "pick_app.py::pick_entry_fn";
    const HELPER = "pick_lib.py::pick_helper_fn";
    const UNUSED = "pick_lib.py::pick_unused_fn";
    const files = () => ["pick_lib.py", "pick_app.py"].map((f) => path.join(folder(), f));
    const focusOf = () => [...api.session().contexts.main.focus];
    const lastFor = (query: string) => api.searchDim().filter((d) => d.query === query).at(-1);

    test("typing the callee's name keeps its call card bright; picking it selects that card and the focus stays; a node the view does not draw is revealed as before", async function () {
      this.timeout(180_000);
      await showSidebar();
      api.setSearch("");
      fs.writeFileSync(files()[0], LIB);
      fs.writeFileSync(files()[1], APP);
      try {
        await waitFor("the fixture in the structure", async () => (await nodesOf()).some((n) => n.id === ENTRY) && (await nodesOf()).some((n) => n.id === UNUSED), 30_000);
        await vscode.commands.executeCommand("vnoc.openCanvas");
        await until("the canvas tab", () => canvasTabs().length === 1);
        // the call view of `entry`: it is the focus, and open, so its calls are drawn as call cards
        api.dispatch({ type: "focus", id: ENTRY });
        api.dispatch({ type: "expand", ids: [ENTRY] });
        await until("its call tree", () => api.callTrees()[ENTRY] !== undefined, 30_000);
        const calls = callNodesIn(ENTRY, api.callTrees()[ENTRY].root);
        const helper = calls.filter((c) => c.node.key === HELPER);
        assert.strictEqual(helper.length, 1, `the callee called twice is one call card (${calls.map((c) => c.node.name).join(", ")})`);
        assert.strictEqual(helper[0].node.call_count, 2, "two calls: one inside the arguments of the other call");
        const card = helper[0].id;
        const wrap = calls.find((c) => c.node.name === "pick_wrap_fn")!.id;
        await until("the call cards to be drawn", () => api.renderedPosts().some((r) => r.callCards >= 2), 30_000);

        // typing its name: its call card is bright, the other is dim
        api.setSearch("pick_helper_fn");
        await until("the dim for the callee's name", () => lastFor("pick_helper_fn") !== undefined, 20_000);
        const dimmed = lastFor("pick_helper_fn")!.dimmed;
        assert.ok(!dimmed.includes(card), `the callee's call card is bright (dimmed: ${dimmed.join(", ")})`);
        assert.ok(dimmed.includes(wrap), "the other call card is dim");

        // picking it (Enter on the first row): that card is selected and the view's focus is as it was
        const focus = focusOf();
        assert.ok(focus.length > 0 && focus.at(-1) === ENTRY, `the view is focused on the entry (${focus.join(" > ")})`);
        const rows = await drawn("the row for the callee", 0, (d) => d.query === "pick_helper_fn" && d.ids.includes(HELPER));
        for (let i = rows.ids.indexOf(HELPER); i > 0; i--) api.searchKey("ArrowDown");
        await until("the callee's row to be pointed at", () => api.session().highlight === HELPER, 5_000);
        api.searchKey("Enter");
        await until("the call card to be selected", () => api.session().contexts.main.selected === card, 20_000);
        await until("the request to be done", () => api.session().reveal === null, 20_000);
        assert.deepStrictEqual(focusOf(), focus, "the focus did not shorten to hold the callee's file");

        // a node the view draws no call card for: revealed by structure, the focus shortens to hold it
        await showSidebar();
        api.setSearch("pick_unused_fn");
        const again = await drawn("the row for the node that is not drawn", api.searchDrawn().length, (d) => d.query === "pick_unused_fn" && d.ids.includes(UNUSED));
        for (let i = again.ids.indexOf(UNUSED); i > 0; i--) api.searchKey("ArrowDown");
        await until("the unused row to be pointed at", () => api.session().highlight === UNUSED, 5_000);
        api.searchKey("Enter");
        await until("the node to be selected", () => api.session().contexts.main.selected === UNUSED, 20_000);
        assert.notDeepStrictEqual(focusOf(), focus, "the focus shortened to hold it");
        assert.deepStrictEqual(api.webviewErrors(), []);
      } finally {
        api.setSearch("");
        api.dispatch({ type: "select", id: null });
        await closeCanvas();
        for (const f of files()) fs.rmSync(f, { force: true });
      }
    });
  });
});
