import * as assert from "assert";
import { execFileSync } from "child_process";
import * as fs from "fs";
import * as path from "path";
import * as vscode from "vscode";

import type { StructureResult, TreeNode } from "@vnoc/shared/messages";
import { allNodes } from "@vnoc/shared/tree";
import { waitFor } from "./util";

// THE PROMISE OF 0.1 in a real window (docs/13-harden-and-publish.md §4): "the doc is still there after a rename". The core's tests prove it with files
// the test writes; this suite does what a person does: an edit in VS Code, a rename in the Explorer, a `git mv` in a terminal.

interface Api {
  structure(): Promise<StructureResult>;
  attention(): { missing: string[] };
  request<T>(method: string, params?: object): Promise<T>;
  webviewErrors(): string[];
}
let api: Api;
let workspace: string;

const FOLDER = "refactor"; // this suite's own folder in the workspace, removed at the end
const at = (...parts: string[]) => path.join(workspace, FOLDER, ...parts);
const uri = (...parts: string[]) => vscode.Uri.file(at(...parts));
const id = (...parts: string[]) => [FOLDER, ...parts].join("/");

const find = async (key: string): Promise<TreeNode | undefined> => allNodes((await api.structure()).root).find((n) => n.id === key);
const git = (...args: string[]) => execFileSync("git", ["-c", "user.name=V-NOC test", "-c", "user.email=test@vnoc.invalid", ...args], { cwd: workspace, stdio: "ignore" });

/**
 * Attaches a doc to the node `key` with markers on (the ID goes into the code), and waits until the core finds the node by its ID.
 * `file` is the node's file, relative to the suite's folder: its document is open, edited and saved. A folder has no document: the core writes its `.vnoc-id`.
 */
async function attached(file: string, key: string, title: string): Promise<string> {
  const folder = fs.statSync(path.join(workspace, key.split("::")[0])).isDirectory();
  const document = folder ? undefined : await vscode.workspace.openTextDocument(uri(...file.split("/")));
  const result = await api.request<{ id: string }>("vnoc/attach", { key, title, markers: true, document: document && { uri: document.uri.toString(), version: document.version, text: document.getText() } });
  if (document) assert.ok(await document.save(), "the ID is in the editor's buffer: the core reads the saved file");
  await waitFor(`${key} found by its ID`, async () => (await find(key))?.link === "marker");
  return result.id;
}

/** One node, the same permanent ID and its doc; and nothing of ours under Missing, nothing crashed. */
async function assertKept(key: string, node_id: string) {
  await waitFor(`${key} to carry its ID`, async () => (await find(key))?.node_id === node_id);
  const node = (await find(key))!;
  assert.deepStrictEqual([node.node_id, node.docs, node.link], [node_id, 1, "marker"], `${key} keeps its ID and its doc`);
  assert.ok(!api.attention().missing.includes(node_id), `${key} is not under Missing`);
  assert.deepStrictEqual(api.webviewErrors(), []);
}

