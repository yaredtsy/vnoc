import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";
import * as vscode from "vscode";

import { dedent } from "@vnoc/shared/indent";
import type { CodeResult, Draft, EditSaveParams, EditSaveResult, EditStartParams, EditStartResult, Page, Range, TreeNode } from "@vnoc/shared/messages";
import { callId } from "@vnoc/shared/calls.ts";
import type { Intent, Session } from "@vnoc/shared/session.ts";
import { allNodes } from "@vnoc/shared/tree";
import { canvasTabs, closeCanvas, sleep, until, waitFor } from "./util";

// Editing in the canvas (docs/17-edit-in-canvas.md), step 9.1: a card that shows the code of a file with unsaved changes says so. The suite cannot see the
// page, so it listens to what the page posts (`codeShown`: each time a card drew the code it read, and whether it showed the mark).

interface Api {
  structure(): Promise<{ root: TreeNode; git_root: string }>;
  codeShown(): { key: string; dirty: boolean; text: string }[];
  dirtyFiles(): string[];
  editStates(): { key: string; state: string; elsewhere: boolean; mark: string | null }[];
  draftsChip(): { count: number; label: string };
  session(): Session;
  callTrees(): Record<string, unknown>;
  viewCommand(data: { name: "edit" | "showCode" | "setDraft" | "save" | "saveFile" | "showDocPanel" | "docEdit" | "docSetDraft" | "docSave"; key?: string; text?: string }): void;
  request<T>(method: string, params?: object): Promise<T>;
  editSave(params: EditSaveParams, locate?: (key: string, document: { uri: string; version: number; text: string }) => Promise<Range | null>, ask?: (key: string) => Promise<boolean>): Promise<EditSaveResult>;
  draftCopy(): Draft[];
  draftCopyOf(page: Page): Draft[];
  feedDrafts(page: Page, drafts: Draft[]): void;
  editStartRequest(params: EditStartParams): Promise<EditStartResult>;
  setDraftNotify(notify?: (message: string, ...buttons: string[]) => Promise<string | undefined>): void;
  draftsAtStart(): Promise<void>;
  checkKeys(): void;
  keysChecked(): { found: boolean; target: string; taken: string[]; hosts: number }[];
  renderedPosts(): unknown[];
  codeOf(path: string, range: Range, saveFirst?: boolean): Promise<CodeResult>;
  dispatch(intent: Intent): void;
  webviewErrors(): string[];
}
let api: Api;

/**
 * Closing the canvas with a draft that has something to lose asks (docs/17 §7, 9.5c), and an answer that is not given leaves the draft in the host's copy for the next canvas. These suites close the
 * canvas all the time and want it as it was before: the draft is dropped. The 9.5c tests answer for themselves.
 */
const discardOnClose = () => api.setDraftNotify(async () => "Discard");

/**
 * Types `text` into a card, as typing does (docs/17 §5, 9.7): the first text that differs from the card's baseline starts the draft. The baseline arrives with the code, so it is asked until the
 * card says `editing`; the suite cannot press keys in a webview, and `setDraft` is what the editor's handler calls.
 */
async function typing(id: string, text: string): Promise<void> {
  const from = api.editStates().length;
  for (let waited = 0; api.editStates().slice(from).filter((p) => p.key === id).at(-1)?.state !== "editing"; waited += 250) {
    assert.ok(waited < 15_000, `typing never started a draft on ${id} (last: ${JSON.stringify(api.editStates().slice(from).filter((p) => p.key === id).at(-1))})`);
    api.viewCommand({ name: "setDraft", key: id, text });
    await sleep(250);
  }
}

const KEY = "nested.py::decorated";

/** The line ending of a fixture's text: git checks the fixtures out with CRLF on Windows, so the tests build what they expect from the fixture's own (the CRLF run: `VNOC_FIXTURE_EOL=crlf`). */
const eolOf = (text: string) => (text.includes("\r\n") ? "\r\n" : "\n");
const inEol = (text: string, eol: string) => text.replace(/\r?\n/g, eol);

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
    discardOnClose();
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

