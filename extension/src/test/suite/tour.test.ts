import * as assert from "assert";
import { execFileSync } from "child_process";
import * as fs from "fs";
import * as path from "path";
import * as vscode from "vscode";

import type { Notes, StructureResult } from "@vnoc/shared/messages";
import type { Session } from "@vnoc/shared/session.ts";
import { canvasTabs, sleep, until } from "./util";

// A tour plays in a real window (docs/26-tours/06-build-order.md TR17). The tour is made by the built `vnoc tour` commands, so its hashes are real: a stop with blocks and
// a detail, a short stop, a file with no blocks (contents), and a stop explained before. The suite cannot see the page, so it reads what the stage posts
// (`stageDrawn`), and presses keys and buttons through the host (`stageKey`, `stageClick`: it cannot press or click in a webview).

type Drawn = Extract<Notes, { type: "stageDrawn" }>;
interface Api {
  structure(): Promise<StructureResult>;
  session(): Session;
  stageDrawn(): Drawn[];
  stageKey(key: string): void;
  stageClick(button: string): void;
  webviewErrors(): string[];
  cli: { binaryPath(): string | undefined; driverPaths(): string[] };
  tours: { open(): unknown; list(): Promise<{ id: string; words: string }[]> };
}
let api: Api;

const workspace = () => vscode.workspace.workspaceFolders![0].uri.fsPath;

// the code the tour is about: `checkout` is 13 lines (an intro and blocks), `has_stock` is 2 (a short stop)
const SHOP = `"""The shop, for the tour suite."""


def checkout(order, stock):
    """Hold the stock, then charge."""
    if not has_stock(stock, order):
        raise ValueError("out of stock")

    reserve(stock, order)
    try:
        receipt = charge(order)
    except TimeoutError:
        release(stock, order)
        raise

    return receipt


def has_stock(stock, order):
    return stock.get(order["sku"], 0) >= order["qty"]


def reserve(stock, order):
    stock[order["sku"]] -= order["qty"]


def charge(order):
    return {"paid": order["qty"] * 3}


def release(stock, order):
    stock[order["sku"]] += order["qty"]
`;

const CHECKOUT = `checkout holds the stock first, then charges, and gives the stock back when the charge times out.

## 6-7 Refuse what is not there
It looks before it holds anything, so nothing is held for an order that cannot be filled.

## 9-14 Hold, then charge
The stock is held before the money is taken, so a second buyer cannot take it while the payment runs.

### 11-14 Give it back on a timeout
A timeout is the one failure it knows how to undo: the units go back, then the error goes up.
`;

let tour = "";
const vnoc = (args: string[], input = "") =>
  execFileSync(api.cli.binaryPath()!, ["--driver", api.cli.driverPaths().find((p) => p.includes("python"))!, ...args], { cwd: workspace(), encoding: "utf8", input, stdio: ["pipe", "pipe", "pipe"], env: { ...process.env, VNOC_DRIVER: "", VNOC_DRIVERS: "" } });
const tourCmd = (args: string[], input = "") => vnoc(["tour", ...args], input);

const last = (): Drawn | undefined => api.stageDrawn().at(-1);
/** Waits for the stage to draw something that satisfies `ready`; the newest drawing is the one asked. */
const stageShows = async (what: string, ready: (d: Drawn) => boolean, ms = 10_000): Promise<Drawn> => {
  try {
    await until(what, () => {
      const d = last();
      return d !== undefined && d.tour === tour && ready(d);
    }, ms);
  } catch (e) {
    // what the page last drew, and if it crashed, why
    throw new Error(`${(e as Error).message}; the stage last drew ${JSON.stringify({ ...last(), code: undefined })}; the page's errors: ${JSON.stringify(api.webviewErrors())}`);
  }
  // to watch it: `VNOC_SLOW=1200` waits that long (ms) on every step; `VNOC_SHOT=<file>` writes that file at the second block, and holds it for 6 s, for a screenshot
  await sleep(Number(process.env.VNOC_SLOW ?? 0));
  if (process.env.VNOC_SHOT && last()!.step === "s1.b3") {
    fs.writeFileSync(process.env.VNOC_SHOT, "ready");
    await sleep(6000);
  }
  return last()!;
};
const atStep = (step: string) => (d: Drawn) => d.step === step;
const toCover = async () => {
  api.stageClick("again");
  await stageShows("the cover", (d) => d.mode === "cover");
};

