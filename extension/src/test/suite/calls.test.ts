import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";
import * as vscode from "vscode";

import { callNodesIn } from "@vnoc/shared/calls.ts";
import type { CallTreeResult, StructureResult, TreeNode } from "@vnoc/shared/messages";
import { type Intent, type Session, openOf } from "@vnoc/shared/session.ts";
import { canvasTabs, sleep, until } from "./util";

// Show Calls in a real window (docs/16-ui-redesign/05-calls-as-nodes.md §2, §5, §7): the host fetches the call trees of what is open, cancels what stops
// being wanted, fetches a stale tree again by itself, and starts over when the core restarts. The suite reads the host's own log of what happened.

type LogEntry = { kind: "state"; key: string; state: "loading" | "failed" | "idle" } | { kind: "tree"; key: string; revision: number; epoch?: number } | { kind: "dropped"; key: string } | { kind: "reset" };
interface Api {
  structure(): Promise<StructureResult>;
  session(): Session;
  webviewErrors(): string[];
  callTrees(): Record<string, CallTreeResult>;
  callTreeLog(): LogEntry[];
  holdCallTrees(ms: number): void;
  coreEpoch(): number;
  dispatch(intent: Intent, context?: string): void;
  restartCore(): Promise<void>;
  chips(): { id: string; label: string; active: boolean }[];
  renderedPosts(): { context: string; visible: number; hidden: number; callCards: number; chevrons: number }[];
  callProbes(): Record<string, { revision: number; count: number }>;
}
let api: Api;

const LIB = "def leaf():\n    return 1\n\n\ndef twice():\n    leaf()\n    leaf()\n";
const APP = "from calls_lib import leaf, twice\n\n\ndef main():\n    leaf()\n    twice()\n    leaf()\n";

const find = (node: TreeNode, id: string): TreeNode | undefined => (node.id === id ? node : node.children.map((c) => find(c, id)).find(Boolean));
const statesOf = (key: string) => api.callTreeLog().filter((e): e is Extract<LogEntry, { kind: "state" }> => e.kind === "state" && e.key === key).map((e) => e.state);

