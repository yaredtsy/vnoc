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
  renderedPosts(): { visible: number; hidden: number; calls: number }[];
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