suiteSetup(async function () {
  this.timeout(120_000);
  api = await vscode.extensions.getExtension("vnoc.vnoc")!.activate();
  // the code, in a folder of its own; the core reads it when the watcher says so
  const dir = path.join(workspace(), "tourdemo");
  fs.mkdirSync(dir, { recursive: true });
  fs.writeFileSync(path.join(dir, "shop.py"), SHOP);
  for (let waited = 0; ; waited += 500) {
    const structure = await api.structure();
    const file = structure.root.children.find((c) => c.id === "tourdemo");
    if (file?.children.some((c) => c.id === "tourdemo/shop.py" && c.children.length >= 5)) break;
    assert.ok(waited < 30_000, "the structure never held tourdemo/shop.py");
    await sleep(500);
  }
  // the tour, as an agent writes it: start, add with heredoc bodies, finish
  const started = JSON.parse(tourCmd(["start", "--title", "How the shop checks out", "--question", "what happens when the payment times out", "--author", "the suite", "--json"]));
  tour = started.tour;
  const add = (node: string, body: string, ...flags: string[]) => tourCmd(["add", tour, node, "--body-file", "-", ...flags], body);
  add("tourdemo/shop.py::checkout", CHECKOUT);
  add("tourdemo/shop.py::has_stock", "has_stock answers one question: are there enough units of this SKU in stock? It reads and never changes the count.\n", "--from", "s1", "--line", "6");
  add("tourdemo/shop.py", "shop.py is the whole shop: checkout runs the sale, and the four small functions under it each do one thing.\n");
  add("tourdemo/shop.py::has_stock", "Called again here: checkout does not trust its caller to have checked.\n", "--same-as", "s2", "--from", "s1", "--line", "6");
  tourCmd(["finish", tour, "--no-ids"]);
});

/** Closes the code tabs and leaves the canvas: `closeAllEditors` would close the canvas with them. */
const closeCodeTabs = async () => {
  await vscode.window.tabGroups.close(vscode.window.tabGroups.all.flatMap((g) => g.tabs).filter((t) => t.input instanceof vscode.TabInputText));
};

suiteTeardown(async () => {
  await closeCodeTabs();
});