suite("editing: typing in a card's code starts a draft (docs/17-edit-in-canvas.md steps 9.2, 9.7)", () => {
  suiteSetup(async () => {
    api = await vscode.extensions.getExtension("vnoc.vnoc")!.activate();
    discardOnClose();
  });

  /** The card's code shown, as the menu's Edit in Card does, until the card says `reading`. Returns where in the page's notes to read what it says next. */
  async function codeShown(node: TreeNode): Promise<number> {
    await vscode.commands.executeCommand("vnoc.openCanvas");
    await until("the canvas tab", () => canvasTabs().length === 1);
    api.dispatch({ type: "expand", ids: ["nested.py"] });
    // only what this canvas says counts: the one the test before closed said the same for the same card
    const since = api.editStates().length;
    // an event sent before the new page listens is lost: ask until the card says so
    for (let waited = 0; api.editStates().slice(since).filter((p) => p.key === node.id).at(-1)?.state !== "reading"; waited += 500) {
      assert.ok(waited < 30_000, `the card never showed its code (last: ${JSON.stringify(api.editStates().slice(since).filter((p) => p.key === node.id).at(-1))})`);
      api.dispatch({ type: "reveal", id: node.id, canvas: true });
      await vscode.commands.executeCommand("vnoc.editInCard", node);
      await sleep(500);
    }
    return since;
  }

  test("the menu's Edit in Card shows the code, with no draft; typing starts the draft (`editing`, the bar counts it); typing back to the original drops it (`reading` again) (9.7)", async () => {
    const node = allNodes((await api.structure()).root).find((n) => n.id === KEY)!;
    try {
      const since = await codeShown(node);
      const states = () => api.editStates().slice(since).filter((p) => p.key === KEY).map((p) => p.state);
      const got = await api.codeOf("nested.py", node.range!);
      const start = dedent(got.text, got.prefix); // what Monaco shows of the baseline
      assert.deepStrictEqual(states().filter((st) => st !== "reading"), [], "the code is shown and no draft was started");
      assert.deepStrictEqual(api.draftsChip(), { count: 0, label: "" });

      await typing(KEY, start.replace("return 9", "return 99"));
      await waitFor("the bar to count the draft", () => api.draftsChip().count === 1, 10_000);
      assert.strictEqual(states().at(-1), "editing");

      // typed back to the original, or undone to it: nothing to lose, so no draft
      api.viewCommand({ name: "setDraft", key: KEY, text: start });
      await waitFor("the draft to be dropped", () => states().at(-1) === "reading", 10_000);
      await waitFor("the bar's count to go", () => api.draftsChip().count === 0, 10_000);
      await waitFor("the host's copy to be empty", () => api.draftCopy().length === 0, 10_000);

      // and typing again (a redo) starts a new one from a fresh baseline
      await typing(KEY, start.replace("return 9", "return 98"));
      assert.deepStrictEqual(api.webviewErrors(), [], "the page drew the card without crashing");
    } finally {
      await closeCanvas(); // the draft lives in the page: closing the tab drops it
    }
  });

  test("keys typed in a card's code stay in the code: the canvas takes none of them (Space, Backspace, F…) (9.2c)", async () => {
    const node = allNodes((await api.structure()).root).find((n) => n.id === KEY)!;
    try {
      await codeShown(node);
      // a keydown dispatched on the editor's own element runs every handler of the page: one that takes the key calls preventDefault (React Flow does for Space)
      // Monaco is made a moment after the card shows its code: ask again until the page finds its editor. The card's first "reading" comes before its editor exists, and a card that
      // left the page since (the canvas draws only what is on screen) must be shown again, as `codeShown` does
      for (let waited = 0; api.keysChecked().at(-1)?.found !== true; waited += 500) {
        assert.ok(
          waited < 30_000,
          `the page never found the editor (last answer: ${JSON.stringify(api.keysChecked().at(-1))}; the card's last state: ${JSON.stringify(api.editStates().filter((p) => p.key === KEY).at(-1))}; ` +
            `the last drawing: ${JSON.stringify(api.renderedPosts().at(-1))}; the canvas tab is active: ${canvasTabs()[0]?.isActive})`,
        );
        api.dispatch({ type: "reveal", id: node.id, canvas: true });
        await vscode.commands.executeCommand("vnoc.editInCard", node);
        api.checkKeys();
        await sleep(500);
      }
      const answer = api.keysChecked().at(-1)!;
      assert.deepStrictEqual({ found: answer.found, target: answer.target, taken: answer.taken }, { found: true, target: "TEXTAREA", taken: [] }, "the editor was there, typing into a textarea (so VS Code's cut and paste land: 9.2d), and the canvas took none of its keys");
      assert.ok(answer.hosts >= 1, `the page had a code area (${answer.hosts})`);
    } finally {
      await closeCanvas();
    }
  });

  test("a file with unsaved changes in a tab: the code is read-only and says Save to edit here, typing starts no draft, and the Save button saves the file so the card takes typing again (E5, 9.7)", async () => {
    const folder = vscode.workspace.workspaceFolders![0].uri.fsPath;
    const file = path.join(folder, "nested.py");
    const original = fs.readFileSync(file, "utf8");
    const eol = eolOf(original);
    const node = allNodes((await api.structure()).root).find((n) => n.id === KEY)!;
    const mark = (since: number) => api.editStates().slice(since).filter((p) => p.key === KEY).at(-1)?.mark;
    try {
      const since = await codeShown(node);
      assert.strictEqual(mark(since), null, "a saved file: no line");
      const got = await api.codeOf("nested.py", node.range!);
      const start = dedent(got.text, got.prefix);

      const document = await vscode.workspace.openTextDocument(vscode.Uri.file(file));
      const edit = new vscode.WorkspaceEdit();
      edit.insert(document.uri, new vscode.Position(document.lineCount, 0), "# unsaved, at the end\n");
      assert.ok(await vscode.workspace.applyEdit(edit));
      assert.ok(document.isDirty);
      await waitFor("the card to say Save to edit here", () => mark(since) === "saveFirst", 30_000);

      // typing is the editor's handler, which is read-only; the suite's `setDraft` is the same handler and finds no baseline
      api.viewCommand({ name: "setDraft", key: KEY, text: start.replace("return 9", "return 99") });
      await sleep(1500);
      assert.ok(!api.editStates().slice(since).some((p) => p.key === KEY && p.state === "editing"), "no draft was started in a file with unsaved changes");
      assert.deepStrictEqual(api.draftsChip(), { count: 0, label: "" });
      assert.ok(document.isDirty, "and nothing was saved");

      // the Save button: the document is saved, the card waits for the index, then takes typing
      api.viewCommand({ name: "saveFile", key: KEY });
      await waitFor("the document to be saved", () => !document.isDirty && fs.readFileSync(file, "utf8").endsWith(inEol("# unsaved, at the end\n", eol)), 30_000);
      await waitFor("the line to go", () => mark(since) === null, 30_000);
      await typing(KEY, start.replace("return 9", "return 99"));
      assert.deepStrictEqual(api.webviewErrors(), []);
    } finally {
      await closeCanvas();
      await vscode.commands.executeCommand("workbench.action.closeAllEditors");
      await restoreFixture(file, original);
    }
  });

  test("E5, at the host: the code answers its baseline (text, version, prefix, eol) and whether the file has unsaved changes; saveFirst saves it (9.7)", async () => {
    const folder = vscode.workspace.workspaceFolders![0].uri.fsPath;
    const file = path.join(folder, "nested.py");
    const original = fs.readFileSync(file, "utf8");
    const eol = eolOf(original);
    const node = allNodes((await api.structure()).root).find((n) => n.id === "nested.py::Outer.Nested.method")!;
    assert.ok(node?.range, "a method of a nested class is in the fixture");
    const range = node.range;
    try {
      const document = await vscode.workspace.openTextDocument(vscode.Uri.file(file));
      const text = document.getText(new vscode.Range(range.start.line, range.start.character, range.end.line, range.end.character));
      const clean = await api.codeOf("nested.py", range);
      assert.deepStrictEqual(clean, { text, version: document.version, prefix: "        ", eol, dirty: false });
      assert.match(clean.text, /^def method\(self\):/, "the range starts at `def`, the prefix is the indentation before it");

      const edit = new vscode.WorkspaceEdit();
      edit.insert(document.uri, new vscode.Position(document.lineCount, 0), "# unsaved\n");
      assert.ok(await vscode.workspace.applyEdit(edit));
      assert.ok(document.isDirty);
      const dirty = await api.codeOf("nested.py", range);
      assert.strictEqual(dirty.dirty, true, "the file has unsaved changes: the card is read-only");
      assert.strictEqual(dirty.text, text, "the text is still answered, so the card can show it");
      assert.ok(document.isDirty, "and nothing was saved");

      // the same folder check as before: a path outside the opened folder is an error
      await assert.rejects(api.codeOf("../outside.py", range), /outside the opened folder/);

      const saved = await api.codeOf("nested.py", range, true);
      assert.ok(!document.isDirty, "Save saved the document");
      assert.deepStrictEqual(saved, { text, version: document.version, prefix: "        ", eol, dirty: false });
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
  let eol: string;

  suiteSetup(async () => {
    api = await vscode.extensions.getExtension("vnoc.vnoc")!.activate();
    discardOnClose();
    original = fs.readFileSync(file(), "utf8");
    eol = eolOf(original);
  });

  suiteTeardown(async () => {
    await closeCanvas();
    await vscode.commands.executeCommand("workbench.action.revertAndCloseActiveEditor").then(undefined, () => {});
    await vscode.commands.executeCommand("workbench.action.closeAllEditors");
    await restoreFixture(file(), original);
  });

  /**
   * Waits until the index shows the function where the fixture has it. The test before may have deleted it, or moved it a line, and put the file back: the index
   * reads that a moment later, and a draft started on the way would have the wrong lines.
   */
  async function inFixturePlace(id: string): Promise<TreeNode> {
    const line = (needle: string) => original.split("\n").findIndex((l) => l.includes(needle));
    const start: Record<string, number> = { "nested.py::decorated": line("@staticmethod"), "nested.py::coro": line("async def coro") };
    let found: TreeNode | undefined;
    await waitFor(`the index to have ${id} where the fixture has it`, async () => {
      found = allNodes((await api.structure()).root).find((n) => n.id === id);
      return found?.range?.start.line === start[id];
    }, 30_000);
    return found!;
  }

  /** The card's code shown, as the menu's Edit in Card does, until this canvas says `reading` and has the card's baseline; the position in the page's notes from which to read what it says next. */
  async function startEditing(id: string): Promise<{ since: number; base: string }> {
    const node = await inFixturePlace(id);
    assert.ok(node?.range, `${id} is in the fixture`);
    await vscode.commands.executeCommand("vnoc.openCanvas");
    await until("the canvas tab", () => canvasTabs().length === 1);
    api.dispatch({ type: "expand", ids: ["nested.py"] });
    await waitFor("the file on disk to be what the fixture is", () => fs.readFileSync(file(), "utf8") === original);
    const since = api.editStates().length;
    const shown = api.codeShown().length;
    const latest = () => api.editStates().slice(since).filter((p) => p.key === id).at(-1)?.state;
    // the card posts `codeShown` after it has set its baseline: from then on typing starts a draft
    for (let waited = 0; latest() !== "reading" || !api.codeShown().slice(shown).some((p) => p.key === id); waited += 500) {
      assert.ok(waited < 30_000, `the card never showed its code (last: ${latest()})`);
      api.dispatch({ type: "reveal", id, canvas: true });
      await vscode.commands.executeCommand("vnoc.editInCard", node);
      await sleep(500);
    }
    const got = await api.codeOf("nested.py", node.range);
    return { since, base: got.text };
  }
  const statesOf = (id: string, since: number) => api.editStates().slice(since).filter((p) => p.key === id).map((p) => p.state);

  test("a draft changed and saved: the file has the new text and not another byte moved, the file is saved, and the card shows the new text, read-only again", async () => {
    const id = "nested.py::decorated";
    try {
      const { since, base } = await startEditing(id);
      const shown = api.codeShown().length;
      await typing(id, base.replace("return 9", "return 99")); // what typing in the card makes
      api.viewCommand({ name: "save", key: id }); // what the button and ⌘S call
      // the whole file, compared with the expected string: the function has the new text and every other byte is as it was. Waited for, not read once: VS Code
      // writes a file by emptying it and then filling it, and a read in between sees nothing
      const expected = original.replace(inEol("return 9\n", eol), inEol("return 99\n", eol));
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
      await typing(id, base.replace("coro", "renamed_coro"));
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

  test("a line added at the top of the file in a tab and saved: Save writes the draft at the function's new place, and not another byte moves (§11 c)", async () => {
    const id = "nested.py::decorated";
    try {
      const { base } = await startEditing(id);
      await typing(id, base.replace("return 9", "return 99"));
      // the user, in a tab: a comment added above everything, and saved
      const document = await vscode.workspace.openTextDocument(vscode.Uri.file(file()));
      const edit = new vscode.WorkspaceEdit();
      edit.insert(document.uri, new vscode.Position(0, 0), "# top\n");
      assert.ok(await vscode.workspace.applyEdit(edit));
      assert.ok(await document.save());

      api.viewCommand({ name: "save", key: id });
      const expected = "# top" + eol + original.replace(inEol("return 9\n", eol), inEol("return 99\n", eol));
      await waitFor("the file to have the comment and the draft, in the function's new place", () => fs.readFileSync(file(), "utf8") === expected, 30_000);
      assert.deepStrictEqual(api.webviewErrors(), []);
    } finally {
      await closeCanvas();
      await restoreFixture(file(), original);
    }
  });

  test("the file changed in a tab and saved since the draft started: Save is a conflict, and the file is as the tab left it (§11 d)", async () => {
    const id = "nested.py::decorated";
    try {
      const { since, base } = await startEditing(id);
      await typing(id, base.replace("return 9", "return 99"));
      // the user, in a tab: the same function changed, and saved
      const document = await vscode.workspace.openTextDocument(vscode.Uri.file(file()));
      const edit = new vscode.WorkspaceEdit();
      const at = document.getText().indexOf(inEol("return 9\n", eol));
      edit.replace(document.uri, new vscode.Range(document.positionAt(at), document.positionAt(at + "return 9".length)), "return 90");
      assert.ok(await vscode.workspace.applyEdit(edit));
      assert.ok(await document.save());
      const tab = fs.readFileSync(file(), "utf8");
      assert.strictEqual(tab, original.replace(inEol("return 9\n", eol), inEol("return 90\n", eol)));

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

  test("an edit elsewhere in a tab that is not saved: Save writes into the buffer and not the disk, the tab stays dirty (§11 e)", async () => {
    const id = "nested.py::decorated";
    try {
      const { since, base } = await startEditing(id);
      await typing(id, base.replace("return 9", "return 99"));
      const document = await vscode.workspace.openTextDocument(vscode.Uri.file(file()));
      const edit = new vscode.WorkspaceEdit();
      edit.insert(document.uri, new vscode.Position(0, 0), "# top, not saved\n");
      assert.ok(await vscode.workspace.applyEdit(edit));
      assert.ok(document.isDirty);

      api.viewCommand({ name: "save", key: id });
      await waitFor("the draft to be in the buffer", () => document.getText().includes("return 99"), 30_000);
      assert.strictEqual(document.getText(), "# top, not saved" + eol + original.replace(inEol("return 9\n", eol), inEol("return 99\n", eol)), "the draft is in the buffer, at the function's new place, and the user's own edit is still there");
      assert.ok(document.isDirty, "E4: the user's unsaved work is never saved for them");
      assert.strictEqual(fs.readFileSync(file(), "utf8"), original, "nothing reached the disk");
      await waitFor("the card to drop the draft at once", () => statesOf(id, since).at(-1) === "reading", 5_000);
    } finally {
      await closeCanvas();
      await restoreFixture(file(), original);
    }
  });

  test("the function is deleted in a tab and saved: Save says gone, and writes nothing", async () => {
    const id = "nested.py::decorated";
    try {
      const { since, base } = await startEditing(id);
      await typing(id, base.replace("return 9", "return 99"));
      const node = allNodes((await api.structure()).root).find((n) => n.id === id)!;
      const document = await vscode.workspace.openTextDocument(vscode.Uri.file(file()));
      const edit = new vscode.WorkspaceEdit();
      edit.delete(document.uri, new vscode.Range(node.range!.start.line, node.range!.start.character, node.range!.end.line, node.range!.end.character));
      assert.ok(await vscode.workspace.applyEdit(edit));
      assert.ok(await document.save());
      const tab = fs.readFileSync(file(), "utf8");
      assert.ok(!tab.includes("def decorated"));

      api.viewCommand({ name: "save", key: id });
      await waitFor("the card to say gone", () => statesOf(id, since).at(-1) === "gone", 30_000);
      assert.strictEqual(fs.readFileSync(file(), "utf8"), tab, "nothing was written");
      assert.deepStrictEqual(api.webviewErrors(), []);
    } finally {
      await closeCanvas();
      await restoreFixture(file(), original);
    }
  });

  /** What the card sends for a draft of `id` whose text has `from` replaced by `to`. */
  async function draftParams(id: string, from: string, to: string): Promise<EditSaveParams & { range: Range }> {
    const node = await inFixturePlace(id);
    const got = await api.codeOf("nested.py", node.range!);
    return { key: id, path: "nested.py", range: node.range!, version: got.version, base: got.text, prefix: got.prefix, eol: got.eol, text: got.text.replace(from, to) };
  }

  test("the core's answer comes after the document moved on: a conflict, and nothing is written (§11 g, §6's re-check)", async () => {
    try {
      const params = await draftParams("nested.py::decorated", "return 9", "return 99");
      const document = await vscode.workspace.openTextDocument(vscode.Uri.file(file()));
      const top = new vscode.WorkspaceEdit();
      top.insert(document.uri, new vscode.Position(0, 0), "# one\n");
      assert.ok(await vscode.workspace.applyEdit(top)); // the version moved: the host asks where the function is
      let asked = 0;
      // a core that finds the function (one line down) but lets the document change before it says so
      const answer = await api.editSave(params, async (_key, sent) => {
        asked++;
        const during = new vscode.WorkspaceEdit();
        during.insert(document.uri, new vscode.Position(0, 0), "# two\n");
        assert.ok(await vscode.workspace.applyEdit(during));
        assert.ok(document.version > sent.version);
        return { start: { line: params.range.start.line + 1, character: params.range.start.character }, end: { line: params.range.end.line + 1, character: params.range.end.character } };
      });
      assert.strictEqual(asked, 1);
      assert.deepStrictEqual(answer, { conflict: true });
      assert.strictEqual(document.getText(), "# two" + eol + "# one" + eol + original, "only the two edits of the test: the draft was not written");
      assert.strictEqual(fs.readFileSync(file(), "utf8"), original, "and nothing reached the disk");
    } finally {
      await restoreFixture(file(), original);
    }
  });

  test("Save twice on the same draft: the second answers saved and writes nothing", async () => {
    try {
      const params = await draftParams("nested.py::decorated", "return 9", "return 99");
      assert.deepStrictEqual(await api.editSave(params), { saved: true });
      const expected = original.replace(inEol("return 9\n", eol), inEol("return 99\n", eol));
      await waitFor("the first Save to be on disk", () => fs.readFileSync(file(), "utf8") === expected, 30_000);
      const document = vscode.workspace.textDocuments.find((d) => d.uri.fsPath === file())!;
      const version = document.version;
      // the page's Save again, or a Save the page lost with a reload: the version moved, the core finds the function, and it holds what Save would write
      assert.deepStrictEqual(await api.editSave(params), { saved: true });
      assert.strictEqual(document.version, version, "the document was not edited");
      assert.strictEqual(fs.readFileSync(file(), "utf8"), expected);
    } finally {
      await restoreFixture(file(), original);
    }
  });

  // ── 9.5c: the host keeps a copy of the drafts, and asks right after the canvas is closed ─────────────────────────────

  /** Answers the notifications of the copy with `answer` (`undefined`: dismissed), and records each one. */
  function answering(answer: string | undefined) {
    const shown: { message: string; buttons: string[] }[] = [];
    api.setDraftNotify(async (message, ...buttons) => (shown.push({ message, buttons }), answer));
    return shown;
  }

  /** A draft with typed text, and the host's copy holding it (typing is sent 500 ms after the last keystroke). Returns the text. */
  async function typedDraft(id: string): Promise<string> {
    const { base } = await startEditing(id);
    const typedText = base.replace("return 9", "return 99");
    await typing(id, typedText);
    await waitFor("the host's copy to hold the draft", () => api.draftCopy().some((d) => d.key === id && d.text === typedText && d.state === "editing"), 10_000);
    return typedText;
  }

  /** Closes the canvas tab (not hides it). Not `closeCanvas`: an answer that reopens the canvas would race its wait for the tab to be gone. */
  const closeTab = () => vscode.window.tabGroups.close(canvasTabs());

  test("a draft, the canvas closed: a notification at once; Reopen Canvas brings the draft back, editing, with its text (9.5c)", async () => {
    const id = "nested.py::decorated";
    const shown = answering("Reopen Canvas");
    try {
      const typedText = await typedDraft(id);
      await closeTab();
      await waitFor("the notification", () => shown.length === 1, 10_000);
      assert.deepStrictEqual(shown[0], { message: "1 unsaved draft from the canvas", buttons: ["Save All", "Reopen Canvas", "Discard"] });
      // Reopen: the canvas opens, and its page asks the host for the drafts
      await until("the canvas to open again", () => canvasTabs().length === 1);
      const since = api.editStates().length;
      const latest = () => api.editStates().slice(since).filter((p) => p.key === id).at(-1)?.state;
      for (let waited = 0; latest() !== "editing"; waited += 500) {
        assert.ok(waited < 30_000, `the card never said editing after the canvas was reopened (last: ${latest()})`);
        api.dispatch({ type: "expand", ids: ["nested.py"] });
        api.dispatch({ type: "reveal", id, canvas: true });
        await sleep(500);
      }
      // with its text: Save writes exactly what was typed before the canvas was closed
      api.viewCommand({ name: "save", key: id });
      await waitFor("the file to have the draft that was typed before the close", () => fs.readFileSync(file(), "utf8") === original.replace(inEol("return 9\n", eol), inEol("return 99\n", eol)), 30_000);
      assert.ok(typedText.includes("return 99"));
      await waitFor("the copy to be empty: the draft was saved", () => api.draftCopy().length === 0, 10_000);
      assert.strictEqual(shown.length, 1, "one notification, not one per step");
      assert.deepStrictEqual(api.webviewErrors(), []);
    } finally {
      discardOnClose();
      await closeCanvas();
      await restoreFixture(file(), original);
    }
  });

  test("a draft, the canvas closed, Save All: the file on disk has the draft, and the copy is empty (9.5c)", async () => {
    const id = "nested.py::decorated";
    const shown = answering("Save All");
    try {
      await typedDraft(id);
      await closeTab();
      await waitFor("the file to have the draft", () => fs.readFileSync(file(), "utf8") === original.replace(inEol("return 9\n", eol), inEol("return 99\n", eol)), 30_000);
      await waitFor("the copy to be empty", () => api.draftCopy().length === 0, 10_000);
      await sleep(500);
      assert.strictEqual(shown.length, 1, "nothing could not be saved: no second notification");
      assert.strictEqual(shown[0].message, "1 unsaved draft from the canvas");
      const document = vscode.workspace.textDocuments.find((d) => d.uri.fsPath === file());
      assert.ok(document && !document.isDirty, "the file was saved (E4: it was not dirty before)");
    } finally {
      discardOnClose();
      await closeCanvas();
      await restoreFixture(file(), original);
    }
  });

  test("a draft, the canvas closed, Discard: the canvas opens later with no draft (9.5c)", async () => {
    const id = "nested.py::decorated";
    const shown = answering("Discard");
    try {
      await typedDraft(id);
      await closeTab();
      await waitFor("the notification", () => shown.length === 1, 10_000);
      await waitFor("the copy to be empty", () => api.draftCopy().length === 0, 10_000);
      await vscode.commands.executeCommand("vnoc.openCanvas");
      await until("the canvas tab", () => canvasTabs().length === 1);
      const since = api.editStates().length;
      api.dispatch({ type: "expand", ids: ["nested.py"] });
      for (let i = 0; i < 6; i++) {
        api.dispatch({ type: "reveal", id, canvas: true });
        await sleep(500);
      }
      assert.ok(!api.editStates().slice(since).some((p) => p.key === id && p.state === "editing"), "no card is editing");
      assert.deepStrictEqual(api.draftsChip(), { count: 0, label: "" }, "and the bar counts nothing");
      assert.deepStrictEqual(api.draftCopy(), []);
      assert.strictEqual(fs.readFileSync(file(), "utf8"), original, "nothing was written");
    } finally {
      discardOnClose();
      await closeCanvas();
      await restoreFixture(file(), original);
    }
  });

  test("the notification dismissed: the copy stays; at start the host asks again, with Reopen Canvas and Discard (9.5c)", async () => {
    const id = "nested.py::decorated";
    const dismissed = answering(undefined);
    try {
      await typedDraft(id);
      await closeTab();
      assert.deepStrictEqual(api.draftsChip(), { count: 0, label: "" }, "the canvas is closed: no bar, no chip");
      await waitFor("the notification", () => dismissed.length === 1, 10_000);
      await sleep(300);
      assert.strictEqual(api.draftCopy().length, 1, "dismissed or ignored: the copy stays, and the draft comes back the next time the canvas opens");
      assert.strictEqual(fs.readFileSync(file(), "utf8"), original, "nothing was written");
      // what the extension does when it starts (a window reload, a new window): no canvas is open, so it asks
      const atStart = answering("Discard");
      await api.draftsAtStart();
      assert.deepStrictEqual(atStart, [{ message: "1 unsaved draft from the canvas", buttons: ["Reopen Canvas", "Discard"] }]);
      assert.deepStrictEqual(api.draftCopy(), [], "Discard clears the copy");
      // nothing counts any more: it does not ask
      const none = answering("Discard");
      await api.draftsAtStart();
      assert.deepStrictEqual(none, []);
    } finally {
      discardOnClose();
      await closeCanvas();
      await restoreFixture(file(), original);
    }
  });
});

suite("one draft per function: the bar counts the drafts (docs/17-edit-in-canvas.md step 9.5a)", () => {
  const folder = () => vscode.workspace.workspaceFolders![0].uri.fsPath;
  const file = () => path.join(folder(), "edit_calls.py");
  const SOURCE = "def helper():\n    return 1\n\n\ndef caller():\n    helper()\n    return 2\n";
  const HELPER = "edit_calls.py::helper";
  const CALLER = "edit_calls.py::caller";

  suiteSetup(async () => {
    api = await vscode.extensions.getExtension("vnoc.vnoc")!.activate();
    discardOnClose();
    fs.writeFileSync(file(), SOURCE);
  });

  suiteTeardown(async () => {
    await closeCanvas();
    api.dispatch({ type: "collapseAll" });
    fs.rmSync(file(), { force: true });
  });

  test("while the structure card holds a function's draft, its call card is read-only and says so, typing there starts nothing, and the bar counts the draft while it is unsaved (§7, 9.7)", async () => {
    let helper: TreeNode | undefined;
    await waitFor("the index to have both functions", async () => {
      const all = allNodes((await api.structure()).root);
      helper = all.find((n) => n.id === HELPER);
      return helper?.range !== undefined && all.some((n) => n.id === CALLER);
    }, 30_000);
    const base = await api.codeOf("edit_calls.py", helper!.range!);
    const start = dedent(base.text, base.prefix);
    const call = callId(CALLER, [HELPER]);
    const last = (id: string) => api.editStates().filter((p) => p.key === id).at(-1);

    await vscode.commands.executeCommand("vnoc.openCanvas");
    await until("the canvas tab", () => canvasTabs().length === 1);
    api.dispatch({ type: "select", id: null });
    api.dispatch({ type: "expand", ids: ["edit_calls.py", CALLER] }); // the structure cards, and the call card under `caller`
    await waitFor("the call tree of caller", () => api.callTrees()[CALLER] !== undefined, 30_000);
    assert.deepStrictEqual(api.draftsChip(), { count: 0, label: "" }, "no draft, no chip");

    // the structure card shows its code (an event sent before the page listens is lost, so ask until it says so)
    for (let waited = 0; last(HELPER)?.state !== "reading"; waited += 500) {
      assert.ok(waited < 30_000, `the structure card never showed its code (last: ${JSON.stringify(last(HELPER))})`);
      api.dispatch({ type: "reveal", id: HELPER, canvas: true });
      api.viewCommand({ name: "edit", key: HELPER });
      await sleep(500);
    }
    assert.deepStrictEqual(api.draftsChip(), { count: 0, label: "" }, "the code is shown and nothing was typed: no draft, nothing to count");

    await typing(HELPER, start.replace("return 1", "return 11"));
    await waitFor("the bar to count the draft", () => api.draftsChip().count === 1, 10_000);
    assert.strictEqual(api.draftsChip().label, "1 unsaved draft");

    // the call card of the same function shows its code: read-only, and says another card holds the draft
    for (let waited = 0; last(call)?.elsewhere !== true; waited += 500) {
      assert.ok(waited < 30_000, `the call card never said "being edited in another card" (last: ${JSON.stringify(last(call))})`);
      api.dispatch({ type: "reveal", id: CALLER, canvas: true });
      api.viewCommand({ name: "showCode", key: call });
      await sleep(500);
    }
    assert.deepStrictEqual([last(call)!.state, last(call)!.mark], ["reading", "elsewhere"], "the call card shows its code, no draft, and the line");
    assert.strictEqual(last(HELPER)!.state, "editing", "the first card is still editing");

    // typing there starts nothing: the first card's draft is untouched, and still one
    api.viewCommand({ name: "setDraft", key: call, text: start.replace("return 1", "return 12") });
    await sleep(1500);
    assert.strictEqual(api.draftsChip().count, 1, "still one draft");
    assert.ok(api.draftCopy().every((d) => d.card === HELPER && d.text === start.replace("return 1", "return 11")), "the draft is the structure card's, with its own text");

    // typed back to where it started: nothing to lose, the draft goes, the bar's count goes, and so does the call card's line
    api.viewCommand({ name: "setDraft", key: HELPER, text: start });
    await waitFor("the bar's count to go", () => api.draftsChip().count === 0, 10_000);
    await waitFor("the call card's line to go", () => last(call)?.elsewhere === false && last(call)?.mark === null, 10_000);
    assert.deepStrictEqual(api.webviewErrors(), []);
  });
});

suite("Save asks before it removes a function's ID (docs/17-edit-in-canvas.md step 9.5b)", () => {
  const folder = () => vscode.workspace.workspaceFolders![0].uri.fsPath;
  const file = () => path.join(folder(), "edit_id.py");
  const ID = "0Q8V5W4T2F7KX9M3B6D1RZ4H7K";
  const SOURCE = `def keeper():\n    """Keeps its ID.\n\n    ID: ${ID}\n    """\n    return 1\n`;
  let node: TreeNode;

  suiteSetup(async () => {
    api = await vscode.extensions.getExtension("vnoc.vnoc")!.activate();
    discardOnClose();
    fs.writeFileSync(file(), SOURCE);
    await waitFor("the index to have the function", async () => {
      const found = allNodes((await api.structure()).root).find((n) => n.id === "edit_id.py::keeper");
      if (found) node = found;
      return !!found?.range && found.range.end.line > 3;
    }, 30_000);
  });

  suiteTeardown(async () => {
    await vscode.commands.executeCommand("workbench.action.closeAllEditors");
    fs.rmSync(file(), { force: true });
  });

  /** What the card sends for a draft of `keeper` whose text has `from` replaced by `to`. */
  async function params(from: string, to: string): Promise<EditSaveParams> {
    const got = await api.codeOf("edit_id.py", node.range!);
    assert.ok(got.text.includes(from), `the draft's start has ${JSON.stringify(from)}`);
    return { key: node.id, path: "edit_id.py", range: node.range!, version: got.version, base: got.text, prefix: got.prefix, eol: got.eol, text: got.text.replace(from, to) };
  }
  /** The file is put back through VS Code, as the other suites do: a document held open that is written behind its back is older than the disk. */
  async function putBack() {
    await restoreFixture(file(), SOURCE);
    await waitFor("the file to be the fixture", () => fs.readFileSync(file(), "utf8") === SOURCE);
  }

  test("a draft that deletes the ID line makes the host ask: Cancel writes nothing, Save Anyway writes the draft (§11 e, first half)", async () => {
    try {
      const idLine = `\n    ID: ${ID}\n`;
      const asked: string[] = [];
      const cancel = await api.editSave(await params(idLine, "\n"), undefined, async (key) => (asked.push(key), false));
      assert.deepStrictEqual(cancel, { cancelled: true });
      assert.deepStrictEqual(asked, [node.id], "asked once, about this function");
      assert.strictEqual(fs.readFileSync(file(), "utf8"), SOURCE, "Cancel: the file is unchanged");
      const document = vscode.workspace.textDocuments.find((d) => d.uri.fsPath === file());
      assert.ok(!document || (document.getText() === SOURCE && !document.isDirty), "and so is the document");

      const anyway = await api.editSave(await params(idLine, "\n"), undefined, async (key) => (asked.push(key), true));
      assert.deepStrictEqual(anyway, { saved: true });
      assert.strictEqual(asked.length, 2);
      const expected = SOURCE.replace(idLine, "\n");
      await waitFor("the file on disk to be the draft", () => fs.readFileSync(file(), "utf8") === expected, 30_000);
    } finally {
      await putBack();
    }
  });

  test("a draft that keeps the ID is not asked, and neither is one that changes it to a text that holds it elsewhere", async () => {
    try {
      let asked = 0;
      const ask = async () => (asked++, false);
      assert.deepStrictEqual(await api.editSave(await params("return 1", "return 2"), undefined, ask), { saved: true });
      assert.strictEqual(asked, 0, "the ID is kept: no question");
      await waitFor("the file to have the edit", () => fs.readFileSync(file(), "utf8") === SOURCE.replace("return 1", "return 2"), 30_000);
      await putBack();
      // the ID line moved up into the first line of the docstring: not a loss
      const moved = await params(`Keeps its ID.\n\n    ID: ${ID}\n`, `Keeps its ID. ID: ${ID}\n`);
      assert.deepStrictEqual(await api.editSave(moved, undefined, ask), { saved: true });
      assert.strictEqual(asked, 0, "the ID moved: no question");
    } finally {
      await putBack();
    }
  });

  test("a draft that changes the ID asks; and the question is asked only in the row that writes: a conflict is answered without it", async () => {
    try {
      const swapped = await params(ID, "1Q8V5W4T2F7KX9M3B6D1RZ4H7K");
      let asked = 0;
      const document = await vscode.workspace.openTextDocument(vscode.Uri.file(file()));
      // the same function changed in the document since the draft started: a conflict, and nobody is asked about an ID that will not be written
      const edit = new vscode.WorkspaceEdit();
      edit.replace(document.uri, new vscode.Range(5, 0, 5, 12), "    return 90");
      assert.ok(await vscode.workspace.applyEdit(edit));
      assert.deepStrictEqual(await api.editSave(swapped, undefined, async () => (asked++, true)), { conflict: true });
      assert.strictEqual(asked, 0);
      await putBack();
      assert.deepStrictEqual(await api.editSave(await params(ID, "1Q8V5W4T2F7KX9M3B6D1RZ4H7K"), undefined, async () => (asked++, false)), { cancelled: true });
      assert.strictEqual(asked, 1, "a changed ID asks");
    } finally {
      await putBack();
    }
  });

  test("the document moves while the question is open: after Save Anyway the rows run again from the top, and the draft is written at the function's new place", async () => {
    try {
      const draft = await params(`\n    ID: ${ID}\n`, "\n");
      const document = await vscode.workspace.openTextDocument(vscode.Uri.file(file()));
      let asked = 0;
      const answer = await api.editSave(draft, undefined, async () => {
        asked++;
        const during = new vscode.WorkspaceEdit();
        during.insert(document.uri, new vscode.Position(0, 0), "# added while the question was open\n");
        assert.ok(await vscode.workspace.applyEdit(during));
        assert.ok(document.version > draft.version);
        return true;
      });
      assert.strictEqual(asked, 1, "the second pass does not ask again");
      assert.deepStrictEqual(answer, { written: true }, "the user's own edit is unsaved, so the draft is in the buffer and not on disk (E4)");
      assert.strictEqual(document.getText(), "# added while the question was open\n" + SOURCE.replace(`\n    ID: ${ID}\n`, "\n"), "the draft is at the function's new place, nothing else moved");
      assert.strictEqual(fs.readFileSync(file(), "utf8"), SOURCE, "nothing reached the disk");
    } finally {
      await putBack();
    }
  });
});

suite("a doc's Markdown saves through the same rows (docs/17-edit-in-canvas.md step 9.6a)", () => {
  const ID = "0Q8V5W4T2F7KX9M3B6D1RZ4H7K";
  const DOC = `---\nnode: ${ID}\nhint: src/orders.py::parse_order\n---\n# Why we parse twice\n\nThe upstream API sends  \nthe same field twice. ID: not-an-ID-line\n\n    an indented code block\n\nLast line\n`;
  let dir: string;
  let file: string;
  let madeVnoc = false;

  suiteSetup(async () => {
    api = await vscode.extensions.getExtension("vnoc.vnoc")!.activate();
    discardOnClose();
    const root = (await api.structure()).git_root;
    madeVnoc = !fs.existsSync(path.join(root, ".vnoc"));
    dir = path.join(root, ".vnoc", "nodes", "7K", ID);
    file = path.join(dir, "overview.md");
    fs.mkdirSync(dir, { recursive: true });
  });

  suiteTeardown(async () => {
    await vscode.commands.executeCommand("workbench.action.closeAllEditors");
    fs.rmSync(madeVnoc ? path.join(path.dirname(path.dirname(path.dirname(dir))), ".vnoc") : dir, { recursive: true, force: true });
  });

  /** The doc on disk as `text`, through VS Code if it holds the file (a file written behind its back leaves the document older than the disk). */
  async function putBack(text = DOC) {
    const open = vscode.workspace.textDocuments.find((d) => d.uri.fsPath === file);
    if (open && open.getText() !== text) {
      const edit = new vscode.WorkspaceEdit();
      edit.replace(open.uri, new vscode.Range(0, 0, open.lineCount, 0), text);
      await vscode.workspace.applyEdit(edit);
    }
    if (open?.isDirty) await open.save();
    if (!fs.existsSync(file) || fs.readFileSync(file, "utf8") !== text) fs.writeFileSync(file, text);
    await waitFor("the doc to be as put back", () => fs.readFileSync(file, "utf8") === text);
  }

  /** What the panel sends for a draft of the doc whose text has `from` replaced by `to`. */
  async function docParams(from: string, to: string, page: Page = "docs"): Promise<EditSaveParams> {
    const got = await api.editStartRequest({ kind: "doc", path: file, page });
    assert.ok("text" in got, `Edit on the doc answered ${JSON.stringify(got)}`);
    assert.ok(got.text.includes(from), `the doc has ${JSON.stringify(from)}`);
    return { kind: "doc", key: file, path: file, range: got.range!, version: got.version, base: got.text, prefix: got.prefix, eol: got.eol, text: got.text.replace(from, to) };
  }

  test("Edit on a doc answers its whole text, the version, no prefix and the whole file as the range", async () => {
    try {
      await putBack();
      const got = await api.editStartRequest({ kind: "doc", path: file, page: "docs" });
      assert.ok("text" in got);
      assert.deepStrictEqual({ ...got, range: undefined }, { text: DOC, version: got.version, prefix: "", eol: "\n", range: undefined });
      assert.deepStrictEqual(got.range, { start: { line: 0, character: 0 }, end: { line: DOC.split("\n").length - 1, character: 0 } });
      // the path is checked as the panel's own reads are: only a Markdown file inside `.vnoc/nodes/`
      await assert.rejects(api.editStartRequest({ kind: "doc", path: path.join(path.dirname(path.dirname(path.dirname(path.dirname(dir)))), "nested.py"), page: "docs" }), /is not a V-NOC doc/);
      await assert.rejects(api.editSave({ ...(await docParams("Last line", "Last line")), path: path.join(dir, "..", "..", "..", "..", "nested.py") }), /is not a V-NOC doc/);
    } finally {
      await putBack();
    }
  });

  test("a doc's text edited and saved: the .md on disk changed byte for byte, the file saved, and no question about an ID (§8)", async () => {
    try {
      await putBack();
      // `ID: not-an-ID-line` is a line the question would be asked about in a function; in a doc it is text
      const params = await docParams("the same field twice. ID: not-an-ID-line", "the same field twice.");
      let asked = 0;
      const answer = await api.editSave(params, undefined, async () => (asked++, false));
      assert.deepStrictEqual(answer, { saved: true });
      assert.strictEqual(asked, 0, "a Markdown file has no ID line to take away");
      const expected = DOC.replace("the same field twice. ID: not-an-ID-line", "the same field twice.");
      await waitFor("the .md on disk to be the edited text", () => fs.readFileSync(file, "utf8") === expected, 30_000);
      const document = vscode.workspace.textDocuments.find((d) => d.uri.fsPath === file);
      assert.ok(document && !document.isDirty, "E4: it was not dirty before, so it is saved");
    } finally {
      await putBack();
    }
  });

  test("a CRLF doc comes back with CRLF, whatever line endings Monaco's text has, and a second Save of the same draft writes nothing (already saved)", async () => {
    // a file nothing has opened: a document already open keeps its own line ending, whatever is written into it
    const crlfFile = path.join(dir, "crlf.md");
    const crlf = DOC.replace(/\n/g, "\r\n");
    fs.writeFileSync(crlfFile, crlf);
    try {
      const got = await api.editStartRequest({ kind: "doc", path: crlfFile, page: "docs" });
      assert.ok("text" in got);
      assert.strictEqual(got.eol, "\r\n");
      const params: EditSaveParams = { kind: "doc", key: crlfFile, path: crlfFile, range: got.range!, version: got.version, base: got.text, prefix: "", eol: got.eol, text: got.text.replace("Last line", "The last line") };
      // Monaco's text, as it may hold it: its own line endings do not matter
      assert.deepStrictEqual(await api.editSave({ ...params, text: params.text.replace(/\r\n/g, "\n") }), { saved: true });
      await waitFor("the file to have the edit and CRLF", () => fs.readFileSync(crlfFile, "utf8") === crlf.replace("Last line", "The last line"), 30_000);
      const document = vscode.workspace.textDocuments.find((d) => d.uri.fsPath === crlfFile)!;
      const version = document.version;
      assert.deepStrictEqual(await api.editSave(params), { saved: true }, "a second Save of the same draft");
      assert.strictEqual(document.version, version, "nothing was written");
    } finally {
      await vscode.commands.executeCommand("workbench.action.closeAllEditors");
      fs.rmSync(crlfFile, { force: true });
    }
  });

  test("the doc changed in a tab and saved since the draft started: Save is a conflict, and the file is as the tab left it", async () => {
    try {
      await putBack();
      const params = await docParams("Last line", "Last line, edited in the panel");
      const document = await vscode.workspace.openTextDocument(vscode.Uri.file(file));
      const edit = new vscode.WorkspaceEdit();
      const at = document.getText().indexOf("Last line");
      edit.replace(document.uri, new vscode.Range(document.positionAt(at), document.positionAt(at + "Last line".length)), "Last line, edited in the tab");
      assert.ok(await vscode.workspace.applyEdit(edit));
      assert.ok(await document.save());
      const tab = fs.readFileSync(file, "utf8");
      assert.strictEqual(tab, DOC.replace("Last line", "Last line, edited in the tab"));
      assert.deepStrictEqual(await api.editSave(params), { conflict: true });
      assert.strictEqual(fs.readFileSync(file, "utf8"), tab, "nothing was written");
    } finally {
      await putBack();
    }
  });

  test("an edit in the tab that left the text as it was (typed and undone): the version moved, the text still equals base, and Save writes at the whole range", async () => {
    try {
      await putBack();
      const params = await docParams("Last line", "Last line, written");
      const document = await vscode.workspace.openTextDocument(vscode.Uri.file(file));
      const add = new vscode.WorkspaceEdit();
      add.insert(document.uri, new vscode.Position(0, 0), "x");
      assert.ok(await vscode.workspace.applyEdit(add));
      const remove = new vscode.WorkspaceEdit();
      remove.delete(document.uri, new vscode.Range(0, 0, 0, 1));
      assert.ok(await vscode.workspace.applyEdit(remove));
      assert.ok(document.version > params.version, "the version moved");
      assert.strictEqual(document.getText(), DOC, "and the text is what it was");
      const answer = await api.editSave(params);
      assert.ok("saved" in answer || "written" in answer, `Save answered ${JSON.stringify(answer)}`);
      assert.strictEqual(document.getText(), DOC.replace("Last line", "Last line, written"));
    } finally {
      await putBack();
    }
  });

  test("a doc with unsaved changes in a tab is refused, and Save and Edit saves it first and answers at once (E5, §8)", async () => {
    try {
      await putBack();
      const document = await vscode.workspace.openTextDocument(vscode.Uri.file(file));
      const edit = new vscode.WorkspaceEdit();
      edit.insert(document.uri, new vscode.Position(document.lineCount - 1, 0), "unsaved\n");
      assert.ok(await vscode.workspace.applyEdit(edit));
      assert.ok(document.isDirty);
      assert.deepStrictEqual(await api.editStartRequest({ kind: "doc", path: file, page: "docs" }), { dirty: true });
      assert.ok(document.isDirty, "nothing was saved");
      const got = await api.editStartRequest({ kind: "doc", path: file, page: "docs", saveFirst: true });
      assert.ok(!document.isDirty, "Save and Edit saved the document");
      assert.ok("text" in got && got.text === document.getText() && got.text.includes("unsaved"), "and the answer is the draft's start: no range to wait for");
    } finally {
      await putBack();
    }
  });

  test("the .md was deleted since the draft started: Save is gone, and nothing is written", async () => {
    const lost = path.join(dir, "lost.md");
    fs.writeFileSync(lost, "# Lost\n\ntext\n");
    try {
      // a draft of a file nothing has opened: its version is whatever the page was told, and the file goes
      const params: EditSaveParams = { kind: "doc", key: lost, path: lost, range: { start: { line: 0, character: 0 }, end: { line: 3, character: 0 } }, version: 1, base: "# Lost\n\ntext\n", prefix: "", eol: "\n", text: "# Lost\n\nnew text\n" };
      fs.rmSync(lost);
      assert.deepStrictEqual(await api.editSave(params), { gone: true });
      assert.ok(!fs.existsSync(lost), "and the file was not made again");
    } finally {
      fs.rmSync(lost, { force: true });
    }
  });

  test("one draft per doc across the pages: while the canvas holds a draft of the doc, the docs tab's Edit is busy; the canvas's own is not", async () => {
    try {
      await putBack();
      await vscode.commands.executeCommand("vnoc.openCanvas");
      await until("the canvas tab", () => canvasTabs().length === 1);
      await sleep(1000); // the page asks the host for its drafts once, when it starts: let it ask before the copy is fed
      const got = await api.editStartRequest({ kind: "doc", path: file, page: "canvas" });
      assert.ok("text" in got);
      const held: Draft = { kind: "doc", key: file, card: "overview", path: file, range: got.range!, version: got.version, base: got.text, prefix: "", eol: "\n", text: got.text + "typed\n", state: "editing" };
      api.feedDrafts("canvas", [held]); // what the canvas page's `drafts` note does
      assert.deepStrictEqual(await api.editStartRequest({ kind: "doc", path: file, page: "docs" }), { busy: true });
      const own = await api.editStartRequest({ kind: "doc", path: file, page: "canvas" });
      assert.ok("text" in own, "the canvas asking again is the canvas's own page: its store decides");
      api.feedDrafts("canvas", []);
      assert.ok("text" in (await api.editStartRequest({ kind: "doc", path: file, page: "docs" })), "the draft is saved or discarded: the doc is free");
      // the other way round, with the docs tab not open: nobody is editing, so a draft the user has not answered for does not block
      api.feedDrafts("docs", [held]);
      assert.ok("text" in (await api.editStartRequest({ kind: "doc", path: file, page: "canvas" })), "the docs tab is not open in this test: a draft it was left with does not block");
    } finally {
      api.feedDrafts("canvas", []);
      api.feedDrafts("docs", []);
      await closeCanvas();
      await putBack();
    }
  });
});

suite("editing a doc in the doc panel, on the canvas and in the docs tab (docs/17-edit-in-canvas.md step 9.6b)", () => {
  const NODE = "nested.py::decorated";
  let docPath: string;
  let before: string;
  let docDir: string;
  let madeVnoc = false;
  let vnoc: string;
  let node: TreeNode;

  suiteSetup(async function () {
    this.timeout(90_000);
    api = await vscode.extensions.getExtension("vnoc.vnoc")!.activate();
    discardOnClose();
    const { root, git_root } = { root: (await api.structure()).root, git_root: (await api.structure()).git_root };
    vnoc = path.join(git_root, ".vnoc");
    madeVnoc = !fs.existsSync(vnoc);
    node = allNodes(root).find((n) => n.id === NODE)!;
    const file = path.join(vscode.workspace.workspaceFolders![0].uri.fsPath, "nested.py");
    // a doc on the function, through the core, as Add Doc does; no marker, so nothing is written into the code
    const { id } = await api.request<{ id: string }>("vnoc/attach", { key: NODE, title: "Why decorated", markers: false, document: { uri: vscode.Uri.file(file).toString(), version: 1, text: fs.readFileSync(file, "utf8") } });
    const docs = await api.request<{ path: string }[]>("vnoc/docs", { id });
    docPath = docs[0].path;
    docDir = path.dirname(docPath);
    before = fs.readFileSync(docPath, "utf8");
    await waitFor("the doc on the function", async () => allNodes((await api.structure()).root).find((n) => n.id === NODE)?.docs === 1, 30_000);
    node = allNodes((await api.structure()).root).find((n) => n.id === NODE)!;
  });

  suiteTeardown(async () => {
    discardOnClose();
    await closeCanvas();
    await closeDocsTab();
    fs.rmSync(madeVnoc ? vnoc : docDir, { recursive: true, force: true });
  });

  const docsTabs = () => vscode.window.tabGroups.all.flatMap((g) => g.tabs).filter((t) => t.input instanceof vscode.TabInputWebview && t.input.viewType.endsWith("vnoc.docs"));
  async function closeDocsTab() {
    await vscode.window.tabGroups.close(docsTabs());
    await until("the docs tab to close", () => docsTabs().length === 0);
  }
  const stateOf = (since: number) => api.editStates().slice(since).filter((p) => p.key === docPath).at(-1);
  /**
   * The pencil, then typing, on the page that shows the doc, until it says `editing` (§8, 9.7): the pencil shows the source with no draft yet, and the first text that differs from its
   * baseline starts the draft. An event sent before a new page listens is lost, and the baseline arrives with the pencil's answer, so both are asked until it says so.
   */
  async function editing(since: number, text: string, show?: () => void) {
    for (let waited = 0; stateOf(since)?.state !== "editing"; waited += 500) {
      assert.ok(waited < 30_000, `the panel never said editing (last: ${JSON.stringify(stateOf(since))})`);
      show?.();
      api.viewCommand({ name: "docEdit", key: docPath });
      api.viewCommand({ name: "docSetDraft", key: docPath, text });
      await sleep(500);
    }
  }
  const edited = (text: string) => text.replace("# Why decorated", "# Why decorated, edited\n\nA paragraph written in the panel.");
  const openDoc = () => vscode.workspace.textDocuments.find((d) => d.uri.fsPath === docPath);

  /**
   * The doc as `before` on disk and in VS Code's copy, through VS Code if it holds the file (as the 9.6a suite's `putBack`): a file written behind its back leaves the document
   * with the edited text on Linux, and the next test's pencil answers that text as its baseline, so typing the same text starts no draft. It does not close the editors:
   * the blocks that call it close the pages themselves, in an order.
   */
  async function putDocBack() {
    const open = openDoc();
    if (open && open.getText() !== before) {
      const edit = new vscode.WorkspaceEdit();
      edit.replace(open.uri, new vscode.Range(0, 0, open.lineCount, 0), before);
      await vscode.workspace.applyEdit(edit);
    }
    if (open?.isDirty) await open.save();
    if (fs.readFileSync(docPath, "utf8") !== before) fs.writeFileSync(docPath, before);
    await waitFor("the doc, on disk and in VS Code, to be as put back", () => fs.readFileSync(docPath, "utf8") === before && (openDoc()?.getText() ?? before) === before);
  }

  test("on the canvas: the pencil in the docs panel shows the source with no draft, typing starts the draft, Save: the .md on disk changed, the panel shows the rendered doc again, and the bar counted the draft meanwhile", async () => {
    try {
      await vscode.commands.executeCommand("vnoc.openCanvas");
      await until("the canvas tab", () => canvasTabs().length === 1);
      const since = api.editStates().length;
      api.dispatch({ type: "expand", ids: ["nested.py"] });
      api.dispatch({ type: "select", id: NODE });
      // the pencil: the source of the `.md`, with no draft yet (the panel says `reading`, and the bar has nothing to count)
      for (let waited = 0; stateOf(since)?.state !== "reading"; waited += 500) {
        assert.ok(waited < 30_000, "the panel never showed the doc");
        api.viewCommand({ name: "showDocPanel" });
        await sleep(500);
      }
      api.viewCommand({ name: "docEdit", key: docPath });
      await sleep(1500);
      assert.deepStrictEqual([stateOf(since)?.state, api.draftsChip()], ["reading", { count: 0, label: "" }], "the source is shown and nothing was typed: no draft, nothing to count");
      await editing(since, edited(before), () => api.viewCommand({ name: "showDocPanel" }));
      // typed back to the original: nothing to lose, and no draft
      api.viewCommand({ name: "docSetDraft", key: docPath, text: before });
      await waitFor("the draft to be dropped by typing back", () => stateOf(since)?.state === "reading" && api.draftCopyOf("canvas").length === 0, 10_000);
      await editing(since, edited(before));
      await waitFor("the bar to count the doc draft and the host to hold it", () => api.draftsChip().count === 1 && api.draftCopyOf("canvas").some((d) => d.kind === "doc" && d.path === docPath && d.text === edited(before)), 10_000);
      assert.strictEqual(fs.readFileSync(docPath, "utf8"), before, "nothing is written until Save");
      api.viewCommand({ name: "docSave", key: docPath });
      await waitFor("the .md on disk to be the edited text, byte for byte", () => fs.readFileSync(docPath, "utf8") === edited(before), 30_000);
      await waitFor("the panel to be reading again", () => stateOf(since)?.state === "reading", 10_000);
      await waitFor("the copy to be empty and the bar's count gone", () => api.draftCopyOf("canvas").length === 0 && api.draftsChip().count === 0, 10_000);
      assert.deepStrictEqual(api.webviewErrors(), []);
    } finally {
      discardOnClose();
      await closeCanvas();
      await putDocBack();
    }
  });

  test("in the docs tab: the same, and the tab's own drafts are the `docs` page's in the host's copy", async () => {
    try {
      const open = vscode.workspace.textDocuments.find((d) => d.uri.fsPath === docPath);
      assert.ok(!open || open.getText() === before, "VS Code's copy of the doc is the doc on disk: the last test put it back through VS Code");
      const since = api.editStates().length;
      await vscode.commands.executeCommand("vnoc.showDocs", node);
      await until("the docs tab", () => docsTabs().length === 1);
      await editing(since, edited(before));
      await waitFor("the host to hold the draft as the docs tab's", () => api.draftCopyOf("docs").some((d) => d.kind === "doc" && d.path === docPath && d.text === edited(before)), 10_000);
      assert.deepStrictEqual(api.draftCopyOf("canvas"), [], "and not the canvas's");
      api.viewCommand({ name: "docSave", key: docPath });
      await waitFor("the .md on disk to be the edited text", () => fs.readFileSync(docPath, "utf8") === edited(before), 30_000);
      await waitFor("the tab to be reading again and the copy empty", () => stateOf(since)?.state === "reading" && api.draftCopyOf("docs").length === 0, 10_000);
      assert.deepStrictEqual(api.webviewErrors(), []);
    } finally {
      discardOnClose();
      await closeDocsTab();
      await putDocBack();
    }
  });

  test("one draft per doc across the pages, and a docs tab closed with a draft asks: the canvas's panel is busy while the docs tab edits; Reopen Docs Tab brings the draft back (§8)", async () => {
    const shown: { message: string; buttons: string[] }[] = [];
    api.setDraftNotify(async (message, ...buttons) => (shown.push({ message, buttons }), "Reopen Docs Tab"));
    try {
      const open = vscode.workspace.textDocuments.find((d) => d.uri.fsPath === docPath);
      assert.ok(!open || open.getText() === before, "VS Code's copy of the doc is the doc on disk: the last test put it back through VS Code");
      const since = api.editStates().length;
      await vscode.commands.executeCommand("vnoc.showDocs", node);
      await until("the docs tab", () => docsTabs().length === 1);
      await editing(since, edited(before));
      await waitFor("the host to hold the draft", () => api.draftCopyOf("docs").some((d) => d.text === edited(before)), 10_000);

      // the canvas, on the same node: its Edit is refused, and says which page has the doc
      await vscode.commands.executeCommand("vnoc.openCanvas");
      await until("the canvas tab", () => canvasTabs().length === 1);
      api.dispatch({ type: "expand", ids: ["nested.py"] });
      api.dispatch({ type: "select", id: NODE });
      for (let waited = 0; !api.editStates().slice(since).some((p) => p.key === docPath && p.elsewhere); waited += 500) {
        assert.ok(waited < 30_000, "the canvas's panel never said the doc is being edited elsewhere");
        api.viewCommand({ name: "showDocPanel" });
        api.viewCommand({ name: "docEdit", key: docPath });
        await sleep(500);
      }
      assert.deepStrictEqual(api.draftCopyOf("canvas"), [], "the canvas has no draft of it");
      assert.strictEqual(api.draftCopyOf("docs").length, 1, "and the docs tab still has its own");
      await closeCanvas();

      // the docs tab is closed with its draft: the notification names it, and Reopen brings it back
      const reopened = api.editStates().length;
      await vscode.window.tabGroups.close(docsTabs());
      await waitFor("the notification", () => shown.length === 1, 10_000);
      assert.deepStrictEqual(shown[0], { message: "1 unsaved draft from the docs tab", buttons: ["Save All", "Reopen Docs Tab", "Discard"] });
      await until("the docs tab to open again", () => docsTabs().length === 1);
      for (let waited = 0; stateOf(reopened)?.state !== "editing"; waited += 500) {
        assert.ok(waited < 30_000, `the reopened tab never said editing (last: ${JSON.stringify(stateOf(reopened))})`);
        await sleep(500);
      }
      // with its text: Save writes what was typed before the tab was closed
      api.viewCommand({ name: "docSave", key: docPath });
      await waitFor("the .md on disk to be the draft typed before the close", () => fs.readFileSync(docPath, "utf8") === edited(before), 30_000);
      await waitFor("the copy to be empty", () => api.draftCopyOf("docs").length === 0, 10_000);
      assert.deepStrictEqual(api.webviewErrors(), []);
    } finally {
      discardOnClose();
      await closeCanvas();
      await closeDocsTab();
      await putDocBack();
    }
  });
});
