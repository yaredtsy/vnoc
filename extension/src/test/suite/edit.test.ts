import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";
import * as vscode from "vscode";

import type { EditStartResult, Range, TreeNode } from "@vnoc/shared/messages";
import type { Intent } from "@vnoc/shared/session.ts";
import { allNodes } from "@vnoc/shared/tree";
import { canvasTabs, closeCanvas, sleep, until, waitFor } from "./util";

// Editing in the canvas (docs/17-edit-in-canvas.md), step 9.1: a card that shows the code of a file with unsaved changes says so. The suite cannot see the
// page, so it listens to what the page posts (`codeShown`: each time a card drew the code it read, and whether it showed the mark).

interface Api {
  structure(): Promise<{ root: TreeNode }>;
  codeShown(): { key: string; dirty: boolean; text: string }[];
  dirtyFiles(): string[];
  editStates(): { key: string; state: string }[];
  viewCommand(data: { name: "setDraft" | "save"; key: string; text?: string }): void;
  checkKeys(): void;
  keysChecked(): { found: boolean; target: string; taken: string[] }[];
  editStart(path: string, range: Range, saveFirst?: boolean): Promise<EditStartResult>;
  dispatch(intent: Intent): void;
  webviewErrors(): string[];
}
let api: Api;

const KEY = "nested.py::decorated";

/**
 * Puts the fixture back. If VS Code holds the file open it does it, with an edit and a save: a file written behind its back leaves the document older than the disk,
 * and the next save of it is refused ("the content of the file is newer") or waits on VS Code's own prompt, so the card stays `saving` (what made 9.3's tests fail
 * one run in three). A file nothing holds open is written directly.
 */
async function restoreFixture(file: string, original: string): Promise<void> {
  const open = vscode.workspace.textDocuments.find((d) => d.uri.fsPath === file);
  if (open && open.getText() !== original) {
    const edit = new vscode.WorkspaceEdit();
    edit.replace(open.uri, new vscode.Range(0, 0, open.lineCount, 0), original);
    await vscode.workspace.applyEdit(edit);
  }
  if (open?.isDirty) await open.save();
  if (fs.readFileSync(file, "utf8") !== original) fs.writeFileSync(file, original);
  await vscode.commands.executeCommand("workbench.action.closeAllEditors");
}

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
        api.dispatch({ type: "reveal", id: KEY, canvas: true }); // the canvas draws only the cards on screen: bring this one into view
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
      await restoreFixture(file, original);
    }
  });
});

