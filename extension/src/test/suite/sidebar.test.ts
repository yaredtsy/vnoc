import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";
import * as vscode from "vscode";

import type { StructureResult, TreeNode } from "@vnoc/shared/messages";
import type { Session } from "@vnoc/shared/session.ts";
import { canvasTabs, closeCanvas, sleep, until } from "./util";

// The sidebar in a real window (docs/10-ui-rework/02-sidebar.md §4, §5): what a click does with the canvas open and closed, and selection both ways.

interface Api {
  structure(): Promise<StructureResult>;
  session(): Session;
  selectInTree(id: string): Promise<void>;
}
let api: Api;

const textTabs = () => vscode.window.tabGroups.all.flatMap((g) => g.tabs).filter((t) => t.input instanceof vscode.TabInputText);

suiteSetup(async () => {
  api = await vscode.extensions.getExtension("vnoc.vnoc")!.activate();
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
  let other: TreeNode;

  suiteSetup(async () => {
    // a second file, so a preview tab can be seen being replaced
    const folder = vscode.workspace.workspaceFolders![0].uri.fsPath;
    fs.writeFileSync(path.join(folder, "other.py"), "def other():\n    pass\n");
    let files: TreeNode[] = [];
    for (let waited = 0; files.length < 2; waited += 200) {
      assert.ok(waited < 20_000, "other.py never reached the structure");
      files = (await api.structure()).root.children.filter((n) => n.kind === "file");
      await sleep(200);
    }
    first = files.find((f) => f.name === "nested.py")!.children[0];
    other = files.find((f) => f.name === "other.py")!.children[0];
  });

  test("with the canvas open, a click selects and reveals on the canvas, and opens no code tab", async () => {
    await vscode.commands.executeCommand("workbench.action.closeAllEditors");
    await vscode.commands.executeCommand("vnoc.openCanvas");
    await until("the canvas tab", () => canvasTabs().length === 1);
    await vscode.commands.executeCommand("vnoc.open", first);
    await sleep(1000);
    assert.strictEqual(textTabs().length, 0, "no code tab was opened");
    assert.strictEqual(canvasTabs().length, 1);
    assert.strictEqual(api.session().contexts.main.selected, first.id);
    await until("the canvas to do the reveal", () => api.session().reveal === null);
  });

  test("with the canvas closed, a click opens one preview tab, and the next click replaces it", async () => {
    await closeCanvas();
    await vscode.commands.executeCommand("workbench.action.closeAllEditors");
    await vscode.commands.executeCommand("vnoc.open", first);
    await until("a code tab", () => textTabs().length === 1);
    assert.ok(textTabs()[0].isPreview, "the tab is a preview tab");
    await vscode.commands.executeCommand("vnoc.open", other);
    await until("the other file", () => textTabs().some((t) => (t.input as vscode.TabInputText).uri.fsPath.endsWith("other.py")));
    assert.strictEqual(textTabs().length, 1, "the preview tab was replaced, not added to");
  });

  test("[check] the arrow keys move through the tree without running the row's command", async () => {
    await closeCanvas();
    await vscode.commands.executeCommand("workbench.action.closeAllEditors");
    await api.selectInTree(first.id); // a known selection to compare with
    await sleep(600);
    await vscode.commands.executeCommand("vnoc.structure.focus");
    await sleep(500);
    for (const key of ["list.focusFirst", "list.focusDown", "list.focusDown", "list.focusUp"]) {
      await vscode.commands.executeCommand(key);
      await sleep(250);
    }
    assert.strictEqual(textTabs().length, 0, "moving the focus opened an editor: VS Code ran the row's command on an arrow key");
    // what VS Code does with the arrow keys in this version: they move the tree's focus (the highlighted row), not the selection; Enter or a click selects
    assert.strictEqual(api.session().contexts.main.selected, first.id, "the arrow keys moved the selection (the plan assumed they would; see 02-sidebar.md §4)");
  });

  test("moving through the search list points at nodes without selecting; closing it clears the pointer", async () => {
    await api.selectInTree(first.id);
    await sleep(600);
    const selected = api.session().contexts.main.selected;
    await vscode.commands.executeCommand("vnoc.search");
    await until("the list's first item to be pointed at", () => api.session().highlight !== null);
    await vscode.commands.executeCommand("workbench.action.quickOpenSelectNext");
    await sleep(400);
    assert.notStrictEqual(api.session().highlight, null);
    assert.strictEqual(api.session().contexts.main.selected, selected, "a highlight never changes the selection");
    await vscode.commands.executeCommand("workbench.action.closeQuickOpen");
    await until("the pointer to clear", () => api.session().highlight === null);
  });

  test("a selection from the tree reaches the session once; the tree and the session do not chase each other", async () => {
    await closeCanvas();
    await api.selectInTree(first.id); // a known starting point
    await sleep(600);
    const before = api.session().seq;
    await api.selectInTree(other.id);
    await sleep(800);
    assert.strictEqual(api.session().contexts.main.selected, other.id);
    assert.strictEqual(api.session().seq, before + 1, "one change: the reveal fired the selection event, and the same selection changed nothing");
    await api.selectInTree(other.id);
    await sleep(500);
    assert.strictEqual(api.session().seq, before + 1, "selecting what is selected changes nothing");
  });
});
