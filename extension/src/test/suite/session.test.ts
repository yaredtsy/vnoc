import * as assert from "assert";
import * as vscode from "vscode";

import type { StructureResult, TreeNode } from "@vnoc/shared/messages";
import type { Session } from "@vnoc/shared/session.ts";
import { canvasTabs, closeCanvas, guarded, sleep, soft, until } from "./util";

// The session in a real window (docs/10-ui-rework/05-build-order.md §4): what the pure tests in shared/ cannot see, the canvas and the host together.

interface Api {
  structure(): Promise<StructureResult>;
  session(): Session;
  saved(): { contexts: Record<string, { focus: string[] }> } | undefined;
}
// the hooks of this file wait for the core: when it has stopped answering they stand aside (`soft`), and each test fails at once (nothing is skipped)
const suiteSetup = (fn: (this: Mocha.Context) => unknown) => globalThis.suiteSetup(soft(fn));

let api: Api;

suiteSetup(async () => {
  api = guarded(await vscode.extensions.getExtension("vnoc.vnoc")!.activate());
});

suite("the session", () => {
  let first: TreeNode;
  let second: TreeNode;

  suiteSetup(async () => {
    const file = (await api.structure()).root.children[0];
    [first, second] = file.children;
    await closeCanvas();
  });

  test("two Open in Canvas on a node before the canvas is up: the second one is the one that waits for it", async () => {
    await vscode.commands.executeCommand("vnoc.openInCanvas", { nodeKey: first.id, webviewSection: "node" }); // what a row's menu passes
    await vscode.commands.executeCommand("vnoc.openInCanvas", { nodeKey: second.id, webviewSection: "node" });
    assert.strictEqual(api.session().reveal?.id, second.id, "the request in the session is the second one");
    assert.strictEqual(api.session().contexts.main.selected, second.id);
    // the canvas starts, finds it in the session, centres the node and reports it done: the session clears it
    await until("the canvas to do the reveal", () => api.session().reveal === null);
  });

  test("a reveal that was done is not done again by a canvas opened later", async () => {
    await closeCanvas();
    await vscode.commands.executeCommand("vnoc.openCanvas");
    await until("the canvas tab", () => canvasTabs().length === 1);
    await sleep(1500); // time enough for a replay, if there were one
    assert.strictEqual(api.session().reveal, null);
  });

  test("Open Canvas takes no node: it opens the canvas and reveals nothing", async () => {
    await closeCanvas();
    const before = api.session().contexts.main.selected;
    await vscode.commands.executeCommand("vnoc.openCanvas", first); // a node given to it is not used
    await until("the canvas tab", () => canvasTabs().length === 1);
    assert.strictEqual(api.session().reveal, null);
    assert.strictEqual(api.session().contexts.main.selected, before, "nothing was selected");
  });

  test("Focus focuses without opening the canvas; Open in Canvas opens it and reveals the node without focusing", async () => {
    const file = (await api.structure()).root.children[0];
    await closeCanvas();
    await vscode.commands.executeCommand("vnoc.focus", file);
    await sleep(800);
    assert.strictEqual(canvasTabs().length, 0, "Focus goes to the sidebar, not the canvas");
    assert.deepStrictEqual([...api.session().contexts.main.focus], [file.id]);
    await vscode.commands.executeCommand("vnoc.focusClear");
    await vscode.commands.executeCommand("vnoc.openInCanvas", { nodeKey: file.id, webviewSection: "node" });
    await until("the canvas tab", () => canvasTabs().length === 1);
    assert.deepStrictEqual([...api.session().contexts.main.focus], [], "Open in Canvas reveals; it does not focus");
    assert.strictEqual(api.session().contexts.main.selected, file.id);
  });

  test("a focus is saved with the workspace, for the next start", async () => {
    const file = (await api.structure()).root.children[0];
    await vscode.commands.executeCommand("vnoc.focus", file);
    assert.deepStrictEqual(api.saved()?.contexts.main.focus, [file.id]);
    await vscode.commands.executeCommand("vnoc.focusClear");
    assert.deepStrictEqual(api.saved()?.contexts.main.focus, []);
  });
});