suite("editing: Edit on a card (docs/17-edit-in-canvas.md step 9.2)", () => {
  suiteSetup(async () => {
    api = await vscode.extensions.getExtension("vnoc.vnoc")!.activate();
  });

  test("the menu's Edit in Card opens the card's code and starts a draft: the card says `editing`", async () => {
    const node = allNodes((await api.structure()).root).find((n) => n.id === KEY)!;
    try {
      await vscode.commands.executeCommand("vnoc.openCanvas");
      await until("the canvas tab", () => canvasTabs().length === 1);
      api.dispatch({ type: "expand", ids: ["nested.py"] });
      const latest = () => api.editStates().filter((p) => p.key === KEY).at(-1)?.state;
      // an event sent before the new page listens is lost, and a second Edit on a node that has a draft is ignored: ask until the card says so
      for (let waited = 0; latest() !== "editing"; waited += 500) {
        assert.ok(waited < 30_000, `the card never said editing (last: ${latest()})`);
        api.dispatch({ type: "reveal", id: KEY, canvas: true });
        await vscode.commands.executeCommand("vnoc.editInCard", node);
        await sleep(500);
      }
      assert.ok(api.editStates().some((p) => p.key === KEY && p.state === "reading"), "it showed its code before it started the draft");
      assert.deepStrictEqual(api.webviewErrors(), [], "the page drew the editing card without crashing");
    } finally {
      await closeCanvas(); // the draft lives in the page: closing the tab drops it
    }
  });

  test("keys typed in an editing card's code stay in the code: the canvas takes none of them (Space, Backspace, F…) (9.2c)", async () => {
    const node = allNodes((await api.structure()).root).find((n) => n.id === KEY)!;
    try {
      await vscode.commands.executeCommand("vnoc.openCanvas");
      await until("the canvas tab", () => canvasTabs().length === 1);
      api.dispatch({ type: "expand", ids: ["nested.py"] });
      // only what this canvas says counts: the one the test before closed said `editing` for the same card
      const since = api.editStates().length;
      const latest = () => api.editStates().slice(since).filter((p) => p.key === KEY).at(-1)?.state;
      for (let waited = 0; latest() !== "editing"; waited += 500) {
        assert.ok(waited < 30_000, `the card never said editing (last: ${latest()})`);
        api.dispatch({ type: "reveal", id: KEY, canvas: true });
        await vscode.commands.executeCommand("vnoc.editInCard", node);
        await sleep(500);
      }
      // a keydown dispatched on the editor's own element runs every handler of the page: one that takes the key calls preventDefault (React Flow does for Space)
      // Monaco is made a moment after the card starts editing: ask again until the page finds its editor
      for (let waited = 0; api.keysChecked().at(-1)?.found !== true; waited += 500) {
        assert.ok(waited < 30_000, `the page never found the editor (last answer: ${JSON.stringify(api.keysChecked().at(-1))})`);
        api.checkKeys();
        await sleep(500);
      }
      assert.deepStrictEqual(api.keysChecked().at(-1), { found: true, target: "TEXTAREA", taken: [] }, "the editor was there, typing into a textarea (so VS Code's cut and paste land: 9.2d), and the canvas took none of its keys");
    } finally {
      await closeCanvas();
    }
  });

  test("E5, at the host: a file with unsaved changes is refused, a saved one answers text, version and prefix, and Save and Edit saves first", async () => {
    const folder = vscode.workspace.workspaceFolders![0].uri.fsPath;
    const file = path.join(folder, "nested.py");
    const original = fs.readFileSync(file, "utf8");
    const node = allNodes((await api.structure()).root).find((n) => n.id === "nested.py::Outer.Nested.method")!;
    assert.ok(node?.range, "a method of a nested class is in the fixture");
    const range = node.range;
    try {
      const document = await vscode.workspace.openTextDocument(vscode.Uri.file(file));
      const edit = new vscode.WorkspaceEdit();
      edit.insert(document.uri, new vscode.Position(document.lineCount, 0), "# unsaved\n");
      assert.ok(await vscode.workspace.applyEdit(edit));
      assert.ok(document.isDirty);

      assert.deepStrictEqual(await api.editStart("nested.py", range), { dirty: true }, "refused: nothing else is answered");
      assert.ok(document.isDirty, "and nothing was saved");

      // the same folder check as `code`: a path outside the opened folder is an error
      await assert.rejects(api.editStart("../outside.py", range), /outside the opened folder/);

      const saved = await api.editStart("nested.py", range, true);
      assert.ok(!document.isDirty, "Save and Edit saved the document");
      assert.ok("text" in saved);
      assert.deepStrictEqual(saved, { text: document.getText(new vscode.Range(range.start.line, range.start.character, range.end.line, range.end.character)), version: document.version, prefix: "        ", eol: "\n" });
      assert.match(saved.text, /^def method\(self\):/, "the range starts at `def`, the prefix is the indentation before it");

      // a clean document just answers
      assert.deepStrictEqual(await api.editStart("nested.py", range), saved);
    } finally {
      await vscode.commands.executeCommand("workbench.action.closeAllEditors");
      await restoreFixture(file, original);
    }
  });
});

