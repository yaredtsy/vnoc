import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";
import * as vscode from "vscode";

import type { StructureResult, TreeNode } from "@vnoc/shared/messages";
import type { Intent, Session } from "@vnoc/shared/session.ts";
import { canvasTabs, closeCanvas, sleep, until, waitFor } from "./util";

// The STRUCTURE view in a real window (docs/16-ui-redesign/06-sidebar.md, 07 §4): it is a webview, so the suite drives it through what it says (the
// `sidebarRendered` note) and through the host's side of a click (`rowClick`). What a key or a click means is unit-tested beside the page.

interface Api {
  structure(): Promise<StructureResult>;
  session(): Session;
  dispatch(intent: Intent, context?: string): void;
  rowClick(context: string, id: string): Promise<void>;
  sidebarPosts(): { rows: number; selected: string | null; inView: boolean; contexts: number; height: number }[];
  sidebarOpen(): boolean;
  renderedPosts(): { context: string; visible: number }[];
  webviewErrors(): string[];
}
let api: Api;

/** Waits until the index has stopped changing: the same revision for a second. */
async function structureSettled(): Promise<void> {
  let revision = -1;
  let since = Date.now();
  await waitFor("the structure to stop changing", async () => {
    const now = (await api.structure()).revision;
    if (now !== revision) (revision = now), (since = Date.now());
    return Date.now() - since >= 1000;
  }, 30_000);
}

/** What changed where the keyboard and the pointer could be taken or cleared, with times: a failure that names its cause is worth more than a timeout. */
function recordTimeline(highlight: () => string | null): { log: string[]; stop: () => void } {
  const t0 = Date.now();
  const log: string[] = [];
  const note = (what: string) => log.push(`+${Date.now() - t0} ${what}`);
  let last = highlight();
  const poll = setInterval(() => {
    const now = highlight();
    if (now !== last) (last = now), note(`highlight ${now === null ? "cleared" : "→ " + now}`);
  }, 20);
  const subs = [
    vscode.window.onDidChangeWindowState((e) => note(`window focused=${e.focused}`)),
    vscode.window.onDidChangeActiveTextEditor((e) => note(`active text editor ${e?.document.uri.path.split("/").pop() ?? "none"}`)),
    vscode.window.tabGroups.onDidChangeTabs((e) => note(`tabs +${e.opened.length} -${e.closed.length} ~${e.changed.length}`)),
    vscode.window.tabGroups.onDidChangeTabGroups(() => note("tab groups changed")),
  ];
  return { log, stop: () => (clearInterval(poll), subs.forEach((s) => s.dispose())) };
}

