import * as assert from "assert";
import * as vscode from "vscode";

const WS = process.env.VNOC_TEST_WORKSPACE!;
let api: { ping(): Promise<any>; reportFacts(): { extension: string; core: string | undefined }; logTail(): string; structure(): Promise<any>; waitRendered(ms: number): Promise<{ visible: number; hidden: number; callCards: number; chevrons: number }> };

suiteSetup(async () => {
  const ext = vscode.extensions.getExtension("vnoc.vnoc")!;
  assert.ok(ext, "the extension is installed in the test host");
  api = await ext.activate();
});

suite("walking skeleton: extension host -> core -> Python driver -> structure -> webview", () => {
  test("the extension starts the core, and the core answers", async () => {
    const pong = await api.ping();
    assert.strictEqual(pong.pong, true);
    assert.ok(pong.pid > 0);
  });

  test("the core reads the opened folder through the driver", async () => {
    const r = await api.structure();
    assert.strictEqual(r.root.children[0].name, "nested.py");
    assert.strictEqual(r.stats.files, 1);
    assert.strictEqual(r.stats.classes, 3);
    assert.strictEqual(r.stats.functions, 12);
    assert.deepStrictEqual(r.failed, []);
  });

  test("'V-NOC: Open canvas' opens a webview that draws the structure", async () => {
    await vscode.commands.executeCommand("vnoc.openCanvas");
    // VS Code's tab model updates asynchronously, so wait for the tab instead of expecting it at once
    const findTab = () => vscode.window.tabGroups.all.flatMap((g) => g.tabs).find((t) => t.input instanceof vscode.TabInputWebview && t.input.viewType.endsWith("vnoc.canvas"));
    for (let i = 0; i < 50 && !findTab(); i++) await new Promise((r) => setTimeout(r, 100));
    assert.ok(findTab(), "a V-NOC canvas tab is open");

    // The webview asked the host for the structure (host -> core -> driver), laid it out and drew it, then reported back.
    // Files start collapsed: the folder and nested.py are drawn, the 15 defs are hidden behind it.
    const drew = await api.waitRendered(30_000);
    assert.deepStrictEqual(drew, { context: "main", visible: 2, hidden: 15, callCards: 0, chevrons: 0 });
  });
});

suite("Report Issue and Help and Feedback (18.03)", () => {
  test("both commands are registered", async () => {
    const all = await vscode.commands.getCommands(true);
    for (const id of ["vnoc.reportIssue", "vnoc.help", "vnoc.showLog"]) assert.ok(all.includes(id), `${id} is registered`);
  });

  test("the facts a report carries: the extension's version, and the core's, which is the version the core answers a ping with", async () => {
    const pong = await api.ping();
    const facts = api.reportFacts();
    assert.strictEqual(facts.extension, vscode.extensions.getExtension("vnoc.vnoc")!.packageJSON.version);
    assert.strictEqual(facts.core, pong.version);
    assert.strictEqual(pong.version, facts.extension, "the core and the extension are released together");
  });

  test("the log a report copies has what the core wrote: the tee sees the channel's lines", () => {
    assert.match(api.logTail(), /core started: /);
  });
});
