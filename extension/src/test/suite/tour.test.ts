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
  toursDrawn(): Extract<Notes, { type: "toursDrawn" }>[];
  toursClick(what: string): void;
  sidebarOpen(): boolean;
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
let second = "";
const vnoc = (args: string[], input = "") =>
  execFileSync(api.cli.binaryPath()!, ["--driver", api.cli.driverPaths().find((p) => p.includes("python"))!, ...args], { cwd: workspace(), encoding: "utf8", input, stdio: ["pipe", "pipe", "pipe"], env: { ...process.env, VNOC_DRIVER: "", VNOC_DRIVERS: "" } });
const tourCmd = (args: string[], input = "") => vnoc(["tour", ...args], input);

const last = (): Drawn | undefined => api.stageDrawn().at(-1);
/** Waits for the stage to draw something that satisfies `ready`; the newest drawing is the one asked. */
const stageShows = async (what: string, ready: (d: Drawn) => boolean, ms = 10_000, of = tour): Promise<Drawn> => {
  try {
    await until(what, () => {
      const d = last();
      return d !== undefined && d.tour === of && ready(d);
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
const toCover = async (of = tour) => {
  api.stageClick("again");
  await stageShows("the cover", (d) => d.mode === "cover", 10_000, of);
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

// ── the TOURS section of the sidebar (docs/26-tours/design/07-tour-list.md §2, 04 §1) ─────────────────────────────────────────────────────────

const section = () => api.toursDrawn().at(-1);
/** Waits for the section to draw something that satisfies `ready`. */
const sectionShows = async (what: string, ready: (d: NonNullable<ReturnType<typeof section>>) => boolean, ms = 10_000) => {
  try {
    await until(what, () => {
      const d = section();
      return d !== undefined && ready(d);
    }, ms);
  } catch (e) {
    throw new Error(`${(e as Error).message}; the section last drew ${JSON.stringify(section())}`);
  }
  return section()!;
};
const stageStep = (step: string) => stageShows(`the stage at ${step}`, atStep(step));

suite("the TOURS section", () => {
  suiteSetup(async function () {
    this.timeout(60_000);
    // a second, newer tour: one short stop (the list is newest first)
    await sleep(1100); // the tours are told apart by the second they were started in
    second = JSON.parse(tourCmd(["start", "--title", "Is there stock?", "--author", "the suite", "--json"])).tour;
    tourCmd(["add", second, "tourdemo/shop.py::has_stock", "--body-file", "-"], "has_stock answers whether there are enough units. It reads and never changes the count.\n");
    tourCmd(["finish", second, "--no-ids"]);
    await vscode.commands.executeCommand("vnoc.structure.focus"); // the sidebar's page, which holds the section
    await until("the sidebar page", () => api.sidebarOpen());
    await sectionShows("the section", () => true, 30_000);
  });

  test("it is folded until a tour is opened; unfolded, the list shows the suite's tours newest first, in the core's words", async () => {
    assert.strictEqual(section()!.folded, true, "folded by default");
    api.toursClick("unfold");
    const list = await sectionShows("the list", (d) => !d.folded && d.mode === "list" && d.list.length >= 2);
    assert.deepStrictEqual(list.list.slice(0, 2).map((r) => r.id), [second, tour], "newest first");
    assert.strictEqual(list.list[0].title, "Is there stock?");
    assert.strictEqual(list.list[0].words, "new · 1 stop", "never opened: new");
    assert.strictEqual(list.list[1].question, "what happens when the payment times out");
    assert.ok(list.list[1].words.endsWith(" · 4 stops"), list.list[1].words);
    assert.ok(list.count >= 2);
  });

  test("a click on a row of a tour never opened opens it at the cover, with its route in the section", async () => {
    api.toursClick(`row:${second}`);
    await until("the second tour's context", () => api.session().active === `tour:${second}`);
    const cover = await stageShows("the cover", (d) => d.mode === "cover", 10_000, second);
    assert.strictEqual(cover.title, "Is there stock?");
    const route = await sectionShows("the route", (d) => d.mode === "route" && d.route.length === 1);
    assert.deepStrictEqual(route.route.map((r) => [r.id, r.kind, r.mark, r.current]), [["s1", "stop", "ahead", false]], "at the cover nothing is here");
    assert.ok(route.header.startsWith("‹ Tours ▶ Is there stock?"), route.header);
    assert.strictEqual(route.folded, false);
  });

  test("leave a tour on stop 3: its row says so, and a click on the row returns to that place", async () => {
    api.toursClick(`row:${tour}`);
    await until("the first tour's context (it replaces the second)", () => api.session().active === `tour:${tour}`);
    await toCover();
    for (let i = 0; i < 6; i++) api.stageClick("next"); // s1, s1.b1, s1.b2, s1.b3, s2, s4: stop 3 of the route
    const here = await stageStep("s4");
    assert.strictEqual(here.position, "stop 3 of 4");
    const route = await sectionShows("the route on stop 3", (d) => d.mode === "route" && d.route.some((r) => r.id === "s4" && r.current));
    assert.deepStrictEqual(route.route.filter((r) => r.kind === "stop").map((r) => [r.id, r.number, r.mark]), [["s1", 1, "seen"], ["s2", 2, "seen"], ["s4", 3, "here"], ["s3", 4, route.route.find((r) => r.id === "s3")!.mark]], "s3 was read by an earlier test: its mark is the core's");
    assert.notStrictEqual(route.route.find((r) => r.id === "s3")!.mark, "here");
    assert.strictEqual(route.route.find((r) => r.id === "s4")!.description, "⇢ 2", "explained at stop 2");
    // leave: the tour closes, the section shows the list, and the row says where it was left
    api.toursClick("close");
    await until("MAIN to be active", () => api.session().active === "main");
    const list = await sectionShows("the list with the tour's row on stop 3", (d) => d.mode === "list" && d.list.some((r) => r.id === tour && r.words.startsWith("stop 3 · ")));
    assert.strictEqual(list.list.find((r) => r.id === tour)!.words, "stop 3 · 4 stops");
    // a click on the row: the tour opens where it was left, not at the cover
    api.toursClick(`row:${tour}`);
    await until("the tour's context", () => api.session().active === `tour:${tour}`);
    const back = await stageStep("s4");
    assert.strictEqual(back.position, "stop 3 of 4");
  });

  test("‹ Tours shows the list and the tour stays open; its row has the bar and a click on it brings the route back", async () => {
    api.toursClick("back");
    const list = await sectionShows("the list", (d) => d.mode === "list" && !d.folded);
    assert.strictEqual(list.list.find((r) => r.id === tour)!.active, true, "the open tour's row is the active one");
    assert.ok(api.session().order.includes(`tour:${tour}`), "the tour's context is still there");
    assert.strictEqual(api.session().active, `tour:${tour}`);
    assert.strictEqual(last()!.step, "s4", "the stage did not move");
    api.toursClick(`row:${tour}`);
    const route = await sectionShows("the route again", (d) => d.mode === "route");
    assert.ok(route.route.some((r) => r.id === "s4" && r.current));
  });

  test("a click on a stop of the route moves the stage, and on a step of the stop you are in", async () => {
    api.toursClick("stop:s2");
    await stageStep("s2");
    api.toursClick("stop:s1");
    await stageStep("s1");
    // the steps of s1 are under it now, indented by their depth: a block, and the detail under it
    const route = await sectionShows("the steps of s1", (d) => d.route.some((r) => r.kind === "step"));
    assert.deepStrictEqual(route.route.filter((r) => r.kind === "step").map((r) => [r.id, r.text, r.depth]), [["s1", "what it is for", 0], ["s1.b1", "Refuse what is not there", 1], ["s1.b2", "Hold, then charge", 1], ["s1.b3", "Give it back on a timeout", 2]]);
    api.toursClick("step:s1.b3");
    const detail = await stageStep("s1.b3");
    assert.strictEqual(detail.kind, "detail");
    await sectionShows("the detail current", (d) => d.route.some((r) => r.id === "s1.b3" && r.current));
    // a terminal's move shows in the route too
    tourCmd(["go", tour, "s3"]);
    await sectionShows("the route follows the terminal", (d) => d.route.some((r) => r.id === "s3" && r.current));
  });

  test("“called on line N” jumps to the step of its caller that holds the line", async () => {
    api.toursClick("stop:s2"); // reached from s1 at line 6, which block s1.b1 (6-7) holds
    await stageStep("s2");
    api.stageClick("calledFrom");
    const step = await stageStep("s1.b1");
    assert.strictEqual(step.lit.join(","), "6,7");
  });

  test("the keys: ↓ ↑ move the focus, Enter opens or jumps, Escape in the route goes back to the list", async () => {
    api.toursClick("key:Escape"); // the route: back to the list
    await sectionShows("the list", (d) => d.mode === "list");
    api.toursClick("key:ArrowDown"); // the first row (the newest)
    api.toursClick("key:ArrowDown"); // the second: the suite's first tour
    api.toursClick("key:Enter");
    await until("the first tour opened by the keys", () => api.session().active === `tour:${tour}`);
    await sectionShows("its route", (d) => d.mode === "route");
    api.toursClick("key:ArrowUp"); // from the current stop to the one above, then jump
    api.toursClick("key:Enter");
    await until("a jump by the keys", () => last()!.stop !== "s3");
  });

  test("the end card says how many stops were not seen: the core's count", async () => {
    api.toursClick("key:Escape"); // the list
    await sectionShows("the list", (d) => d.mode === "list");
    api.toursClick(`row:${second}`); // never read: its one stop is not seen
    await until("the second tour", () => api.session().active === `tour:${second}`);
    await toCover(second);
    api.stageClick("end");
    const end = await stageShows("the end card", (d) => d.mode === "end", 10_000, second);
    assert.strictEqual(end.unseen, 1, "one written stop, not read to its last step");
    api.stageClick("again");
    await stageShows("the cover", (d) => d.mode === "cover", 10_000, second);
    api.stageClick("next");
    await stageShows("its only step", (d) => d.step === "s1", 10_000, second);
    api.stageClick("end");
    const done = await stageShows("the end card, read", (d) => d.mode === "end" && d.unseen === 0, 10_000, second);
    assert.strictEqual(done.unseen, 0, "all seen");
    api.stageClick("closeTour");
    await until("MAIN to be active", () => api.session().active === "main");
    assert.deepStrictEqual(api.webviewErrors(), []);
  });
});