const textTabs = () => vscode.window.tabGroups.all.flatMap((g) => g.tabs).filter((t) => t.input instanceof vscode.TabInputText);
const lastPost = () => api.sidebarPosts().at(-1);

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
  let firstFile: TreeNode;
  let other: TreeNode;
  const many = path.join(vscode.workspace.workspaceFolders![0].uri.fsPath, "sidebar_many.py");

  suiteSetup(async () => {
    // a second file, so a preview tab can be seen being replaced; and a long one, so a row can be below the fold
    const folder = vscode.workspace.workspaceFolders![0].uri.fsPath;
    fs.writeFileSync(path.join(folder, "other.py"), "def other():\n    pass\n");
    fs.writeFileSync(many, Array.from({ length: 120 }, (_, i) => `def many${i}():\n    return ${i}\n`).join("\n\n"));
    let files: TreeNode[] = [];
    for (let waited = 0; !(files.length >= 3 && files.find((f) => f.name === "sidebar_many.py")?.children.length === 120); waited += 200) {
      assert.ok(waited < 30_000, "the new files never reached the structure");
      files = (await api.structure()).root.children.filter((n) => n.kind === "file");
      await sleep(200);
    }
    firstFile = files.find((f) => f.name === "nested.py")!;
    first = firstFile.children[0];
    other = files.find((f) => f.name === "other.py")!.children[0];
  });

  suiteTeardown(async () => {
    fs.rmSync(many, { force: true });
    api.dispatch({ type: "focusJump", depth: 0 });
    api.dispatch({ type: "collapseAll" });
  });

  test("[check] vnoc.structure.focus brings the webview view up: its page is made and reports what it drew", async () => {
    assert.ok((await vscode.commands.getCommands(true)).includes("vnoc.structure.focus"), "VS Code makes `<viewId>.focus` for a webview view");
    await vscode.commands.executeCommand("vnoc.structure.focus");
    await until("the STRUCTURE page to exist", () => api.sidebarOpen(), 30_000);
    await until("the page to draw its rows", () => (lastPost()?.rows ?? 0) > 0, 30_000);
    assert.deepStrictEqual(api.webviewErrors(), []);
  });

  test("with the canvas open, a click selects and reveals on the canvas, and opens no code tab", async () => {
    await vscode.commands.executeCommand("workbench.action.closeAllEditors");
    await vscode.commands.executeCommand("vnoc.openCanvas");
    await until("the canvas tab", () => canvasTabs().length === 1);
    await api.rowClick("main", first.id);
    await sleep(1000);
    assert.strictEqual(textTabs().length, 0, "no code tab was opened");
    assert.strictEqual(canvasTabs().length, 1);
    assert.strictEqual(api.session().contexts.main.selected, first.id);
    await until("the canvas to do the reveal", () => api.session().reveal === null);
  });

  test("with the canvas closed, a click opens one preview tab, and the next click replaces it", async () => {
    await closeCanvas();
    await vscode.commands.executeCommand("workbench.action.closeAllEditors");
    await api.rowClick("main", first.id);
    await until("a code tab", () => textTabs().length === 1);
    assert.ok(textTabs()[0].isPreview, "the tab is a preview tab");
    await api.rowClick("main", other.id);
    await until("the other file", () => textTabs().some((t) => (t.input as vscode.TabInputText).uri.fsPath.endsWith("other.py")));
    assert.strictEqual(textTabs().length, 1, "the preview tab was replaced, not added to");
  });

  test("`vnoc.open` is the same click, for the palette and for older callers", async () => {
    await closeCanvas();
    await vscode.commands.executeCommand("workbench.action.closeAllEditors");
    await vscode.commands.executeCommand("vnoc.open", first);
    await until("a code tab", () => textTabs().length === 1);
    assert.strictEqual(api.session().contexts.main.selected, first.id);
  });

  test("a selection made elsewhere (a card, a reveal) is reported by the page as the selected row, in view", async () => {
    await closeCanvas();
    await vscode.commands.executeCommand("workbench.action.closeAllEditors");
    api.dispatch({ type: "reveal", id: first.id, canvas: false }); // opens the ancestors and selects, as Reveal in Sidebar does
    await until("the page to name the revealed row, in view", () => lastPost()?.selected === first.id && lastPost()?.inView === true, 30_000);
    api.dispatch({ type: "reveal", id: other.id, canvas: false });
    await until("the page to follow the selection", () => lastPost()?.selected === other.id && lastPost()?.inView === true, 30_000);
    api.dispatch({ type: "select", id: first.id }); // what a click on a card sends: its row is there, the ancestors being open
    await until("the page to follow a plain select", () => lastPost()?.selected === first.id && lastPost()?.inView === true, 30_000);
    assert.strictEqual(textTabs().length, 0, "a selection from elsewhere opens no code");
  });

  test("a selection far down a long list is scrolled into view by the page itself", async () => {
    const file = (await api.structure()).root.children.find((n) => n.name === "sidebar_many.py")!;
    const last = file.children[119];
    // the list can only scroll when the view is shown and laid out: bring it up, and wait for the page to say it has a height
    await vscode.commands.executeCommand("vnoc.structure.focus");
    await until("the list to have a height", () => (lastPost()?.height ?? 0) > 0, 30_000);
    api.dispatch({ type: "expand", ids: [file.id] }); // the file open: its 120 functions are rows
    await until("120 functions as rows", () => (lastPost()?.rows ?? 0) > 120, 30_000);
    api.dispatch({ type: "select", id: file.children[0].id });
    await until("the first function in view", () => lastPost()?.selected === file.children[0].id && lastPost()?.inView === true, 30_000);
    api.dispatch({ type: "select", id: last.id });
    try {
      await until("the last function, 119 rows below, scrolled into view", () => lastPost()?.selected === last.id && lastPost()?.inView === true, 30_000);
    } catch (e) {
      throw new Error(`${(e as Error).message}; wanted ${last.id}; the page's last reports: ${JSON.stringify(api.sidebarPosts().slice(-4))}; session selected ${api.session().contexts.main.selected}`);
    }
    assert.deepStrictEqual(api.webviewErrors(), []);
  });

  test("Reveal in Sidebar selects the node and the page shows it", async () => {
    api.dispatch({ type: "focusJump", depth: 0 });
    api.dispatch({ type: "collapseAll" });
    await vscode.commands.executeCommand("vnoc.revealInSidebar", { nodeKey: first.id, webviewSection: "node" }); // what a card's or a row's menu passes
    await until("the page to show it", () => lastPost()?.selected === first.id && lastPost()?.inView === true, 30_000);
    assert.strictEqual(api.session().contexts.main.selected, first.id);
  });

  test("Focus from the menu (a row's context object) focuses the sidebar and the canvas on the node, and Back and Clear undo it", async () => {
    await vscode.commands.executeCommand("vnoc.focus", { nodeKey: firstFile.id, webviewSection: "node", webview: "vnoc.structure" });
    assert.deepStrictEqual([...api.session().contexts.main.focus].at(-1), firstFile.id);
    await until("the page to draw the focused node alone", () => (lastPost()?.rows ?? 99) <= firstFile.children.length + 1, 30_000);
    await vscode.commands.executeCommand("vnoc.focusBack");
    assert.deepStrictEqual([...api.session().contexts.main.focus], []);
  });

  test("moving through the search list points at nodes without selecting; closing it clears the pointer", async () => {
    // With the canvas open, cards are drawn, so a highlight really reaches them (it once crashed the page: a hook was skipped on the highlighted card).
    // The list is a QuickPick, and it closes the moment anything else takes the keyboard, which clears the pointer. So before it is opened, wait for what
    // could take the keyboard or change what it lists: the canvas page (VS Code runs the new tab's focus request when its page is ready, and the page says
    // it drew only after that), and the index (a node that goes while it is pointed at drops the pointer, correctly).
    const drawn = api.renderedPosts().length;
    await vscode.commands.executeCommand("vnoc.openCanvas");
    await until("the canvas tab", () => canvasTabs().length === 1);
    await until("the canvas page to draw, its focus request done", () => api.renderedPosts().length > drawn, 30_000);
    await structureSettled();
    api.dispatch({ type: "select", id: first.id });
    const selected = api.session().contexts.main.selected;
    const timeline = recordTimeline(() => api.session().highlight);
    try {
      await vscode.commands.executeCommand("vnoc.search");
      await until("the list's first item to be pointed at", () => api.session().highlight !== null);
      for (const step of ["next", "next again"]) {
        const was = api.session().highlight;
        await vscode.commands.executeCommand("workbench.action.quickOpenSelectNext");
        await until(`the pointer to move to ${step} item`, () => api.session().highlight !== null && api.session().highlight !== was);
      }
      assert.strictEqual(api.session().contexts.main.selected, selected, "a highlight never changes the selection");
      await vscode.commands.executeCommand("workbench.action.closeQuickOpen");
      await until("the pointer to clear", () => api.session().highlight === null);
    } catch (e) {
      throw new Error(`${(e as Error).message}; what happened, in ms from the start: ${timeline.log.join(" | ")}`);
    } finally {
      timeline.stop();
    }
  });

  test("a click selects the row; clicking the selected row again changes nothing in the contexts", async () => {
    await closeCanvas();
    await api.rowClick("main", first.id); // a known starting point
    await sleep(600);
    const before = api.session().contexts;
    await api.rowClick("main", other.id);
    await sleep(800);
    assert.strictEqual(api.session().contexts.main.selected, other.id);
    assert.notStrictEqual(api.session().contexts, before, "the selection is a change");
    // (the code that opens moves the cursor, which is not in `contexts`: `seq` would count it)
    const settled = api.session().contexts;
    await api.rowClick("main", other.id);
    await sleep(500);
    assert.strictEqual(api.session().contexts, settled, "clicking what is selected changes nothing");
  });
});

suite("the pages", () => {
  test("no V-NOC page crashed while the other tests ran", () => {
    assert.deepStrictEqual(api.webviewErrors(), []);
  });
});
