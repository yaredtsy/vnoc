import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";
import * as vscode from "vscode";

import type { CallTreeResult, LanguageInfo, StructureResult, TreeNode } from "@vnoc/shared/messages";
import type { Session } from "@vnoc/shared/session.ts";
import { canvasTabs, closeCanvas, until, waitFor } from "./util";

// A second language in the same window: the opened folder holds a Python file and `bend/` (runTest.ts copies it from src/test/fixtures/bend). Nothing here is
// written for Bend alone: it is what the extension does for a language it was never told about, which is every driver the core reports.

interface Api {
  structure(): Promise<StructureResult>;
  session(): Session;
  request<T>(method: string, params?: object): Promise<T>;
  callTrees(): Record<string, CallTreeResult>;
  languageContexts(): { extensions: string[]; callTreeExtensions: string[] };
  webviewErrors(): string[];
}
let api: Api;
let folder: string;

const BEND = () => path.join(folder, "bend");
const find = (node: TreeNode, id: string): TreeNode | undefined => (node.id === id ? node : node.children.map((c) => find(c, id)).find(Boolean));
const def = async (file: string, name: string): Promise<TreeNode | undefined> => (await api.structure()).root.children.find((n) => n.name === "bend")?.children.find((n) => n.name === file)?.children.find((n) => n.name === name);

