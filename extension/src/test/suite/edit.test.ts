import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";
import * as vscode from "vscode";

import type { TreeNode } from "@vnoc/shared/messages";
import type { Intent } from "@vnoc/shared/session.ts";
import { allNodes } from "@vnoc/shared/tree";
import { canvasTabs, sleep, until } from "./util";

// Editing in the canvas (docs/17-edit-in-canvas.md), step 9.1: a card that shows the code of a file with unsaved changes says so. The suite cannot see the
// page, so it listens to what the page posts (`codeShown`: each time a card drew the code it read, and whether it showed the mark).

interface Api {
  structure(): Promise<{ root: TreeNode }>;
  codeShown(): { key: string; dirty: boolean; text: string }[];
  dirtyFiles(): string[];
  dispatch(intent: Intent): void;
  webviewErrors(): string[];
}
let api: Api;

const KEY = "nested.py::decorated";

suite("reading: a card marks a file with unsaved changes", () => {
  suiteSetup(async () => {
    api = await vscode.extensions.getExtension("vnoc.vnoc")!.activate();
  });

  test("a card's code is marked while its file is dirty, and not after a save or a revert", async () => {
    const file = path.join(vscode.workspace.workspaceFolders![0].uri.fsPath, "nested.py");
    const original = fs.readFileSync(file, "utf8");
    const node = allNodes((await api.structure()).root).find((n) => n.id === KEY)!;
    assert.ok(node, `${KEY} is in the fixture`);
    const last = () => api.codeShown().filter((p) => p.key === KEY).at(-1);
    /** Does `act`, then waits for a card to draw its code with this `dirty` (only drawings after the action count). */
    const shownAfter = async (what: string, dirty: boolean, act: () => PromiseLike<unknown>) => {
      const before = api.codeShown().length;
      await act();
      // the canvas draws only the cards on screen (03 §6.1): a card that left the page has nothing to mark, and one that comes back reads its code again
      for (let waited = 0; !api.codeShown().slice(before).some((p) => p.key === KEY && p.dirty === dirty); waited += 500) {
        assert.ok(waited < 30_000, `timed out waiting for ${what}`);
        api.dispatch({ type: "reveal", id: KEY, canvas: true });
        await sleep(500);
      }
    };
    const appendComment = async (document: vscode.TextDocument) => {
      const edit = new vscode.WorkspaceEdit();
      edit.insert(document.uri, new vscode.Position(document.lineCount, 0), "# a comment at the end\n");
      assert.ok(await vscode.workspace.applyEdit(edit));
      assert.ok(document.isDirty, "the edit left unsaved changes");
    };

    try {
      await vscode.commands.executeCommand("vnoc.openCanvas");
      await until("the canvas tab", () => canvasTabs().length === 1);
      const document = await vscode.workspace.openTextDocument(vscode.Uri.file(file));
      assert.ok(!document.isDirty);

      api.dispatch({ type: "expand", ids: ["nested.py"] }); // its function cards are drawn
      // an event sent before the new page listens is lost, and showing the code twice is the same as once: ask until a card draws it
      for (let waited = 0; !last(); waited += 500) {
        assert.ok(waited < 30_000, "the card never drew its code");
        await vscode.commands.executeCommand("vnoc.showCodeInCard", node);
        await sleep(500);
      }
      assert.strictEqual(last()!.dirty, false, "unmarked while the file is saved");
      assert.match(last()!.text, /def decorated\(\):/, "the code of the function (its range starts at the decorator)");

      await shownAfter("the mark, after an edit in the file", true, async () => {
        await appendComment(document);
        await until("the host to hold the file as dirty", () => api.dirtyFiles().includes("nested.py"));
      });
      await shownAfter("no mark, after a save", false, () => document.save());
      assert.ok(!document.isDirty);

      await shownAfter("the mark again", true, () => appendComment(document));
      await shownAfter("no mark, after a revert", false, async () => {
        await vscode.window.showTextDocument(document); // the revert command acts on the active editor
        await vscode.commands.executeCommand("workbench.action.files.revert");
      });
      assert.ok(!document.isDirty);
      assert.deepStrictEqual(api.webviewErrors(), [], "the page drew the mark without crashing");
    } finally {
      await vscode.commands.executeCommand("vnoc.hideCodeInCard", node);
      api.dispatch({ type: "collapseAll" });
      await vscode.commands.executeCommand("workbench.action.closeAllEditors");
      fs.writeFileSync(file, original);
    }
  });
});
