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

  test("a problem on a file opens it at the line", async () => {
    const problem: Problem = { text: "x", file: file(), line: 4 };
    assert.strictEqual(await vscode.commands.executeCommand("vnoc.openProblem", { row: "problem", problem }), "opened");
    assert.strictEqual(vscode.window.activeTextEditor?.document.uri.fsPath, file());
    assert.strictEqual(vscode.window.activeTextEditor?.selection.active.line, 4);
  });
});