suite("a second language", () => {
  let original = "";
  const mainFile = () => path.join(BEND(), "main.bend");
  const settings = () => path.join(folder, ".vnoc", "vnoc.toml");
  let settingsBefore: string | null = null;
  let docsBefore: string[] = [];

  suiteSetup(async () => {
    api = await vscode.extensions.getExtension("vnoc.vnoc")!.activate();
    folder = vscode.workspace.workspaceFolders![0].uri.fsPath;
    original = fs.readFileSync(mainFile(), "utf8");
    settingsBefore = fs.existsSync(settings()) ? fs.readFileSync(settings(), "utf8") : null;
    const nodes = path.join(folder, ".vnoc", "nodes");
    docsBefore = fs.existsSync(nodes) ? fs.readdirSync(nodes) : [];
    await waitFor("the Bend files in the structure", async () => (await def("main.bend", "uses_lib")) !== undefined);
    await closeCanvas();
  });

  suiteTeardown(async () => {
    await vscode.commands.executeCommand("workbench.action.closeAllEditors");
    fs.writeFileSync(mainFile(), original);
    const nodes = path.join(folder, ".vnoc", "nodes");
    if (fs.existsSync(nodes)) for (const added of fs.readdirSync(nodes).filter((n) => !docsBefore.includes(n))) fs.rmSync(path.join(nodes, added), { recursive: true, force: true });
    if (settingsBefore === null) fs.rmSync(settings(), { force: true });
    else fs.writeFileSync(settings(), settingsBefore);
  });

  test("the core reports both languages, running, each with its extensions, and the extension builds its context keys from that", async () => {
    const languages = await api.request<LanguageInfo[]>("vnoc/languages");
    const by = (name: string) => languages.find((l) => l.name === name);
    assert.ok(by("python") && by("bend"), `python and bend: ${JSON.stringify(languages.map((l) => l.name))}`);
    for (const l of languages) assert.strictEqual(l.state, "running", `${l.name}: ${l.error}`);
    assert.ok(by("python")!.extensions.includes("py"));
    assert.deepStrictEqual(by("bend")!.extensions, ["bend"]);
    const { extensions, callTreeExtensions } = api.languageContexts();
    // the V-NOC submenu is offered on a .bend file and on a .py file (`resourceExtname in vnoc.extensions`), and Show Calls on both (both drivers have call trees)
    assert.ok(extensions.includes(".bend") && extensions.includes(".py"), JSON.stringify(extensions));
    assert.ok(callTreeExtensions.includes(".bend") && callTreeExtensions.includes(".py"), JSON.stringify(callTreeExtensions));
    assert.ok(!extensions.includes("bend"), "with the dot, as `resourceExtname` has it");
  });

  test("the tree shows the .bend files and their defs next to the Python file", async () => {
    const root = (await api.structure()).root;
    const bend = root.children.find((n) => n.name === "bend");
    assert.ok(bend && bend.kind === "folder", "the folder");
    assert.deepStrictEqual(bend.children.map((n) => n.name).sort(), ["lib.bend", "main.bend"]);
    assert.deepStrictEqual(bend.children.find((n) => n.name === "lib.bend")!.children.map((n) => n.name), ["inc", "dec"]);
    assert.deepStrictEqual(bend.children.find((n) => n.name === "main.bend")!.children.map((n) => n.name), ["helper", "uses_lib"]);
    assert.ok(root.children.some((n) => n.name === "nested.py"), "and the Python file is still there");
    const inc = (await def("lib.bend", "inc"))!;
    assert.strictEqual(inc.link, null, "an ID is only a link once a doc has it");
    assert.strictEqual(inc.kind, "function");
  });

  test("a file the watcher's glob names is read when it appears, and gone when it is deleted", async () => {
    const extra = path.join(BEND(), "extra.bend");
    fs.writeFileSync(extra, "def extra() -> U32:\n  1\n");
    await waitFor("extra.bend in the structure", async () => (await def("extra.bend", "extra")) !== undefined);
    fs.rmSync(extra);
    await waitFor("extra.bend gone", async () => (await api.structure()).root.children.find((n) => n.name === "bend")?.children.every((n) => n.name !== "extra.bend") === true);
  });

  test("Open in Canvas on a Bend def opens the canvas with it selected", async () => {
    const inc = (await def("lib.bend", "inc"))!;
    await vscode.commands.executeCommand("vnoc.openInCanvas", { nodeKey: inc.id, webviewSection: "node" });
    await until("the canvas tab", () => canvasTabs().length === 1);
    assert.strictEqual(api.session().contexts.main.selected, inc.id);
    await closeCanvas();
  });

  test("Write ID on a Bend def puts `# ID: <id>` on the line above it, and the def is found by its marker", async () => {
    const node = (await def("main.bend", "uses_lib"))!;
    const document = await vscode.workspace.openTextDocument(vscode.Uri.file(mainFile()));
    // a doc, found by name (no marker yet): the core writes nothing into the code
    const { id } = await api.request<{ id: string }>("vnoc/attach", { key: node.id, title: "Why uses_lib", markers: false, document: { uri: document.uri.toString(), version: document.version, text: document.getText() } });
    await waitFor("the doc on the def", async () => (await def("main.bend", "uses_lib"))?.docs === 1);
    assert.ok(!fs.readFileSync(mainFile(), "utf8").includes("ID:"), "nothing was written yet");
    // attaching with no marker wrote `[ids] default = "none"`: IDs are on again for Write ID (as in the attention suite)
    fs.writeFileSync(settings(), "format = 1\n");

    await vscode.commands.executeCommand("vnoc.writeId", await def("main.bend", "uses_lib"));
    await waitFor("the def found by its marker", async () => (await def("main.bend", "uses_lib"))?.link === "marker");
    const lines = fs.readFileSync(mainFile(), "utf8").split(/\r?\n/);
    const at = lines.findIndex((l) => l.startsWith("def uses_lib"));
    assert.strictEqual(lines[at - 1], `# ID: ${id}`, "directly above the def, as a Bend comment");
    assert.strictEqual((await def("main.bend", "uses_lib"))!.node_id, id);
  });

  test("Show Calls on a Bend def gives the def it calls in another file", async () => {
    const uses = (await def("main.bend", "uses_lib"))!;
    await vscode.commands.executeCommand("vnoc.showCalls", uses);
    await until("the canvas tab", () => canvasTabs().length === 1);
    await until("its call tree", () => api.callTrees()[uses.id] !== undefined, 30_000);
    const callees = api.callTrees()[uses.id].root.children;
    assert.deepStrictEqual(callees.map((c) => c.name).sort(), ["helper", "inc"]);
    const inc = callees.find((c) => c.name === "inc")!;
    assert.strictEqual(inc.path, "bend/lib.bend", "the callee is in the other file");
    assert.strictEqual(inc.key, (await def("lib.bend", "inc"))!.id, "and the index knows it");
    assert.ok(find((await api.structure()).root, inc.key!), "its key is a node of the tree");
    assert.deepStrictEqual(api.webviewErrors(), []);
    await closeCanvas();
  });
});
