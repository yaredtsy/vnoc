import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";
import * as vscode from "vscode";

import type { CallTreeResult, StructureResult, TreeNode } from "@vnoc/shared/messages";
import type { Intent, Session } from "@vnoc/shared/session.ts";
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
  renderedPosts(): { visible: number; hidden: number; calls: number; chevrons: number }[];
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
    await until("a card to draw what it calls", () => api.renderedPosts().some((r) => r.calls >= 1), 30_000);
    api.dispatch({ type: "toggle", id: main.id }); // closed again for the tests after
    api.dispatch({ type: "collapseAll" });
  });

  test("Show Calls on a function opens the canvas on it and opens it: its tree arrives (loading, then idle) and a card draws it", async () => {
    await vscode.commands.executeCommand("workbench.action.closeAllEditors");
    await vscode.commands.executeCommand("vnoc.showCalls", main);
    await until("the canvas tab", () => canvasTabs().length === 1);
    assert.strictEqual(api.session().contexts.main.selected, main.id, "the entry is selected");
    assert.ok(api.session().contexts.main.expanded.includes(main.id), "and open: that is what makes the host fetch it");
    await until("its tree", () => api.callTrees()[main.id] !== undefined, 30_000);
    assert.deepStrictEqual(statesOf(main.id).slice(0, 2), ["loading", "idle"]);
    // main calls leaf twice (merged: one node) and twice once, which calls leaf: three nodes
    assert.strictEqual(api.callTrees()[main.id].root.children.length, 2, "leaf (×2) and twice");
    await until("a card to draw the Calls section", () => api.renderedPosts().some((r) => r.calls >= 1), 30_000);
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
    assert.ok(api.session().contexts.main.expanded.includes(main.id), "it is still open");
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