suite("refactor: the doc is still there after a rename or a move", () => {
  const settings = () => path.join(workspace, ".vnoc", "vnoc.toml");
  let before: string | null;

  suiteSetup(async () => {
    api = await vscode.extensions.getExtension("vnoc.vnoc")!.activate();
    workspace = vscode.workspace.workspaceFolders![0].uri.fsPath;
    // IDs on (an earlier suite wrote `[ids] default = "none"` here); put back at the end
    before = fs.existsSync(settings()) ? fs.readFileSync(settings(), "utf8") : null;
    fs.mkdirSync(path.dirname(settings()), { recursive: true });
    fs.writeFileSync(settings(), "format = 1\n");
    fs.mkdirSync(at(), { recursive: true });
  });

  suiteTeardown(async () => {
    await vscode.commands.executeCommand("workbench.action.closeAllEditors");
    fs.rmSync(at(), { recursive: true, force: true });
    if (before === null) fs.rmSync(settings(), { force: true });
    else fs.writeFileSync(settings(), before);
  });

  /** `git mv` needs a tracked file: the files and folders of a test are added first. */
  const gitMove = async (from: string, to: string) => {
    await vscode.commands.executeCommand("workbench.action.closeAllEditors");
    git("add", "--", FOLDER);
    git("mv", "--", `${FOLDER}/${from}`, `${FOLDER}/${to}`);
  };
  const explorerMove = async (from: string, to: string) => {
    await vscode.commands.executeCommand("workbench.action.closeAllEditors");
    const edit = new vscode.WorkspaceEdit();
    edit.renameFile(uri(from), uri(to));
    assert.ok(await vscode.workspace.applyEdit(edit), "VS Code renamed it");
  };

  /** A file with a documented function, and a documented file: the setup of tests 3 and 4. */
  async function documentedFile(name: string) {
    fs.writeFileSync(at(name), "def inner():\n    pass\n");
    await waitFor(`${name} in the structure`, async () => (await find(id(name)))?.children.length === 1);
    const fn = await attached(name, `${id(name)}::inner`, "Why inner");
    const file = await attached(name, id(name), "About the file");
    return { fn, file };
  }

  /** A folder with a documented folder, file and function in it: the setup of tests 5 and 6. */
  async function documentedFolder(name: string) {
    fs.mkdirSync(at(name), { recursive: true });
    fs.writeFileSync(at(name, "m.py"), "def deep():\n    pass\n");
    await waitFor(`${name}/m.py in the structure`, async () => (await find(id(name, "m.py")))?.children.length === 1);
    const fn = await attached(`${name}/m.py`, `${id(name, "m.py")}::deep`, "Why deep");
    const file = await attached(`${name}/m.py`, id(name, "m.py"), "About m.py");
    const folder = await attached(`${name}/m.py`, id(name), "About the folder");
    return { fn, file, folder };
  }

  test("1. a function renamed in the editor keeps its doc", async () => {
    fs.writeFileSync(at("one.py"), "def alpha():\n    pass\n");
    await waitFor("one.py in the structure", async () => (await find(id("one.py")))?.children.length === 1);
    const node = await attached("one.py", `${id("one.py")}::alpha`, "Why alpha");

    const document = await vscode.workspace.openTextDocument(uri("one.py"));
    const edit = new vscode.WorkspaceEdit();
    edit.replace(document.uri, new vscode.Range(0, 4, 0, 9), "beta");
    assert.ok(await vscode.workspace.applyEdit(edit));
    assert.ok(await document.save());
    await waitFor("the function under its new name", async () => (await find(`${id("one.py")}::beta`)) !== undefined);
    await assertKept(`${id("one.py")}::beta`, node);
  });

  test("2. a function moved to another file keeps its doc", async () => {
    fs.writeFileSync(at("from.py"), "def mover():\n    pass\n\n\ndef stays():\n    pass\n");
    fs.writeFileSync(at("to.py"), "X = 1\n");
    await waitFor("both files in the structure", async () => (await find(id("from.py")))?.children.length === 2 && (await find(id("to.py"))) !== undefined);
    const node = await attached("from.py", `${id("from.py")}::mover`, "Why mover");

    const from = await vscode.workspace.openTextDocument(uri("from.py"));
    const to = await vscode.workspace.openTextDocument(uri("to.py"));
    const lines = from.getText().split("\n");
    const end = lines.findIndex((l) => l.trim() === "pass") + 1; // the def, its `ID:` line and its body
    const moved = lines.slice(0, end).join("\n") + "\n";
    const edit = new vscode.WorkspaceEdit();
    edit.delete(from.uri, new vscode.Range(0, 0, end, 0));
    edit.insert(to.uri, new vscode.Position(to.lineCount, 0), `\n\n${moved}`);
    assert.ok(await vscode.workspace.applyEdit(edit));
    // the new place first, then the old: the ID is never absent from both
    assert.ok(await to.save());
    assert.ok(await from.save());
    await waitFor("the function in its new file", async () => (await find(`${id("to.py")}::mover`)) !== undefined);
    await assertKept(`${id("to.py")}::mover`, node);
    assert.strictEqual(await find(`${id("from.py")}::mover`), undefined, "and not in the old one");
  });

  test("3. a file renamed in the Explorer keeps its doc, and so does the function in it", async () => {
    const { fn, file } = await documentedFile("three.py");
    await explorerMove("three.py", "three_new.py");
    await waitFor("the file under its new name", async () => (await find(id("three_new.py"))) !== undefined);
    await assertKept(id("three_new.py"), file);
    await assertKept(`${id("three_new.py")}::inner`, fn);
  });

  test("4. a file renamed with git mv keeps its doc, and so does the function in it", async () => {
    const { fn, file } = await documentedFile("four.py");
    await gitMove("four.py", "four_new.py");
    await waitFor("the file under its new name", async () => (await find(id("four_new.py"))) !== undefined);
    await assertKept(id("four_new.py"), file);
    await assertKept(`${id("four_new.py")}::inner`, fn);
  });

  test("5. a folder renamed in the Explorer keeps its doc, and so do the file and function in it", async () => {
    const { fn, file, folder } = await documentedFolder("five");
    await explorerMove("five", "five_new");
    await waitFor("the folder under its new name", async () => (await find(id("five_new"))) !== undefined);
    await assertKept(id("five_new"), folder);
    await assertKept(id("five_new", "m.py"), file);
    await assertKept(`${id("five_new", "m.py")}::deep`, fn);
  });

  test("6. a folder renamed with git mv keeps its doc, and so do the file and function in it", async () => {
    const { fn, file, folder } = await documentedFolder("six");
    await gitMove("six", "six_new");
    await waitFor("the folder under its new name", async () => (await find(id("six_new"))) !== undefined);
    await assertKept(id("six_new"), folder);
    await assertKept(id("six_new", "m.py"), file);
    await assertKept(`${id("six_new", "m.py")}::deep`, fn);
  });
});
