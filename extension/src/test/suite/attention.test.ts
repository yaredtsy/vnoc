import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";
import * as vscode from "vscode";

import type { Problem, StructureResult } from "@vnoc/shared/messages";
import type { Session } from "@vnoc/shared/session.ts";
import { sleep, until } from "./util";

// NEEDS ATTENTION in a real window (docs/11-missing-relink-remove.md §6): a deleted function shows under Missing and leaves when it is back;
// a problem on a folder is revealed, not opened. The suite reads the view's rows through the extension's test API: it cannot see the tree itself.

interface Api {
  structure(): Promise<StructureResult>;
  session(): Session;
  attention(): { missing: string[]; problems: Problem[] };
  request<T>(method: string, params?: object): Promise<T>;
  webviewErrors(): string[];
  moveToTrash(id: string): Promise<void>;
  restoreEntry(entry: string): Promise<void>;
}
let api: Api;
let folder: string;

const textTabs = () => vscode.window.tabGroups.all.flatMap((g) => g.tabs).filter((t) => t.input instanceof vscode.TabInputText);

suiteSetup(async () => {
  api = await vscode.extensions.getExtension("vnoc.vnoc")!.activate();
  folder = vscode.workspace.workspaceFolders![0].uri.fsPath;
});

suite("needs attention", () => {
  const file = () => path.join(folder, "doomed.py");
  const BOTH = "def doomed():\n    pass\n\n\ndef stays():\n    pass\n";
  let id: string;

  const functionsOf = async () => (await api.structure()).root.children.find((n) => n.name === "doomed.py")?.children ?? [];
  const nameFound = (name: string, docs: number) => async () => (await functionsOf()).some((n) => n.name === name && n.docs === docs);
  const waitFor = async (what: string, check: () => Promise<boolean> | boolean) => {
    for (let waited = 0; !(await check()); waited += 200) {
      assert.ok(waited < 20_000, `timed out waiting for ${what}`);
      await sleep(200);
    }
  };

  suiteSetup(async () => {
    fs.writeFileSync(file(), BOTH);
    await waitFor("doomed.py in the structure", async () => (await functionsOf()).length === 2);
    // markers off for this attach: the doc is found by name, so deleting the function makes it Missing (the core writes nothing into the code)
    const key = (await functionsOf()).find((n) => n.name === "doomed")!.id;
    const r = await api.request<{ id: string }>("vnoc/attach", { key, title: "Why doomed", markers: false, document: { uri: vscode.Uri.file(file()).toString(), version: 1, text: BOTH } });
    id = r.id;
    await waitFor("the doc on the function", nameFound("doomed", 1));
  });

  suiteTeardown(() => {
    fs.rmSync(file(), { force: true });
  });

  test("a documented function that is deleted shows under Missing, and leaves when it is back", async () => {
    assert.deepStrictEqual(api.attention().missing, [], "nothing is missing while the code is there");
    fs.writeFileSync(file(), "def stays():\n    pass\n");
    await until("the function under Missing", () => api.attention().missing.includes(id));
    // a Missing node is not in the tree, so the session cannot be pointing at it
    assert.strictEqual((await functionsOf()).some((n) => n.name === "doomed"), false);

    fs.writeFileSync(file(), BOTH);
    await until("the function to leave Missing", () => !api.attention().missing.includes(id));
    assert.ok(await nameFound("doomed", 1)(), "found again, with its doc");
  });

  test("a Missing row opens its docs in the docs tab, pinned to it", async () => {
    fs.writeFileSync(file(), "def stays():\n    pass\n");
    await until("the function under Missing", () => api.attention().missing.includes(id));
    const before = api.session();
    await vscode.commands.executeCommand("vnoc.openMissingDocs", { row: "missing", node: (await api.structure()).missing.find((m) => m.id === id) });
    await until("the docs tab", () => vscode.window.tabGroups.all.flatMap((g) => g.tabs).some((t) => t.input instanceof vscode.TabInputWebview && t.input.viewType.endsWith("vnoc.docs")));
    assert.strictEqual(api.session().contexts.main.selected, before.contexts.main.selected, "the session did not change");
    await sleep(1500);
    assert.deepStrictEqual(api.webviewErrors(), [], "the docs tab drew a node that is not in the tree without crashing");
    fs.writeFileSync(file(), BOTH);
    await until("the function to leave Missing", () => !api.attention().missing.includes(id));
  });

  test("a problem on a folder is revealed in the Explorer, not opened as a document", async () => {
    const broken = path.join(folder, ".vnoc", "nodes", "zz", "not-an-id");
    fs.mkdirSync(broken, { recursive: true });
    try {
      await until("the problem", () => api.attention().problems.some((p) => p.file === broken));
      const problem = api.attention().problems.find((p) => p.file === broken)!;
      assert.strictEqual(problem.text, ".vnoc/nodes/zz/not-an-id: the folder name is not a node ID");
      const tabs = textTabs().length;
      const went = await vscode.commands.executeCommand("vnoc.openProblem", { row: "problem", problem });
      assert.strictEqual(went, "revealed");
      assert.strictEqual(textTabs().length, tabs, "no editor was opened");
    } finally {
      fs.rmSync(path.join(folder, ".vnoc", "nodes", "zz"), { recursive: true, force: true });
    }
    await until("the problem to go", () => api.attention().problems.every((p) => p.file !== broken));
  });

  test("Remove then Restore gives back the same bytes, and an open doc follows the move", async () => {
    // a node that still has its code cannot be removed: the core refuses
    await assert.rejects(api.moveToTrash(id), /not Missing/);

    fs.writeFileSync(file(), "def stays():\n    pass\n");
    await until("the function under Missing", () => api.attention().missing.includes(id));
    const docs = await api.request<{ path: string }[]>("vnoc/docs", { id });
    const nodeFolder = path.dirname(docs[0].path);
    const bytes = (dir: string) => fs.readdirSync(dir).sort().map((f) => [f, fs.readFileSync(path.join(dir, f), "utf8")]);
    const before = bytes(nodeFolder);
    assert.ok(before.length >= 2, "node.toml and a doc");
    await vscode.window.showTextDocument(vscode.Uri.file(docs[0].path));

    await api.moveToTrash(id);
    assert.ok(!fs.existsSync(nodeFolder), "the node folder moved");
    await until("the node to leave Missing (the index dropped the record)", () => !api.attention().missing.includes(id));
    const entries = await api.request<{ entry: string; id: string; docs: number }[]>("vnoc/trashList");
    const entry = entries.find((e) => e.id === id)!;
    assert.ok(entry, "it is in the trash list");
    assert.ok(
      vscode.window.tabGroups.all.flatMap((g) => g.tabs).some((t) => t.input instanceof vscode.TabInputText && t.input.uri.fsPath.includes(`${path.sep}local${path.sep}trash${path.sep}`)),
      "the open doc followed the move (spike S3)",
    );

    await api.restoreEntry(entry.entry);
    assert.deepStrictEqual(bytes(nodeFolder), before, "the same bytes");
    await until("the node to be Missing again", () => api.attention().missing.includes(id));
    assert.ok(!(await api.request<{ id: string }[]>("vnoc/trashList")).some((e) => e.id === id), "the emptied entry is gone");
    // restoring over docs that are there is refused (never overwritten): remove again, make the folder, restore
    await api.moveToTrash(id);
    const again = (await api.request<{ entry: string; id: string }[]>("vnoc/trashList")).find((e) => e.id === id)!;
    fs.mkdirSync(nodeFolder, { recursive: true });
    await assert.rejects(api.restoreEntry(again.entry), /already has docs/);
    fs.rmSync(nodeFolder, { recursive: true });
    await api.restoreEntry(again.entry);
    assert.deepStrictEqual(bytes(nodeFolder), before);
    await vscode.commands.executeCommand("workbench.action.closeAllEditors");
    fs.writeFileSync(file(), BOTH);
    await until("the function to leave Missing", () => !api.attention().missing.includes(id));
  });

  test("relinking a Missing function to another puts its docs there, and it leaves Missing", async () => {
    const only = "def stays():\n    pass\n";
    fs.writeFileSync(file(), only);
    await until("the function under Missing", () => api.attention().missing.includes(id));
    const stays = (await functionsOf()).find((n) => n.name === "stays")!;
    assert.strictEqual(stays.docs, 0);

    const document = { uri: vscode.Uri.file(file()).toString(), version: 2, text: only };
    const result = await api.request<{ written: string }>("vnoc/relink", { id, to: stays.id, document });
    assert.strictEqual(result.written, "anchorOnly", "this project writes no IDs (the first attach said so)");
    await until("the docs on the other function", () => !api.attention().missing.includes(id));
    const after = (await functionsOf()).find((n) => n.name === "stays")!;
    assert.deepStrictEqual([after.node_id, after.docs, after.link], [id, 1, "name"], "the badge is on the function it was moved to");
    fs.writeFileSync(file(), BOTH);
  });

  test("a problem on a file opens it at the line", async () => {
    const problem: Problem = { text: "x", file: file(), line: 4 };
    assert.strictEqual(await vscode.commands.executeCommand("vnoc.openProblem", { row: "problem", problem }), "opened");
    assert.strictEqual(vscode.window.activeTextEditor?.document.uri.fsPath, file());
    assert.strictEqual(vscode.window.activeTextEditor?.selection.active.line, 4);
  });
});
