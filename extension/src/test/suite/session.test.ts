import * as assert from "assert";
import * as vscode from "vscode";

import type { StructureResult, TreeNode } from "@vnoc/shared/messages";
import type { Session } from "@vnoc/shared/session.ts";
import { canvasTabs, closeCanvas, sleep, until } from "./util";

// The session in a real window (docs/10-ui-rework/05-build-order.md §4): what the pure tests in shared/ cannot see, the canvas and the host together.

interface Api {
  structure(): Promise<StructureResult>;
  session(): Session;
  saved(): { contexts: Record<string, { focus: string[] }> } | undefined;
}
let api: Api;

suiteSetup(async () => {
  api = await vscode.extensions.getExtension("vnoc.vnoc")!.activate();
});

suite("the session", () => {
  let first: TreeNode;
  let second: TreeNode;

  suiteSetup(async () => {
    const file = (await api.structure()).root.children[0];
    [first, second] = file.children;
    await closeCanvas();
  });

  test("two Open Canvas on a node before the canvas is up: the second one is the one that waits for it", async () => {
    await vscode.commands.executeCommand("vnoc.openCanvas", first);
    await vscode.commands.executeCommand("vnoc.openCanvas", second);
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

  test("Focus focuses without opening the canvas; Show on Canvas focuses and opens it", async () => {
    const file = (await api.structure()).root.children[0];
    await closeCanvas();
    await vscode.commands.executeCommand("vnoc.focus", file);
    await sleep(800);
    assert.strictEqual(canvasTabs().length, 0, "Focus goes to the sidebar, not the canvas");
    assert.deepStrictEqual([...api.session().contexts.main.focus], [file.id]);
    await vscode.commands.executeCommand("vnoc.focusClear");
    await vscode.commands.executeCommand("vnoc.showOnCanvas", file);
    await until("the canvas tab", () => canvasTabs().length === 1);
    assert.deepStrictEqual([...api.session().contexts.main.focus], [file.id]);
    await vscode.commands.executeCommand("vnoc.focusClear");
  });

  test("a focus is saved with the workspace, for the next start", async () => {
    const file = (await api.structure()).root.children[0];
    await vscode.commands.executeCommand("vnoc.focus", file);
    assert.deepStrictEqual(api.saved()?.contexts.main.focus, [file.id]);
    await vscode.commands.executeCommand("vnoc.focusClear");
    assert.deepStrictEqual(api.saved()?.contexts.main.focus, []);
  });
});