suite("saving: Save writes the draft into the file (docs/17-edit-in-canvas.md step 9.3)", () => {
  const folder = () => vscode.workspace.workspaceFolders![0].uri.fsPath;
  const file = () => path.join(folder(), "nested.py");
  let original: string;

  suiteSetup(async () => {
    api = await vscode.extensions.getExtension("vnoc.vnoc")!.activate();
    original = fs.readFileSync(file(), "utf8");
  });

  suiteTeardown(async () => {
    await closeCanvas();
    await vscode.commands.executeCommand("workbench.action.revertAndCloseActiveEditor").then(undefined, () => {});
    await vscode.commands.executeCommand("workbench.action.closeAllEditors");
    await restoreFixture(file(), original);
  });

  /** Edit on the card, as the menu does, until this canvas says `editing`; the position in the page's notes from which to read what it says next. */
  async function startEditing(id: string): Promise<{ since: number; base: string }> {
    const node = allNodes((await api.structure()).root).find((n) => n.id === id)!;
    assert.ok(node?.range, `${id} is in the fixture`);
    await vscode.commands.executeCommand("vnoc.openCanvas");
    await until("the canvas tab", () => canvasTabs().length === 1);
    api.dispatch({ type: "expand", ids: ["nested.py"] });
    await waitFor("the file on disk to be what the fixture is", () => fs.readFileSync(file(), "utf8") === original);
    const since = api.editStates().length;
    const latest = () => api.editStates().slice(since).filter((p) => p.key === id).at(-1)?.state;
    for (let waited = 0; latest() !== "editing"; waited += 500) {
      assert.ok(waited < 30_000, `the card never said editing (last: ${latest()})`);
      api.dispatch({ type: "reveal", id, canvas: true });
      await vscode.commands.executeCommand("vnoc.editInCard", node);
      await sleep(500);
    }
    const got = await api.editStart("nested.py", node.range);
    assert.ok("text" in got);
    return { since, base: got.text };
  }
  const statesOf = (id: string, since: number) => api.editStates().slice(since).filter((p) => p.key === id).map((p) => p.state);

  test("a draft changed and saved: the file has the new text and not another byte moved, the file is saved, and the card shows the new text, read-only again", async () => {
    const id = "nested.py::decorated";
    try {
      const { since, base } = await startEditing(id);
      const shown = api.codeShown().length;
      api.viewCommand({ name: "setDraft", key: id, text: base.replace("return 9", "return 99") }); // what typing in the card makes
      api.viewCommand({ name: "save", key: id }); // what the button and ⌘S call
      // the whole file, compared with the expected string: the function has the new text and every other byte is as it was. Waited for, not read once: VS Code
      // writes a file by emptying it and then filling it, and a read in between sees nothing
      const expected = original.replace("return 9\n", "return 99\n");
      await waitFor("the file on disk to be the original with the one line changed", () => fs.readFileSync(file(), "utf8") === expected, 30_000);
      const document = vscode.workspace.textDocuments.find((d) => d.uri.fsPath === file());
      assert.ok(document && !document.isDirty, "E4: the file was not dirty before, so it is saved");

      // the card kept the saved text until the index moved on, then dropped the draft and read its code again
      await waitFor("the card to be reading again", () => statesOf(id, since).at(-1) === "reading", 30_000);
      assert.ok(statesOf(id, since).includes("saving"), `it was saving on the way (${statesOf(id, since)})`);
      for (let waited = 0; !api.codeShown().slice(shown).some((p) => p.key === id && p.text.includes("return 99")); waited += 500) {
        assert.ok(waited < 30_000, "the card never showed the new text");
        api.dispatch({ type: "reveal", id, canvas: true });
        await sleep(500);
      }
      assert.deepStrictEqual(api.webviewErrors(), []);
    } finally {
      await closeCanvas();
      await restoreFixture(file(), original);
    }
  });

  test("a function renamed in the draft: the file has the new name, and the index follows", async () => {
    const id = "nested.py::coro";
    try {
      const { base } = await startEditing(id);
      api.viewCommand({ name: "setDraft", key: id, text: base.replace("coro", "renamed_coro") });
      api.viewCommand({ name: "save", key: id });
      const expected = original.replace("async def coro():", "async def renamed_coro():");
      await waitFor("the file on disk to be the original with the function renamed", () => fs.readFileSync(file(), "utf8") === expected, 30_000);
      await waitFor("the tree to have the new name and not the old one", async () => {
        const ids = allNodes((await api.structure()).root).map((n) => n.id);
        return ids.includes("nested.py::renamed_coro") && !ids.includes("nested.py::coro");
      }, 30_000);
    } finally {
      await closeCanvas();
      await restoreFixture(file(), original);
    }
  });

  test("the file changed in a tab and saved since the draft started: Save is a conflict, and the file is as the tab left it", async () => {
    const id = "nested.py::decorated";
    try {
      const { since, base } = await startEditing(id);
      api.viewCommand({ name: "setDraft", key: id, text: base.replace("return 9", "return 99") });
      // the user, in a tab: the same function changed, and saved
      const document = await vscode.workspace.openTextDocument(vscode.Uri.file(file()));
      const edit = new vscode.WorkspaceEdit();
      const at = document.getText().indexOf("return 9\n");
      edit.replace(document.uri, new vscode.Range(document.positionAt(at), document.positionAt(at + "return 9".length)), "return 90");
      assert.ok(await vscode.workspace.applyEdit(edit));
      assert.ok(await document.save());
      const tab = fs.readFileSync(file(), "utf8");
      assert.strictEqual(tab, original.replace("return 9\n", "return 90\n"));

      api.viewCommand({ name: "save", key: id });
      await waitFor("the card to say conflict", () => statesOf(id, since).at(-1) === "conflict", 30_000);
      assert.strictEqual(fs.readFileSync(file(), "utf8"), tab, "nothing was written");
      assert.deepStrictEqual(api.webviewErrors(), []);
    } finally {
      await closeCanvas();
      await vscode.commands.executeCommand("workbench.action.closeAllEditors");
      await restoreFixture(file(), original);
    }
  });
});