suite("show calls", () => {
  let main: TreeNode;
  let twice: TreeNode;

  suiteSetup(async () => {
    api = await vscode.extensions.getExtension("vnoc.vnoc")!.activate();
    const folder = vscode.workspace.workspaceFolders![0].uri.fsPath;
    fs.writeFileSync(path.join(folder, "calls_lib.py"), LIB);
    fs.writeFileSync(path.join(folder, "calls_app.py"), APP);
    for (let waited = 0; ; waited += 200) {
      const structure = await api.structure();
      const a = find(structure.root, "calls_app.py::main");
      const b = find(structure.root, "calls_lib.py::twice");
      if (a && b) {
        [main, twice] = [a, b];
        assert.strictEqual(structure.capabilities.callTree, true, "the driver says it has call trees, so the command is offered");
        break;
      }
      assert.ok(waited < 20_000, "the fixture never reached the structure");
      await sleep(200);
    }
  });

  suiteTeardown(() => api.holdCallTrees(0));

  test("the function cards of a file get their chevrons from probes: main and twice have one, leaf (it calls nothing) does not", async () => {
    await vscode.commands.executeCommand("vnoc.openCanvas");
    await until("the canvas tab", () => canvasTabs().length === 1);
    const before = api.callTreeLog().length; // the log holds what the suites before this one did too
    api.dispatch({ type: "select", id: null });
    api.dispatch({ type: "expand", ids: ["calls_app.py", "calls_lib.py"] }); // their function cards are drawn: the page says so, the host probes them
    const probes = () => api.callProbes();
    await until("the probes", () => [main.id, twice.id, "calls_lib.py::leaf"].every((k) => probes()[k] !== undefined), 30_000);
    assert.deepStrictEqual([probes()[main.id].count, probes()[twice.id].count, probes()["calls_lib.py::leaf"].count], [2, 1, 0], "leaf and twice, merged; leaf twice, merged; nothing");
    assert.strictEqual(api.callTrees()[main.id], undefined, "a probe is not a tree: nothing was fetched");
    await until("the page to draw their chevrons", () => api.renderedPosts().at(-1)?.chevrons === 2, 30_000);
    assert.ok(!api.callTreeLog().slice(before).some((e) => e.kind === "state" && e.state === "loading"), "no call tree was asked for: only probes ran");
  });

  test("opening a card that has a chevron fetches its tree and a card draws it", async () => {
    api.dispatch({ type: "toggle", id: main.id });
    await until("its tree", () => api.callTrees()[main.id] !== undefined, 30_000);
    assert.strictEqual(api.callTrees()[main.id].root.children.length, 2, "what the probe counted");
    await until("a card to draw what it calls", () => api.renderedPosts().some((r) => r.callCards >= 1), 30_000);
    api.dispatch({ type: "toggle", id: main.id }); // closed again for the tests after
    api.dispatch({ type: "collapseAll" });
  });

  test("Show Calls on a function opens the canvas on it and opens it: its tree arrives (loading, then idle) and a card draws it", async () => {
    await vscode.commands.executeCommand("workbench.action.closeAllEditors");
    await vscode.commands.executeCommand("vnoc.showCalls", main);
    await until("the canvas tab", () => canvasTabs().length === 1);
    assert.strictEqual(api.session().contexts.main.selected, main.id, "the entry is selected");
    assert.ok(openOf(api.session().contexts.main).includes(main.id), "and open: that is what makes the host fetch it");
    await until("its tree", () => api.callTrees()[main.id] !== undefined, 30_000);
    assert.deepStrictEqual(statesOf(main.id).slice(0, 2), ["loading", "idle"]);
    // main calls leaf twice (merged: one node) and twice once, which calls leaf: three nodes
    assert.strictEqual(api.callTrees()[main.id].root.children.length, 2, "leaf (×2) and twice");
    await until("a card to draw the Calls section", () => api.renderedPosts().some((r) => r.callCards >= 1), 30_000);
  });

  test("closing it before the answer cancels the request: no tree is kept for it", async () => {
    api.holdCallTrees(1500); // the answer would take 1.5 s
    try {
      const before = api.callTreeLog().length;
      await vscode.commands.executeCommand("vnoc.showCalls", twice);
      api.dispatch({ type: "toggle", id: twice.id }); // closed again, and no longer the selection (a selected function is prefetched)
      api.dispatch({ type: "select", id: null });
      const mine = () => api.callTreeLog().slice(before).filter((e) => "key" in e && e.key === twice.id);
      await until("the request to start and be cancelled", () => mine().some((e) => e.kind === "state" && e.state === "idle"));
      await sleep(2200); // longer than the hold: a request that was not cancelled would have answered by now
      assert.deepStrictEqual(mine().map((e) => (e.kind === "state" ? e.state : e.kind)), ["loading", "idle"], "loading, then cancelled: never a tree");
      assert.strictEqual(api.callTrees()[twice.id], undefined);
    } finally {
      api.holdCallTrees(0);
    }
  });

  test("when the code changes, a tree that is open is fetched again by itself, and the page is not left behind", async () => {
    const was = api.callTrees()[main.id].revision;
    const folder = vscode.workspace.workspaceFolders![0].uri.fsPath;
    fs.writeFileSync(path.join(folder, "calls_lib.py"), LIB + "\n\ndef more():\n    pass\n");
    await until("the tree to be fetched again", () => (api.callTrees()[main.id]?.revision ?? 0) > was, 30_000);
    assert.ok(statesOf(main.id).filter((s) => s === "loading").length >= 2, "a second request was made, nobody pressed a button");
    assert.deepStrictEqual(api.webviewErrors(), []);
  });

  test("a core restart while a function is open: every tree is dropped, and the open one comes back from the new core", async () => {
    const epoch = api.coreEpoch();
    const old = api.callTrees()[main.id];
    assert.ok(old.epoch === epoch, "the tree says which run of the core made it");
    await api.restartCore();
    await until("the tree from the new core", () => api.callTrees()[main.id]?.epoch === api.coreEpoch(), 60_000);
    assert.ok(api.coreEpoch() > epoch);
    assert.ok(api.callTreeLog().some((e) => e.kind === "reset"), "the host said so, so the pages drop theirs too");
    assert.ok(openOf(api.session().contexts.main).includes(main.id), "it is still open");
    await sleep(500);
    assert.deepStrictEqual(api.webviewErrors(), []);
  });

  test("Show Calls on a class with its own __init__ shows the __init__'s calls (R7): the tree is the __init__'s, not the class's", async () => {
    const folder = vscode.workspace.workspaceFolders![0].uri.fsPath;
    fs.writeFileSync(path.join(folder, "calls_cls.py"), "from calls_lib import leaf\n\n\nclass Widget:\n    def __init__(self):\n        leaf()\n");
    let widget: TreeNode | undefined;
    await until("the class in the structure", () => (void api.structure().then((s) => (widget = find(s.root, "calls_cls.py::Widget"))), widget !== undefined), 20_000);
    await vscode.commands.executeCommand("vnoc.showCalls", widget);
    const init = "calls_cls.py::Widget.__init__";
    await until("the __init__'s tree", () => api.callTrees()[init] !== undefined, 30_000);
    assert.strictEqual(api.callTrees()["calls_cls.py::Widget"], undefined, "the class itself owns no tree");
    assert.deepStrictEqual(api.callTrees()[init].root.children.map((c) => c.name), ["leaf"]);
  });

  test("a function that is open has its call cards drawn, and a call card is a row of the layout like any other", async () => {
    api.dispatch({ type: "collapseAll" }); // what earlier tests left open would draw cards too
    await vscode.commands.executeCommand("vnoc.showCalls", main);
    await until("its two call cards: leaf (merged from two calls) and twice", () => api.renderedPosts().at(-1)?.callCards === 2, 30_000);
  });

  test("selecting a call card opens its EXPLORE context, not activated; double-click (Explore) activates it", async () => {
    const [leaf] = callNodesIn(main.id, api.callTrees()[main.id].root);
    const explore = `explore:${leaf.id}`;
    api.dispatch({ type: "select", id: leaf.id });
    await until("its EXPLORE context", () => api.session().order.includes(explore));
    assert.strictEqual(api.session().active, "main", "the canvas stays where it was");
    assert.deepStrictEqual([...api.session().contexts[explore].focus], ["calls_lib.py", "calls_lib.py::leaf"], "it shows the callee at home");
    api.dispatch({ type: "explore", call: leaf.id, activate: true }); // what a double-click sends
    await until("it to be active", () => api.session().active === explore);
    api.dispatch({ type: "activate", id: "main" });
    api.dispatch({ type: "select", id: null });
    await until("it to close", () => !api.session().order.includes(explore));
  });

  test("a double-click on a call card draws a chip, active; the Main chip brings the canvas back; the ✕ closes the context and the chip goes", async () => {
    // the suite cannot click in the page: it sends the intent a click sends, and reads what the page says it drew (`chipsDrawn`, `rendered.context`)
    const [leaf] = callNodesIn(main.id, api.callTrees()[main.id].root);
    const explore = `explore:${leaf.id}`;
    const drewContext = () => api.renderedPosts().at(-1)?.context;
    await until("the previous test's chip to go (one context: no chips)", () => api.chips().length === 0);

    api.dispatch({ type: "explore", call: leaf.id, activate: true }); // the double-click
    await until("a chip for it, filled", () => api.chips().some((c) => c.id === explore && c.active));
    assert.deepStrictEqual(api.chips().map((c) => [c.label, c.active]), [["Main", false], ["leaf", true]]);
    await until("the canvas to draw the EXPLORE context", () => drewContext() === explore);

    api.dispatch({ type: "activate", id: "main" }); // the click on Main
    await until("Main's chip to be filled", () => api.chips().find((c) => c.id === "main")?.active === true);
    await until("the canvas to draw MAIN again", () => drewContext() === "main");
    assert.ok(api.session().order.includes(explore), "the EXPLORE context is still there, one click away");

    api.dispatch({ type: "closeContext", id: explore }); // the ✕
    await until("the chip to go", () => api.chips().length === 0);
    assert.ok(!api.session().order.includes(explore));

    // the active one closed: its parent is active, and the canvas draws it
    api.dispatch({ type: "explore", call: leaf.id, activate: true });
    await until("the chip, active", () => api.chips().some((c) => c.id === explore && c.active));
    api.dispatch({ type: "closeContext", id: explore });
    await until("the canvas back on MAIN", () => api.session().active === "main" && drewContext() === "main" && api.chips().length === 0);
    assert.deepStrictEqual(api.webviewErrors(), []);
  });

  test("a cut call card that is opened gets its callee's tree fetched", async () => {
    const folder = vscode.workspace.workspaceFolders![0].uri.fsPath;
    // 40 functions in a chain: the full tree is cut at the depth cap (32), so the node at the cap has its calls not loaded
    let text = "def head():\n    c0()\n\n\n";
    for (let i = 0; i < 39; i++) text += `def c${i}():\n    c${i + 1}()\n\n\n`;
    text += "def c39():\n    return 1\n";
    fs.writeFileSync(path.join(folder, "calls_chain.py"), text);
    let head: TreeNode | undefined;
    await until("the chain in the structure", () => (void api.structure().then((s) => (head = find(s.root, "calls_chain.py::head"))), head !== undefined), 20_000);
    await vscode.commands.executeCommand("vnoc.showCalls", head);
    await until("the tree of head", () => api.callTrees()["calls_chain.py::head"] !== undefined, 30_000);
    const nodes = callNodesIn("calls_chain.py::head", api.callTrees()["calls_chain.py::head"].root);
    const cutNode = nodes.at(-1)!;
    assert.deepStrictEqual(cutNode.node.marks, ["cut"], "the node at the cap says the walk stopped there");
    assert.strictEqual(api.callTrees()[cutNode.node.key!], undefined, "its own tree is not held yet");
    // open every card down the chain, the cut one last: the host then wants the callee's own tree
    api.dispatch({ type: "expand", ids: nodes.map((n) => n.id) });
    await until("the cut node's own tree", () => api.callTrees()[cutNode.node.key!] !== undefined, 30_000);
    await until("more call cards than the cap", () => (api.renderedPosts().at(-1)?.callCards ?? 0) > 33, 30_000);
    api.dispatch({ type: "collapseAll" });
  });

  test("what is opened inside a focus stays inside it: clearing the focus gives the project's set as it was left, and the canvas draws it", async () => {
    api.dispatch({ type: "focusJump", depth: 0 });
    api.dispatch({ type: "collapseAll" });
    const before = [...openOf(api.session().contexts.main)];
    api.dispatch({ type: "focus", id: twice.id });
    api.dispatch({ type: "expand", ids: [twice.id] }); // opened inside the focus on `twice`
    assert.ok(openOf(api.session().contexts.main).includes(twice.id));
    api.dispatch({ type: "focusJump", depth: 0 });
    assert.deepStrictEqual([...openOf(api.session().contexts.main)], before, "the project's own set is as it was left: what was opened in the focus did not come out");
    await until("the canvas to draw the project again", () => api.renderedPosts().at(-1)?.context === "main" && (api.renderedPosts().at(-1)?.visible ?? 0) > 1);
    api.dispatch({ type: "focus", id: twice.id });
    assert.ok(openOf(api.session().contexts.main).includes(twice.id), "focus on it again: its set as it was left");
    api.dispatch({ type: "focusJump", depth: 0 });
    api.dispatch({ type: "collapseAll" });
  });

  test("a big file opened draws at most 200 cards, and what it held back is hidden, not lost (03 §6.1)", async () => {
    const folder = vscode.workspace.workspaceFolders![0].uri.fsPath;
    const file = path.join(folder, "calls_big.py");
    fs.writeFileSync(file, Array.from({ length: 250 }, (_, i) => `def big${i}():\n    return ${i}\n`).join("\n\n"));
    try {
      let big: TreeNode | undefined;
      await until("the big file in the structure", () => (void api.structure().then((s) => (big = find(s.root, "calls_big.py"))), big !== undefined && big.children.length === 250), 30_000);
      const seen = api.renderedPosts().length;
      api.dispatch({ type: "expand", ids: [big!.id] }); // what Expand All Below does
      await until("a drawing that held the file's functions back", () => api.renderedPosts().length > seen && (api.renderedPosts().at(-1)?.hidden ?? 0) >= 250, 30_000);
      assert.ok((api.renderedPosts().at(-1)?.visible ?? 999) <= 200, `visible ${api.renderedPosts().at(-1)?.visible}`);
      assert.deepStrictEqual(api.webviewErrors(), []);
    } finally {
      api.dispatch({ type: "collapseAll" });
      fs.rmSync(file, { force: true });
      let left = false;
      await until("the file to leave the structure", () => (void api.structure().then((st) => (left = find(st.root, "calls_big.py") === undefined)), left), 30_000);
    }
  });

  test("the command refuses a file: calls start at a function or a class", async () => {
    const file = find((await api.structure()).root, "calls_app.py")!;
    // the command shows the refusal as a message and does not throw; nothing is fetched for the file
    await vscode.commands.executeCommand("vnoc.showCalls", file);
    await sleep(500);
    assert.strictEqual(api.callTrees()[file.id], undefined);
  });

  test("no V-NOC page crashed", () => {
    assert.deepStrictEqual(api.webviewErrors(), []);
  });
});