suite("a tour on the canvas", () => {
  test("Open Tour… lists the tour, and picking it opens the tour's context on the cover", async () => {
    const rows = await api.tours.list();
    assert.strictEqual(rows[0]?.id, tour, `the core lists the tour first (newest first): ${JSON.stringify(rows.map((r) => [r.id, r.words]))}`);
    assert.ok(rows[0].words.startsWith("new · 4 stops"), `a row says its words: ${rows[0].words}`);
    const open = vscode.commands.executeCommand("vnoc.openTour"); // waits for the pick
    // the QuickPick is shown when the core's list has arrived: accept the first row (newest first, and ours is the newest) until the context is there
    for (let waited = 0; api.session().active !== `tour:${tour}`; waited += 600) {
      assert.ok(waited < 20_000, `the tour's context never opened: ${JSON.stringify(api.session().order)}`);
      await sleep(600);
      // the list's first row is the active one when it opens; Next then Previous makes sure the picker has the keyboard
      await vscode.commands.executeCommand("workbench.action.quickOpenSelectNext");
      await vscode.commands.executeCommand("workbench.action.quickOpenSelectPrevious");
      await vscode.commands.executeCommand("workbench.action.acceptSelectedQuickOpenItem");
    }
    await open;
    await until("the canvas tab", () => canvasTabs().length > 0);
    const cover = await stageShows("the cover", (d) => d.mode === "cover");
    assert.strictEqual(cover.title, "How the shop checks out");
    assert.strictEqual(cover.next, "Start");
    assert.strictEqual(cover.code, null, "the cover is one column");
    assert.strictEqual(api.session().contexts[`tour:${tour}`].tour, tour);
    assert.strictEqual(api.session().contexts[`tour:${tour}`].selected, "tourdemo/shop.py::checkout", "the first stop is the selection at the cover");
  });

  test("Next walks to the end card through a block and its details, and Prev walks back", async () => {
    await toCover();
    const steps: [string, string, string, string | null, number[], number[]][] = [
      // step, kind, position, title, lit lines, thin lines
      ["s1", "intro", "stop 1 of 4 · 1 of 4", null, [], [4]],
      ["s1.b1", "block", "stop 1 of 4 · 2 of 4", "Refuse what is not there", [6, 7], []],
      ["s1.b2", "block", "stop 1 of 4 · 3 of 4", "Hold, then charge", [9, 10, 11, 12, 13, 14], []],
      ["s1.b3", "detail", "stop 1 of 4 · 4 of 4", "Give it back on a timeout", [11, 12, 13, 14], [9, 10]],
      ["s2", "short", "stop 2 of 4", null, [19, 20], []],
      // s4 hangs from s1 (`--from s1`), so it comes before s3 on the route, which has no parent: the core's order, not the order they were written in
      ["s4", "same_as", "stop 3 of 4", null, [], []],
      ["s3", "contents", "stop 4 of 4", null, [], []],
    ];
    const check = (d: Drawn, [step, kind, position, title, lit, thin]: (typeof steps)[number]) => {
      assert.deepStrictEqual([d.step, d.kind, d.position, d.title, d.lit, d.thin], [step, kind, position, title, lit, thin], `at ${step}`);
    };
    for (const row of steps) {
      api.stageClick("next");
      check(await stageShows(`${row[0]}`, atStep(row[0])), row);
    }
    assert.strictEqual(last()!.next, "Finish", "the last step says Finish");
    // the code pane of each: the node's code for a block, the contents of a file, the same code dim for a stop explained before
    assert.strictEqual(steps.length, 7);
    api.stageClick("next");
    const end = await stageShows("the end card", (d) => d.mode === "end");
    assert.strictEqual(end.step, null);
    assert.strictEqual(end.code, null, "the end is one column");
    for (const row of [...steps].reverse()) {
      api.stageClick("prev");
      check(await stageShows(`back to ${row[0]}`, atStep(row[0])), row);
    }
    api.stageClick("prev");
    await stageShows("back to the cover", (d) => d.mode === "cover");
    assert.deepStrictEqual(api.webviewErrors(), []);
  });

  test("a stop of the tour: the code pane shows the file's own lines, with their numbers; a file stop shows no code", async () => {
    await toCover();
    api.stageClick("next");
    const s1 = await stageShows("the intro", atStep("s1"));
    const lines = fs.readFileSync(path.join(workspace(), "tourdemo/shop.py"), "utf8").split("\n");
    assert.strictEqual(s1.code?.first, 4, "checkout starts at line 4");
    assert.strictEqual(s1.code?.text, lines.slice(3, 16).join("\n"), "the code is the file's lines 4 to 16");
    assert.ok(s1.header.includes("checkout") && s1.header.includes("tourdemo/shop.py"), s1.header);
    api.stageClick("next");
    api.stageClick("next");
    const block = await stageShows("the second block", atStep("s1.b2"));
    assert.strictEqual(block.code?.text, s1.code?.text, "a block shows the same code, with other lines lit");
    for (let i = 0; i < 4; i++) api.stageClick("next"); // s1.b3, s2, s4, s3
    const file = await stageShows("the file stop", atStep("s3"));
    assert.strictEqual(file.code, null, "contents are rows, not code");
    assert.strictEqual(file.text?.startsWith("shop.py is the whole shop"), true);
  });

  test("the keys → ← Space Home End move the place, and Home and End go to the cover and the end", async () => {
    await toCover();
    api.stageKey("ArrowRight");
    await stageShows("→ to the first step", atStep("s1"));
    api.stageKey(" ");
    await stageShows("Space to the next", atStep("s1.b1"));
    api.stageKey("ArrowRight");
    await stageShows("→ again", atStep("s1.b2"));
    api.stageKey("ArrowLeft");
    await stageShows("← back", atStep("s1.b1"));
    api.stageKey("End");
    await stageShows("End: the end card", (d) => d.mode === "end");
    api.stageKey("ArrowLeft");
    await stageShows("← from the end: the last step", atStep("s3"));
    api.stageKey("Home");
    await stageShows("Home: the cover", (d) => d.mode === "cover");
    // the canvas's own arrows did not move a card: the tour context's selection is the current stop's
    assert.strictEqual(api.session().contexts[`tour:${tour}`].selected, "tourdemo/shop.py::checkout");
  });

  test("the selection follows the stop: the node of the stop the place is on", async () => {
    await toCover();
    for (let i = 0; i < 5; i++) api.stageClick("next"); // s1, s1.b1, s1.b2, s1.b3, s2
    await stageShows("the short stop", atStep("s2"));
    await until("the selection to follow", () => api.session().contexts[`tour:${tour}`].selected === "tourdemo/shop.py::has_stock");
  });

  test("Go to Code opens the file at the step's first line", async () => {
    await toCover();
    for (let i = 0; i < 3; i++) api.stageClick("next");
    await stageShows("the second block", atStep("s1.b2"));
    await closeCodeTabs();
    api.stageClick("goToCode");
    await until("the editor at line 9", () => {
      const e = vscode.window.activeTextEditor;
      return !!e && e.document.uri.fsPath.endsWith(path.join("tourdemo", "shop.py")) && e.selection.active.line === 8;
    });
    await closeCodeTabs();
  });

  test("`vnoc tour go` run from a terminal moves the stage, and the place is the one the core keeps", async () => {
    await toCover();
    tourCmd(["go", tour, "s2"]);
    const moved = await stageShows("the stage to follow the terminal", atStep("s2"));
    assert.strictEqual(moved.kind, "short");
    tourCmd(["go", tour, "s1.b3"]);
    await stageShows("the stage to follow a second move", atStep("s1.b3"));
    tourCmd(["go", tour, "cover"]);
    await stageShows("the cover again", (d) => d.mode === "cover");
  });

  test("after three clicks on Next, `vnoc tour where` says the step the stage shows", async () => {
    await toCover();
    for (let i = 0; i < 3; i++) api.stageClick("next");
    const shown = await stageShows("three steps in", atStep("s1.b2"));
    const where = JSON.parse(tourCmd(["where", tour, "--json"]));
    assert.strictEqual(where.step.id, shown.step, "a terminal sees the place the stage is at");
    assert.strictEqual(where.step.id, "s1.b2");
    assert.deepStrictEqual(api.webviewErrors(), []);
  });

  test("closing the tour gives MAIN back", async () => {
    api.stageClick("closeTour");
    await until("MAIN to be active", () => api.session().active === "main" && api.session().order.length >= 1 && !api.session().order.some((id) => id.startsWith("tour:")));
  });
});
