import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";
import * as vscode from "vscode";

import { callNodesIn } from "@vnoc/shared/calls.ts";
import type { CallTreeResult, StructureResult, TreeNode } from "@vnoc/shared/messages";
import { type Intent, type Session, exploreId, openOf } from "@vnoc/shared/session.ts";
import { canvasTabs, closeCanvas, guarded, sleep, until, waitFor } from "./util";

// The STRUCTURE view in a real window (docs/16-ui-redesign/06-sidebar.md, 07 §4): it is a webview, so the suite drives it through what it says (the
// `sidebarRendered` note) and through the host's side of a click (`rowClick`). What a key or a click means is unit-tested beside the page.

interface Api {
  structure(): Promise<StructureResult>;
  session(): Session;
  dispatch(intent: Intent, context?: string): void;
  rowClick(context: string, id: string): Promise<void>;
  sidebarPosts(): { context: string; rows: number; selected: string | null; inView: boolean; drawn: boolean; height: number }[];
  callTrees(): Record<string, CallTreeResult>;
  sidebarOpen(): boolean;
  readingStarts(): number;
  emitIndexChanged(): void;
  searchKey(key: string): void;
  renderedPosts(): { context: string; visible: number }[];
  webviewErrors(): string[];
}
let api: Api;

/** Waits until the index has stopped changing: the same revision for a second. */
async function structureSettled(): Promise<void> {
  let revision = -1;
  let since = Date.now();
  await waitFor("the structure to stop changing", async () => {
    const now = (await api.structure()).revision;
    if (now !== revision) (revision = now), (since = Date.now());
    return Date.now() - since >= 1000;
  }, 30_000);
}

/** What changed where the keyboard and the pointer could be taken or cleared, with times: a failure that names its cause is worth more than a timeout. */
function recordTimeline(highlight: () => string | null): { log: string[]; stop: () => void } {
  const t0 = Date.now();
  const log: string[] = [];
  const note = (what: string) => log.push(`+${Date.now() - t0} ${what}`);
  let last = highlight();
  const poll = setInterval(() => {
    const now = highlight();
    if (now !== last) (last = now), note(`highlight ${now === null ? "cleared" : "→ " + now}`);
  }, 20);
  const subs = [
    vscode.window.onDidChangeWindowState((e) => note(`window focused=${e.focused}`)),
    vscode.window.onDidChangeActiveTextEditor((e) => note(`active text editor ${e?.document.uri.path.split("/").pop() ?? "none"}`)),
    vscode.window.tabGroups.onDidChangeTabs((e) => note(`tabs +${e.opened.length} -${e.closed.length} ~${e.changed.length}`)),
    vscode.window.tabGroups.onDidChangeTabGroups(() => note("tab groups changed")),
  ];
  return { log, stop: () => (clearInterval(poll), subs.forEach((s) => s.dispose())) };
}

const textTabs = () => vscode.window.tabGroups.all.flatMap((g) => g.tabs).filter((t) => t.input instanceof vscode.TabInputText);
/** What a panel last said it drew (MAIN's unless a context is named): every panel reports on its own. */
const lastPost = (context = "main") => api.sidebarPosts().filter((p) => p.context === context).at(-1);
/** The page says this node's row is selected in that panel, the list is scrolled to it (`inView`, read from the element) and the virtualizer has put it in the page (`drawn`): three facts, not one. */
const shown = (id: string, context = "main"): boolean => lastPost(context)?.selected === id && lastPost(context)?.inView === true && lastPost(context)?.drawn === true;

suiteSetup(async () => {
  api = guarded(await vscode.extensions.getExtension("vnoc.vnoc")!.activate());
});

suite("the commands", () => {
  test("every command package.json declares is registered by the extension", async () => {
    const declared: string[] = vscode.extensions.getExtension("vnoc.vnoc")!.packageJSON.contributes.commands.map((c: { command: string }) => c.command);
    const registered = new Set(await vscode.commands.getCommands(true));
    assert.deepStrictEqual(declared.filter((c) => !registered.has(c)), [], "declared in package.json, but nothing registers them");
  });
});

