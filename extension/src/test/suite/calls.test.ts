import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";
import * as vscode from "vscode";

import type { StructureResult, TreeNode } from "@vnoc/shared/messages";
import type { Session } from "@vnoc/shared/session.ts";
import { canvasTabs, sleep, until } from "./util";

// Show Call Tree in a real window (docs/14 §7): the command asks the core, the canvas comes up on the entry, and its card draws the "Calls" rows.

interface Api {
  structure(): Promise<StructureResult>;
  session(): Session;
  webviewErrors(): string[];
  waitCallsDrawn(key: string, ready: (drawn: { rows: number; stale: boolean }) => boolean, timeoutMs: number): Promise<{ rows: number; stale: boolean }>;
}
let api: Api;

const LIB = "def leaf():\n    return 1\n\n\ndef twice():\n    leaf()\n    leaf()\n";
const APP = "from calls_lib import leaf, twice\n\n\ndef main():\n    leaf()\n    twice()\n    leaf()\n";

const find = (node: TreeNode, id: string): TreeNode | undefined => (node.id === id ? node : node.children.map((c) => find(c, id)).find(Boolean));

suite("show call tree", () => {
  let main: TreeNode;

  suiteSetup(async () => {
    api = await vscode.extensions.getExtension("vnoc.vnoc")!.activate();
    const folder = vscode.workspace.workspaceFolders![0].uri.fsPath;
    fs.writeFileSync(path.join(folder, "calls_lib.py"), LIB);
    fs.writeFileSync(path.join(folder, "calls_app.py"), APP);
    for (let waited = 0; ; waited += 200) {
      const structure = await api.structure();
      const found = find(structure.root, "calls_app.py::main");
      if (found) {
        main = found;
        assert.strictEqual(structure.capabilities.callTree, true, "the driver says it has call trees, so the command is offered");
        break;
      }
      assert.ok(waited < 20_000, "calls_app.py never reached the structure");
      await sleep(200);
    }
  });

  test("the command on a function opens the canvas on it, and its card draws what it calls", async () => {
    await vscode.commands.executeCommand("workbench.action.closeAllEditors");
    await vscode.commands.executeCommand("vnoc.showCallTree", main);
    await until("the canvas tab", () => canvasTabs().length === 1);
    assert.strictEqual(api.session().contexts.main.selected, main.id, "the entry is selected");
    // main calls leaf twice (one row, merged) and twice once, which calls leaf: three rows
    const drawn = await api.waitCallsDrawn(main.id, (d) => d.rows === 3, 30_000);
    assert.deepStrictEqual(drawn, { rows: 3, stale: false });
  });

  test("when the code changes, the section says so and does not run again by itself", async () => {
    const folder = vscode.workspace.workspaceFolders![0].uri.fsPath;
    fs.writeFileSync(path.join(folder, "calls_lib.py"), LIB + "\n\ndef more():\n    pass\n");
    const drawn = await api.waitCallsDrawn(main.id, (d) => d.stale, 30_000);
    assert.deepStrictEqual(drawn, { rows: 3, stale: true }, "the old rows stay, marked stale");
  });

  test("the command refuses a file: a call tree starts at a function or a class", async () => {
    const file = find((await api.structure()).root, "calls_app.py")!;
    // the command shows the refusal as a message and does not throw; nothing is drawn for the file
    await vscode.commands.executeCommand("vnoc.showCallTree", file);
    await sleep(500);
    await assert.rejects(api.waitCallsDrawn(file.id, () => true, 500));
  });

  test("no V-NOC page crashed", () => {
    assert.deepStrictEqual(api.webviewErrors(), []);
  });
});