suite("the sidebar", () => {
  let first: TreeNode;
  let firstFile: TreeNode;
  let other: TreeNode;
  const folderPath = vscode.workspace.workspaceFolders![0].uri.fsPath;
  const many = path.join(folderPath, "sidebar_many.py");
  const callFiles = [path.join(folderPath, "sidebar_calls_lib.py"), path.join(folderPath, "sidebar_calls_app.py")];
  let caller: TreeNode;
  let single: TreeNode;

  suiteSetup(async () => {
    // a second file, so a preview tab can be seen being replaced; and a long one, so a row can be below the fold
    const folder = vscode.workspace.workspaceFolders![0].uri.fsPath;
    fs.writeFileSync(path.join(folder, "other.py"), "def other():\n    pass\n");
    fs.writeFileSync(many, Array.from({ length: 120 }, (_, i) => `def many${i}():\n    return ${i}\n`).join("\n\n"));
    fs.writeFileSync(callFiles[0], "def sleaf():\n    return 1\n");
    // smain calls sleaf in two places (one merged call with two sites); sone in one (lines 4, 5 and 9, 0-based)
    fs.writeFileSync(callFiles[1], "from sidebar_calls_lib import sleaf\n\n\ndef smain():\n    sleaf()\n    sleaf()\n\n\ndef sone():\n    sleaf()\n");
    let files: TreeNode[] = [];
    for (let waited = 0; !(files.length >= 5 && files.find((f) => f.name === "sidebar_many.py")?.children.length === 120 && files.find((f) => f.name === "sidebar_calls_app.py")?.children.length === 2); waited += 200) {
      assert.ok(waited < 30_000, "the new files never reached the structure");
      files = (await api.structure()).root.children.filter((n) => n.kind === "file");
      await sleep(200);
    }
    firstFile = files.find((f) => f.name === "nested.py")!;
    first = firstFile.children[0];
    other = files.find((f) => f.name === "other.py")!.children[0];
    [caller, single] = files.find((f) => f.name === "sidebar_calls_app.py")!.children;
  });

  suiteTeardown(async () => {
    fs.rmSync(many, { force: true });
    callFiles.forEach((f) => fs.rmSync(f, { force: true }));
    api.dispatch({ type: "focusJump", depth: 0 });
    api.dispatch({ type: "collapseAll" });
  });

  test("[check] vnoc.structure.focus brings the webview view up: its page is made and reports what it drew", async () => {
    assert.ok((await vscode.commands.getCommands(true)).includes("vnoc.structure.focus"), "VS Code makes `<viewId>.focus` for a webview view");
    await vscode.commands.executeCommand("vnoc.structure.focus");
    await until("the STRUCTURE page to exist", () => api.sidebarOpen(), 30_000);
    await until("the page to draw its rows", () => (lastPost()?.rows ?? 0) > 0, 30_000);
    assert.deepStrictEqual(api.webviewErrors(), []);
  });

  test("the first read of the code has VS Code's progress line on the view; a read after rows are drawn does not (06 §8)", async () => {
    const first = api.readingStarts();
    assert.ok(first >= 1, "the first read ran with the line on the view");
    const posts = api.sidebarPosts().length;
    api.emitIndexChanged(); // every page asks for the structure again, as after a save
    await sleep(1500);
    assert.strictEqual(api.readingStarts(), first, "a quiet refresh over drawn rows starts no line");
    assert.ok((lastPost()?.rows ?? 0) > 0 && api.sidebarPosts().length >= posts, "and the rows stayed");
  });

  test("with the canvas open, a click selects and reveals on the canvas, and opens no code tab", async () => {
    await vscode.commands.executeCommand("workbench.action.closeAllEditors");
    await vscode.commands.executeCommand("vnoc.openCanvas");
    await until("the canvas tab", () => canvasTabs().length === 1);
    await api.rowClick("main", first.id);
    await sleep(1000);
    assert.strictEqual(textTabs().length, 0, "no code tab was opened");
    assert.strictEqual(canvasTabs().length, 1);
    assert.strictEqual(api.session().contexts.main.selected, first.id);
    await until("the canvas to do the reveal", () => api.session().reveal === null);
  });

  test("with the canvas closed, a click opens one preview tab, and the next click replaces it", async () => {
    await closeCanvas();
    await vscode.commands.executeCommand("workbench.action.closeAllEditors");
    await api.rowClick("main", first.id);
    await until("a code tab", () => textTabs().length === 1);
    assert.ok(textTabs()[0].isPreview, "the tab is a preview tab");
    await api.rowClick("main", other.id);
    await until("the other file", () => textTabs().some((t) => (t.input as vscode.TabInputText).uri.fsPath.endsWith("other.py")));
    assert.strictEqual(textTabs().length, 1, "the preview tab was replaced, not added to");
  });

  test("`vnoc.open` is the same click, for the palette and for older callers", async () => {
    await closeCanvas();
    await vscode.commands.executeCommand("workbench.action.closeAllEditors");
    await vscode.commands.executeCommand("vnoc.open", first);
    await until("a code tab", () => textTabs().length === 1);
    assert.strictEqual(api.session().contexts.main.selected, first.id);
  });

  test("a selection made elsewhere (a card, a reveal) is reported by the page as the selected row, in view", async () => {
    await closeCanvas();
    await vscode.commands.executeCommand("workbench.action.closeAllEditors");
    api.dispatch({ type: "reveal", id: first.id, canvas: false }); // opens the ancestors and selects, as Reveal in Sidebar does
    await until("the page to name the revealed row, in view", () => shown(first.id), 30_000);
    api.dispatch({ type: "reveal", id: other.id, canvas: false });
    await until("the page to follow the selection", () => shown(other.id), 30_000);
    api.dispatch({ type: "select", id: first.id }); // what a click on a card sends: its row is there, the ancestors being open
    await until("the page to follow a plain select", () => shown(first.id), 30_000);
    assert.strictEqual(textTabs().length, 0, "a selection from elsewhere opens no code");
  });

  test("a selection far down a long list is scrolled into view by the page itself", async () => {
    const file = (await api.structure()).root.children.find((n) => n.name === "sidebar_many.py")!;
    const last = file.children[119];
    // the list can only scroll when the view is shown and laid out: bring it up, and wait for the page to say it has a height
    await vscode.commands.executeCommand("vnoc.structure.focus");
    await until("the list to have a height", () => (lastPost()?.height ?? 0) > 0, 30_000);
    api.dispatch({ type: "expand", ids: [file.id] }); // the file open: its 120 functions are rows
    await until("120 functions as rows", () => (lastPost()?.rows ?? 0) > 120, 30_000);
    api.dispatch({ type: "select", id: file.children[0].id });
    await until("the first function in view", () => shown(file.children[0].id), 30_000);
    api.dispatch({ type: "select", id: last.id });
    try {
      await until("the last function, 119 rows below, scrolled into view", () => shown(last.id), 30_000);
    } catch (e) {
      throw new Error(`${(e as Error).message}; wanted ${last.id}; the page's last reports: ${JSON.stringify(api.sidebarPosts().slice(-4))}; session selected ${api.session().contexts.main.selected}`);
    }
    assert.deepStrictEqual(api.webviewErrors(), []);
  });

  test("Reveal in Sidebar selects the node and the page shows it", async () => {
    api.dispatch({ type: "focusJump", depth: 0 });
    api.dispatch({ type: "collapseAll" });
    await vscode.commands.executeCommand("vnoc.revealInSidebar", { nodeKey: first.id, webviewSection: "node" }); // what a card's or a row's menu passes
    await until("the page to show it", () => shown(first.id), 30_000);
    assert.strictEqual(api.session().contexts.main.selected, first.id);
  });

  test("Focus from the menu (a row's context object) focuses the sidebar and the canvas on the node, and Back and Clear undo it", async () => {
    await vscode.commands.executeCommand("vnoc.focus", { nodeKey: firstFile.id, webviewSection: "node", webview: "vnoc.structure" });
    assert.deepStrictEqual([...api.session().contexts.main.focus].at(-1), firstFile.id);
    await until("the page to draw the focused node alone", () => (lastPost()?.rows ?? 99) <= firstFile.children.length + 1, 30_000);
    await vscode.commands.executeCommand("vnoc.focusBack");
    assert.deepStrictEqual([...api.session().contexts.main.focus], []);
  });

  test("with an EXPLORE context active, Reveal in Sidebar selects the row in the EXPLORE panel, drawn and in view: the gap of 8.5 is closed (06 §4)", async () => {
    await closeCanvas();
    api.dispatch({ type: "focusJump", depth: 0 });
    api.dispatch({ type: "collapseAll" });
    await vscode.commands.executeCommand("vnoc.structure.focus");
    await until("the list to have a height", () => (lastPost()?.height ?? 0) > 0, 30_000);
    api.dispatch({ type: "expand", ids: [caller.id] }); // an open function: the host fetches what it calls
    await until("the call tree of the function", () => api.callTrees()[caller.id] !== undefined, 30_000);
    const [call] = callNodesIn(caller.id, api.callTrees()[caller.id].root);
    const explore = exploreId(call.id);
    api.dispatch({ type: "explore", call: call.id, activate: true }); // a double-click on the call card
    try {
      await until("the EXPLORE context to be the active one", () => api.session().active === explore);
      await until("both panels to draw", () => (lastPost("main")?.rows ?? 0) > 0 && (lastPost(explore)?.rows ?? 0) > 0, 30_000);
      const mainSelected = api.session().contexts.main.selected;
      assert.strictEqual(mainSelected, call.id, "MAIN has the call selected: its panel has no row for it");
      await vscode.commands.executeCommand("vnoc.revealInSidebar", { nodeKey: first.id, webviewSection: "node" }); // a node the callee's focus hides
      await until("the EXPLORE panel to show it: selected, in view, drawn", () => shown(first.id, explore), 30_000);
      assert.strictEqual(api.session().contexts[explore].selected, first.id);
      assert.strictEqual(api.session().contexts.main.selected, mainSelected, "MAIN's selection did not move: the context that acted is the active one");
      assert.notStrictEqual(lastPost("main")?.selected, first.id, "and its panel does not show it");
    } catch (e) {
      throw new Error(`${(e as Error).message}; the panels' last reports: ${JSON.stringify(api.sidebarPosts().slice(-6))}; active ${api.session().active}`);
    } finally {
      api.dispatch({ type: "closeContext", id: explore });
      api.dispatch({ type: "select", id: null });
      api.dispatch({ type: "collapseAll" });
    }
  });

  test("the call menu's commands take what a call card's context gives (08 §3, §5)", async () => {
    await closeCanvas();
    await vscode.commands.executeCommand("workbench.action.closeAllEditors");
    api.dispatch({ type: "focusJump", depth: 0 });
    api.dispatch({ type: "collapseAll" });
    api.dispatch({ type: "expand", ids: [caller.id, single.id] }); // two open functions: the host fetches what they call
    await until("both call trees", () => api.callTrees()[caller.id] !== undefined && api.callTrees()[single.id] !== undefined, 30_000);
    const [several] = callNodesIn(caller.id, api.callTrees()[caller.id].root); // sleaf, called in two places
    const [once] = callNodesIn(single.id, api.callTrees()[single.id].root); // sleaf, called in one
    assert.deepStrictEqual([several.node.call_sites.length, once.node.call_sites.length], [2, 1]);
    const card = (id: string, over: object = {}) => ({ webviewSection: "node", role: "call", callId: id, nodeKey: "sidebar_calls_lib.py::sleaf", contextId: "main", surface: "canvas", ...over });
    const line = () => vscode.window.activeTextEditor?.selection.active.line;
    const inApp = () => vscode.window.activeTextEditor?.document.uri.fsPath.endsWith("sidebar_calls_app.py") === true;
    try {
      // Expand and Collapse are the call card's own, not its callee's
      await vscode.commands.executeCommand("vnoc.expand", card(several.id));
      assert.ok(openOf(api.session().contexts.main).includes(several.id), "the call is open");
      assert.ok(!openOf(api.session().contexts.main).includes("sidebar_calls_lib.py::sleaf"), "its callee's structure node is not what was opened");
      await vscode.commands.executeCommand("vnoc.collapse", card(several.id));
      assert.ok(!openOf(api.session().contexts.main).includes(several.id));

      // Go to Call Site: one site goes there
      await vscode.commands.executeCommand("vnoc.goToCallSite", card(once.id));
      await until("the editor at the one site", () => inApp() && line() === 9);
      await vscode.commands.executeCommand("workbench.action.closeAllEditors");

      // …several are a list that previews each; Enter goes to the one picked
      await vscode.commands.executeCommand("vnoc.goToCallSite", card(several.id));
      await vscode.commands.executeCommand("workbench.action.quickOpenSelectNext");
      await until("the preview of the second site", () => vscode.window.visibleTextEditors.some((e) => e.document.uri.fsPath.endsWith("sidebar_calls_app.py") && e.selection.active.line === 5));
      await vscode.commands.executeCommand("workbench.action.acceptSelectedQuickOpenItem");
      await until("the editor at the picked site", () => inApp() && line() === 5);

      // Explore follows the call into its own context, activated
      const explore = exploreId(several.id);
      await vscode.commands.executeCommand("vnoc.explore", card(several.id));
      await until("the EXPLORE context to be active", () => api.session().active === explore);

      // Close with no node closes the active EXPLORE; MAIN stays; Close Others keeps the one it is asked about
      await vscode.commands.executeCommand("vnoc.closeContext", { contextId: "main", webviewSection: "context", contextKind: "main" });
      assert.deepStrictEqual(api.session().order, ["main", explore], "MAIN has no Close");
      await vscode.commands.executeCommand("vnoc.closeOtherContexts", { contextId: explore, webviewSection: "context", contextKind: "explore" });
      assert.deepStrictEqual(api.session().order, ["main", explore], "the one it is asked about stays");
      await vscode.commands.executeCommand("vnoc.closeOtherContexts", { contextId: "main", webviewSection: "context", contextKind: "main" });
      assert.deepStrictEqual(api.session().order, ["main"], "from MAIN every EXPLORE is another");
      await vscode.commands.executeCommand("vnoc.explore", card(several.id));
      await until("the EXPLORE context again", () => api.session().active === explore);
      await vscode.commands.executeCommand("vnoc.closeContext"); // the canvas background's Close Explore: no node, the active one
      assert.deepStrictEqual([api.session().order, api.session().active], [["main"], "main"]);
    } finally {
      await vscode.commands.executeCommand("workbench.action.closeAllEditors");
      api.dispatch({ type: "activate", id: "main" });
      api.dispatch({ type: "select", id: null });
      api.dispatch({ type: "collapseAll" });
    }
  });

  test("moving through the search rows points at nodes without selecting; clearing the box clears the pointer", async () => {
    // With the canvas open, cards are drawn, so a highlight really reaches them (it once crashed the page: a hook was skipped on the highlighted card).
    // The rows are the sidebar box's now (*V-NOC: Search…* puts its cursor there), so nothing closes them when the keyboard goes elsewhere; the keys are sent to the page as the box handles
    // them. Before it is opened, wait for what could change what it lists: the canvas page (it says it drew only after VS Code ran the new tab's focus request), and the index (a node that goes
    // while it is pointed at drops the pointer, correctly).
    const drawn = api.renderedPosts().length;
    await vscode.commands.executeCommand("vnoc.openCanvas");
    await until("the canvas tab", () => canvasTabs().length === 1);
    await until("the canvas page to draw, its focus request done", () => api.renderedPosts().length > drawn, 30_000);
    await structureSettled();
    api.dispatch({ type: "select", id: first.id });
    const selected = api.session().contexts.main.selected;
    const timeline = recordTimeline(() => api.session().highlight);
    try {
      await vscode.commands.executeCommand("vnoc.search", "method"); // an empty box shows nothing: the start is a query (four `method` nodes: enough for two moves)
      await until("the list's first row to be pointed at", () => api.session().highlight !== null);
      for (const step of ["next", "next again"]) {
        const was = api.session().highlight;
        api.searchKey("ArrowDown");
        await until(`the pointer to move to ${step} row`, () => api.session().highlight !== null && api.session().highlight !== was);
      }
      assert.strictEqual(api.session().contexts.main.selected, selected, "a highlight never changes the selection");
      api.searchKey("Escape"); // clears the box, which clears the pointer
      await until("the pointer to clear", () => api.session().highlight === null);
    } catch (e) {
      throw new Error(`${(e as Error).message}; what happened, in ms from the start: ${timeline.log.join(" | ")}`);
    } finally {
      timeline.stop();
    }
  });

  test("picking a result the focus hides (*V-NOC: Search…*, then Enter in the box) shortens the focus and selects the node (03 §3)", async () => {
    await closeCanvas();
    await vscode.commands.executeCommand("workbench.action.closeAllEditors");
    const root = (await api.structure()).root;
    const everything: TreeNode[] = [];
    const walk = (n: TreeNode) => (everything.push(n), n.children.forEach(walk));
    root.children.forEach(walk);
    const last = everything.at(-1)!; // searched for by its name: the first item of the list
    assert.strictEqual(everything.filter((n) => n.name === last.name).length, 1, "its name is the name of no other node, so it is the first the core finds");
    const otherFile = root.children.find((n) => n.name === "other.py")!;
    api.dispatch({ type: "focus", id: otherFile.id });
    assert.deepStrictEqual([...api.session().contexts.main.focus].at(-1), otherFile.id);
    assert.notStrictEqual(last.path, otherFile.path, "the node is outside the focus");
    await structureSettled();
    const timeline = recordTimeline(() => api.session().highlight);
    try {
      await vscode.commands.executeCommand("vnoc.search", last.name);
      await until("the list's first row to be pointed at", () => api.session().highlight === last.id);
      api.searchKey("Enter");
      await until("the focus to shorten and the node to be selected", () => api.session().contexts.main.focus.length === 0 && api.session().contexts.main.selected === last.id);
    } catch (e) {
      throw new Error(`${(e as Error).message}; what happened, in ms from the start: ${timeline.log.join(" | ")}`);
    } finally {
      timeline.stop();
    }
    await vscode.commands.executeCommand("workbench.action.closeAllEditors");
  });

  test("a click selects the row; clicking the selected row again changes nothing in the contexts", async () => {
    await closeCanvas();
    await api.rowClick("main", first.id); // a known starting point
    await sleep(600);
    const before = api.session().contexts;
    await api.rowClick("main", other.id);
    await sleep(800);
    assert.strictEqual(api.session().contexts.main.selected, other.id);
    assert.notStrictEqual(api.session().contexts, before, "the selection is a change");
    // (the code that opens moves the cursor, which is not in `contexts`: `seq` would count it)
    const settled = api.session().contexts;
    await api.rowClick("main", other.id);
    await sleep(500);
    assert.strictEqual(api.session().contexts, settled, "clicking what is selected changes nothing");
  });
});

suite("the pages", () => {
  test("no V-NOC page crashed while the other tests ran", () => {
    assert.deepStrictEqual(api.webviewErrors(), []);
  });
});
