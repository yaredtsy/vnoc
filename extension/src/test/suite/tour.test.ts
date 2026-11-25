import * as assert from "assert";
import { execFileSync } from "child_process";
import * as fs from "fs";
import * as path from "path";
import * as vscode from "vscode";

import type { Notes, StructureResult } from "@vnoc/shared/messages";
import type { Session } from "@vnoc/shared/session.ts";
import { canvasTabs, closeCanvas, sleep, until } from "./util";

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
  mapDrawn(): Extract<Notes, { type: "mapDrawn" }>[];
  tourBar(): Extract<Notes, { type: "tourBar" }>[];
  chips(): { id: string; label: string; active: boolean; dot?: "pulsing" | "still" }[];
  mapClick(stop: string): void;
  setView(text: string): void;
  viewports(): Record<string, { x: number; y: number; zoom: number }>;
  dispatch(intent: unknown, context?: string): void;
  sidebarOpen(): boolean;
  webviewErrors(): string[];
  cli: { binaryPath(): string | undefined; driverPaths(): string[] };
  tours: { open(): unknown; list(): Promise<{ id: string; words: string }[]> };
  tourStatus(): string | null;
  restartCore(): Promise<void>;
  toursNoticed(): string[];
  toursNoticeOpen(id: string): Promise<void> | undefined;
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
    const coreAt = (() => {
      try {
        return JSON.parse(tourCmd(["where", of, "--json"])).at;
      } catch {
        return "?";
      }
    })();
    throw new Error(`${(e as Error).message}; the core's place is ${coreAt}; the stage last drew ${JSON.stringify({ ...last(), code: undefined })}; the page's errors: ${JSON.stringify(api.webviewErrors())}`);
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
  // the core has the cover before the next click is sent: two moves sent together are two requests that may overtake each other (and when the stage already shows the cover
  // there is nothing new to wait for on the page)
  await until("the core's place to be the cover", () => JSON.parse(tourCmd(["where", of, "--json"])).at === "cover");
  await stageShows("the cover", (d) => d.mode === "cover", 10_000, of);
};

suiteSetup(async () => {
  api = await vscode.extensions.getExtension("vnoc.vnoc")!.activate();
});

/** The code and the tours of this suite. Not a root hook: every suite of the run shares the window and its workspace, and the ones before this must not see these files. */
async function makeTours() {
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
}

/** Closes the code tabs and leaves the canvas: `closeAllEditors` would close the canvas with them. */
const closeCodeTabs = async () => {
  await vscode.window.tabGroups.close(vscode.window.tabGroups.all.flatMap((g) => g.tabs).filter((t) => t.input instanceof vscode.TabInputText));
};

suiteTeardown(async () => {
  await closeCodeTabs();
});

suite("a tour on the canvas", () => {
  suiteSetup(async function () {
    this.timeout(120_000);
    await makeTours();
  });

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

  test("it folds and unfolds; unfolded, the list shows the suite's tours newest first, in the core's words", async () => {
    api.toursClick("fold"); // folded, it says its name and the count (it is unfolded when a tour was opened before: the page keeps it)
    await sectionShows("the section folded", (d) => d.folded);
    api.toursClick("unfold");
    const list = await sectionShows("the list", (d) => !d.folded && d.mode === "list" && d.list.length >= 2);
    assert.deepStrictEqual(list.list.slice(0, 2).map((r) => r.id), [second, tour], "newest first");
    assert.strictEqual(list.list[0].title, "Is there stock?");
    assert.match(list.list[0].words, /^new · 1 stop · (just now|\d+ min ago)$/, "never opened: new, the size, and when it was started");
    assert.strictEqual(list.list[1].question, "what happens when the payment times out");
    assert.ok(/ · 4 stops · /.test(list.list[1].words), list.list[1].words);
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
    assert.match(list.list.find((r) => r.id === tour)!.words, /^stop 3 · 4 stops · (just now|\d+ min ago)$/);
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

  test("the end card says how many stops were not seen, and its first button goes to the first one skipped: the core's words", async () => {
    api.toursClick("key:Escape"); // the list
    await sectionShows("the list", (d) => d.mode === "list");
    api.toursClick(`row:${second}`); // never read: its one stop is not seen
    await until("the second tour", () => api.session().active === `tour:${second}`);
    await toCover(second);
    api.stageClick("end");
    const end = await stageShows("the end card", (d) => d.mode === "end", 10_000, second);
    assert.strictEqual(end.unseen, 1, `one written stop, not read to its last step; the core says ${tourCmd(["where", second, "--json"])}; the stage drew ${JSON.stringify(api.stageDrawn().slice(-6).map((d) => [d.tour === second ? "second" : "other", d.mode, d.step, d.unseen]))}`);
    assert.strictEqual(end.firstUnseen, "s1", "the core names the first one skipped");
    api.stageClick("skipped"); // the button: a jump to it
    await stageShows("the stop that was skipped", (d) => d.step === "s1", 10_000, second);
    api.stageClick("end");
    const done = await stageShows("the end card, read", (d) => d.mode === "end" && d.unseen === 0, 10_000, second);
    assert.strictEqual(done.firstUnseen, null, "all seen: nothing to go back to");
    api.stageClick("again");
    await stageShows("the cover", (d) => d.mode === "cover", 10_000, second);
    api.stageClick("closeTour");
    await until("MAIN to be active", () => api.session().active === "main");
    assert.deepStrictEqual(api.webviewErrors(), []);
  });
});

// ── the tour on the map (docs/26-tours/design/04-route-and-map.md §2 – §4) ──────────────────────────────────────────────────────────────────

const map = () => api.mapDrawn().filter((m) => m.tour === tour).at(-1);
const mapShows = async (what: string, ready: (m: NonNullable<ReturnType<typeof map>>) => boolean, ms = 10_000) => {
  try {
    await until(what, () => {
      const m = map();
      return m !== undefined && ready(m);
    }, ms);
  } catch (e) {
    throw new Error(`${(e as Error).message}; the map last drew ${JSON.stringify(map())}`);
  }
  return map()!;
};
const near = (a: { x: number; y: number; zoom: number } | undefined, x: number, y: number, zoom: number) => !!a && Math.abs(a.x - x) < 0.5 && Math.abs(a.y - y) < 0.5 && Math.abs(a.zoom - zoom) < 0.005;

suite("the tour on the map", () => {
  suiteSetup(async function () {
    this.timeout(60_000);
    api.toursClick("key:Escape"); // the list, if a route is shown
    api.toursClick(`row:${tour}`);
    await until("the tour's context", () => api.session().active === `tour:${tour}`);
    await toCover();
    for (let i = 0; i < 3; i++) api.stageClick("next");
    await stageStep("s1.b2");
  });

  test("the map has one card for every stop, in the route's order, with the ring on the current one", async () => {
    const m = await mapShows("the map at s1", (d) => d.cards.some((c) => c.here && c.id === "s1"));
    assert.deepStrictEqual(m.cards.map((c) => [c.id, c.number]), [["s1", 1], ["s2", 2], ["s4", 3], ["s3", 4]]);
    assert.deepStrictEqual(m.cards.filter((c) => c.here).map((c) => c.id), ["s1"], "one ring, on the stop the stage is on");
    assert.ok(m.cards.every((c) => !c.planned), "a finished tour has no stop that is not written");
    assert.strictEqual(m.cards.find((c) => c.id === "s1")!.mark, "here");
  });

  test("a stop reached by a call is a call card with its line, joined to its caller by a dashed edge; one with no caller has no edge", async () => {
    const m = map()!;
    assert.deepStrictEqual(m.cards.map((c) => [c.id, c.callLine]), [["s1", null], ["s2", 6], ["s4", 6], ["s3", null]]);
    assert.deepStrictEqual([...m.edges].sort(), ["s1->s2:call", "s1->s4:call"]);
    assert.strictEqual(m.cards.find((c) => c.id === "s4")!.description, "⇢ 2", "explained at stop 2");
    assert.deepStrictEqual(m.heavy, [], "the current stop is the first: no way to it");
  });

  test("a click on a card is a jump to the stop: the stage moves, the ring moves, and the edges from the first stop are heavy", async () => {
    api.mapClick("s4");
    await stageStep("s4");
    const m = await mapShows("the ring on s4", (d) => d.cards.some((c) => c.here && c.id === "s4"));
    assert.deepStrictEqual(m.heavy, ["s1->s4"]);
    assert.deepStrictEqual(m.cards.filter((c) => c.here).map((c) => c.id), ["s4"]);
    api.mapClick("s1");
    await stageStep("s1");
  });

  test("a call in the code that is a later stop of the tour is a jump to it", async () => {
    assert.ok(last()!.calls.includes(6), `the code of checkout leads to the stops reached from line 6: ${JSON.stringify(last()!.calls)}`);
    api.stageClick("callLine:6");
    await stageStep("s2"); // the first stop on the route reached from that line
    api.stageClick("jump:s1");
    await stageStep("s1");
  });

  test("Main ⇄ tour keeps the step and both viewports", async () => {
    const context = `tour:${tour}`;
    api.stageClick("jump:s2");
    await stageStep("s2");
    api.setView("120,60,0.8"); // the tour's own pan and zoom
    await until("the tour's viewport", () => near(api.viewports()[context], 120, 60, 0.8));
    api.dispatch({ type: "activate", id: "main" }, "main");
    await until("MAIN to be active", () => api.session().active === "main");
    await sleep(600);
    api.setView("40,30,0.6"); // MAIN's
    await until("MAIN's viewport", () => near(api.viewports().main, 40, 30, 0.6));
    delete api.viewports()[context];
    delete api.viewports().main;
    api.dispatch({ type: "activate", id: context }, context);
    await until("the tour to be active again", () => api.session().active === context);
    await until("the tour's viewport as it was", () => near(api.viewports()[context], 120, 60, 0.8));
    await stageStep("s2");
    assert.strictEqual(last()!.position, "stop 2 of 4", "the step is the one it was");
    api.dispatch({ type: "activate", id: "main" }, "main");
    await until("MAIN's viewport as it was", () => near(api.viewports().main, 40, 30, 0.6));
    api.dispatch({ type: "activate", id: context }, context);
    await until("the tour again", () => api.session().active === context);
    assert.deepStrictEqual(api.webviewErrors(), []);
  });
});

// ── the bar, the menus and Esc (docs/26-tours/design/04-route-and-map.md §5, §7) ────────────────────────────────────────────────────────────

const bar = () => api.tourBar().at(-1);
const barShows = async (what: string, ready: (b: NonNullable<ReturnType<typeof bar>>) => boolean, ms = 10_000) => {
  try {
    await until(what, () => {
      const b = bar();
      return b !== undefined && ready(b);
    }, ms);
  } catch (e) {
    throw new Error(`${(e as Error).message}; the bar last said ${JSON.stringify(bar())}`);
  }
  return bar()!;
};

suite("the tour's bar", () => {
  const context = () => `tour:${tour}`;
  suiteSetup(async function () {
    this.timeout(60_000);
    api.toursClick("key:Escape");
    api.toursClick(`row:${tour}`);
    await until("the tour's context", () => api.session().active === context());
    await toCover();
    api.stageClick("jump:s2");
    await stageStep("s2");
  });

  test("the chip is `▶ title` and the crumbs are the stops from the first to the current one", async () => {
    const chip = api.chips().find((c) => c.id === context());
    assert.deepStrictEqual([chip?.label, chip?.active], ["▶ How the shop checks out", true]);
    assert.deepStrictEqual((await barShows("the crumbs", (b) => b.crumbs.length === 2)).crumbs, ["checkout", "has_stock"], "s2 was reached from s1");
    api.stageClick("jump:s3"); // reached from no stop: its own crumb
    await stageStep("s3");
    assert.deepStrictEqual((await barShows("one crumb", (b) => b.crumbs.length === 1)).crumbs, ["shop.py"]);
    api.stageClick("jump:s4"); // from s1
    await stageStep("s4");
    assert.deepStrictEqual((await barShows("two crumbs", (b) => b.crumbs.length === 2)).crumbs, ["checkout", "has_stock"]);
  });

  test("← is back to where you were, Alt+← is the same, and it is disabled with nothing to go back to", async () => {
    await toCover();
    api.stageClick("jump:s2"); // a jump: the cover is where we were
    await stageStep("s2");
    await barShows("← enabled", (b) => b.back);
    api.stageKey("Alt+ArrowLeft");
    await stageShows("back at the cover", (d) => d.mode === "cover");
    assert.strictEqual(api.session().active, context(), "back is the tour's own, not MAIN's focus");
  });

  test("`vnoc tour go` from a terminal says `moved by the agent` for a few seconds, and a click of the reader's does not", async () => {
    api.stageClick("jump:s2");
    await stageStep("s2");
    await sleep(500);
    assert.strictEqual(bar()!.moved, false, "the reader moved it");
    const posted = api.tourBar().length;
    tourCmd(["go", tour, "s4"]);
    await stageStep("s4");
    // every drawing of the bar since, not the last one: the words are on for three seconds, and a slow machine can look at them late
    await until("moved by the agent", () => api.tourBar().slice(posted).some((b) => b.moved));
    await barShows("the words go", (b) => !b.moved, 8_000);
  });

  test("Start Again goes to the cover", async () => {
    api.stageClick("jump:s2");
    await stageStep("s2");
    api.stageClick("menuStartAgain");
    await stageShows("the cover", (d) => d.mode === "cover");
  });

  test("the menus of a stop: Reveal in Main makes MAIN active on that node and the tour stays where it was; Go to Code opens the file", async () => {
    api.stageClick("jump:s2");
    await stageStep("s2");
    await vscode.commands.executeCommand("vnoc.tourRevealInMain", { nodeKey: "tourdemo/shop.py::has_stock" });
    await until("MAIN to be active, on the node", () => api.session().active === "main" && api.session().contexts.main.selected === "tourdemo/shop.py::has_stock");
    assert.ok(api.session().order.includes(context()), "the tour is still open");
    api.dispatch({ type: "activate", id: context() }, context());
    await stageStep("s2");
    await closeCodeTabs();
    await vscode.commands.executeCommand("vnoc.tourGoToCode", { nodeKey: "tourdemo/shop.py::has_stock" });
    await until("the editor on has_stock", () => {
      const e = vscode.window.activeTextEditor;
      return !!e && e.document.uri.fsPath.endsWith(path.join("tourdemo", "shop.py")) && e.selection.active.line === 18;
    });
    await closeCodeTabs();
    // the same entries from the bar's ⋯: Go to Code on the current stop, Reveal Root in Main
    api.stageClick("menuGoToCode");
    await until("the editor on the current stop", () => !!vscode.window.activeTextEditor?.document.uri.fsPath.endsWith(path.join("tourdemo", "shop.py")));
    await closeCodeTabs();
    api.stageClick("menuRevealRoot");
    await until("MAIN on the first stop's node", () => api.session().active === "main" && api.session().contexts.main.selected === "tourdemo/shop.py::checkout");
    api.dispatch({ type: "activate", id: context() }, context());
    await until("the tour again", () => api.session().active === context());
  });

  test("Esc closes the tour's context and MAIN is as it was: its selection, its focus and its viewport", async () => {
    api.dispatch({ type: "activate", id: "main" }, "main");
    await until("MAIN", () => api.session().active === "main");
    await sleep(600);
    api.setView("50,40,0.7");
    await until("MAIN's viewport", () => near(api.viewports().main, 50, 40, 0.7));
    const before = JSON.stringify(api.session().contexts.main);
    api.dispatch({ type: "activate", id: context() }, context());
    await until("the tour", () => api.session().active === context());
    await stageStep(last()!.step!);
    delete api.viewports().main;
    api.stageKey("Escape");
    await until("MAIN to be active, the tour closed", () => api.session().active === "main" && !api.session().order.some((id) => id.startsWith("tour:")));
    assert.strictEqual(JSON.stringify(api.session().contexts.main), before, "MAIN is as it was");
    await until("MAIN's viewport as it was", () => near(api.viewports().main, 50, 40, 0.7));
    assert.deepStrictEqual(api.webviewErrors(), []);
  });
});

const MARKDOWN = `checkout runs the sale in three moves.

- check the stock, and refuse an order that cannot be filled
- hold the units, then charge
- give the units back on a timeout

| move | what it touches |
| --- | --- |
| \`has_stock\` | reads the count |
| \`reserve\` | takes units out |

\`\`\`mermaid
flowchart LR
  order --> has_stock
  has_stock --> reserve
  reserve --> charge
\`\`\`
`;

const BROKEN = `has_stock answers one question about the count.

\`\`\`mermaid
flowchart LR
  a -->
\`\`\`
`;

suite("a step's text is Markdown", () => {
  let md = "";
  suiteSetup(async function () {
    this.timeout(90_000);
    await sleep(1100); // told apart by the second they were started in
    md = JSON.parse(tourCmd(["start", "--title", "Markdown in a step", "--author", "the suite", "--json"])).tour;
    // made by the built `vnoc tour add`, as an agent writes it: the core accepts the list, the table and the diagram
    tourCmd(["add", md, "tourdemo/shop.py::checkout", "--body-file", "-"], MARKDOWN);
    tourCmd(["add", md, "tourdemo/shop.py::has_stock", "--body-file", "-"], BROKEN);
    tourCmd(["finish", md, "--no-ids"]);
    await sectionShows("the new tour in the list", (d) => d.list.some((r) => r.id === md), 30_000);
    api.toursClick("key:Escape");
    api.toursClick(`row:${md}`);
    await until("the Markdown tour's context", () => api.session().active === `tour:${md}`);
    await stageShows("the cover", (d) => d.mode === "cover", 10_000, md);
  });

  test("a list, a table and a Mermaid flowchart are drawn, and the code pane has syntax colours", async () => {
    api.stageClick("start");
    const d = await stageShows("the step drawn with its diagram", (x) => x.step === "s1" && x.markdown.diagrams === 1 && x.markdown.drawn === 1, 30_000, md);
    assert.deepStrictEqual(d.markdown, { lists: 1, tables: 1, fences: 0, diagrams: 1, drawn: 1, failed: 0 });
    assert.ok(d.coloured > 10, `the code pane drew ${d.coloured} coloured tokens`);
    assert.strictEqual(d.text, MARKDOWN.trim(), "the stage has the core's text, with its lines");
    assert.deepStrictEqual(api.webviewErrors(), []);
  });

  test("a diagram that does not parse shows its source and one line, and the next step is drawn", async () => {
    api.stageClick("next");
    const d = await stageShows("the broken diagram's note", (x) => x.step === "s2" && x.markdown.diagrams === 1 && x.markdown.failed === 1, 30_000, md);
    assert.deepStrictEqual(d.markdown, { lists: 0, tables: 0, fences: 0, diagrams: 1, drawn: 0, failed: 1 });
    assert.deepStrictEqual(api.webviewErrors(), []);
  });
});

// ── a tour read while it is written (docs/26-tours/05-on-screen.md §3, design 02 frames 6 and 8, 04 §6) ────────────────────────────────────────────────────

const dropTourContext = async () => {
  const open = Object.values(api.session().contexts).find((c) => c.kind === "tour");
  if (open) api.dispatch({ type: "closeContext", id: open.id }, open.id);
  await until("no tour context", () => !Object.values(api.session().contexts).some((c) => c.kind === "tour"));
};
const mapOfTour = (id: string) => api.mapDrawn().filter((m) => m.tour === id).at(-1);
const aMapShows = async (id: string, what: string, ready: (m: NonNullable<ReturnType<typeof mapOfTour>>) => boolean) => {
  await until(what, () => {
    const m = mapOfTour(id);
    return m !== undefined && ready(m);
  });
  return mapOfTour(id)!;
};
/** a short stop of the demo code: its text, and the node it is about */
const SHORT = {
  has_stock: "has_stock answers one question: are there enough units of this SKU in stock? It reads and never changes the count.\n",
  reserve: "reserve takes the units out of the count, so a second buyer cannot take them while this one pays.\n",
  charge: "charge is the one place money is taken; it returns what was paid, and raises nothing itself.\n",
  release: "release puts the units back when the charge did not go through, so the count is the one it was.\n",
};
const node = (name: keyof typeof SHORT) => `tourdemo/shop.py::${name}`;

suite("a tour read while it is written", () => {
  let writer = "";
  let bare = "";

  suiteSetup(async function () {
    this.timeout(60_000);
    await dropTourContext();
    api.dispatch({ type: "activate", id: "main" }, "main");
  });

  test("a tour that starts while the canvas is hidden opens nothing: one notification, once, and its Open Tour brings the tour up on the cover", async function () {
    this.timeout(60_000);
    await closeCanvas();
    await sleep(1100);
    writer = JSON.parse(tourCmd(["start", "--title", "Written as it goes", "--question", "what keeps the stock right", "--author", "the suite", "--plan", node("has_stock"), node("reserve"), node("charge"), node("release"), "--json"])).tour;
    await until("the notification for the new tour", () => api.toursNoticed().includes(writer));
    assert.deepStrictEqual(canvasTabs().length, 0, "the canvas stayed closed");
    assert.ok(!Object.values(api.session().contexts).some((c) => c.kind === "tour"), "no tour context was made");
    assert.strictEqual(api.tourStatus(), null, "no tour is open: no status bar item");
    tourCmd(["add", writer, node("has_stock"), "--body-file", "-"], SHORT.has_stock);
    await sleep(800);
    assert.strictEqual(api.toursNoticed().filter((id) => id === writer).length, 1, "once per tour: a second change says nothing");
    await api.toursNoticeOpen(writer); // the button of the notification
    await until("the tour's context", () => api.session().active === `tour:${writer}`);
    await until("the canvas tab", () => canvasTabs().length > 0);
    const cover = await stageShows("the cover", (d) => d.mode === "cover", 20_000, writer);
    assert.strictEqual(cover.beat, "writing");
  });

  test("the stops of the plan are on the route and the map at once, not written; the bar, the foot, the chip and the status bar item say the agent is writing", async () => {
    const route = await sectionShows("the route of the tour being written", (d) => d.mode === "route" && d.route.filter((r) => r.kind === "stop").length === 4);
    assert.deepStrictEqual(route.route.filter((r) => r.kind === "stop").map((r) => [r.id, r.mark, r.description]), [["s1", "ahead", null], ["s2", "planned", "writing…"], ["s3", "planned", null], ["s4", "planned", null]], "stop 1 is written, the agent is on stop 2");
    assert.deepStrictEqual(route.foot, { text: "writing 2 of 4", stop: true });
    assert.strictEqual(route.more, false, "a plan: no row for more");
    const map = await aMapShows(writer, "the four cards", (m) => m.cards.length === 4);
    assert.deepStrictEqual(map.cards.map((c) => [c.id, c.planned, c.writing]), [["s1", false, false], ["s2", true, true], ["s3", true, false], ["s4", true, false]]);
    await barShows("the agent's words and the ■", (b) => b.agent === "writing 2/4" && b.stop);
    await until("the writing dot, pulsing", () => api.chips().find((c) => c.id === `tour:${writer}`)?.dot === "pulsing");
    await until("the status bar item", () => api.tourStatus() === "▶ Tour · writing 2/4");
    const cover = last()!;
    assert.deepStrictEqual(cover.segments, { total: 4, planned: 3, filled: 0 }, "four segments, three dotted");
  });

  test("an add puts the stop on the route and the map with no reload, and the stage stays on the reader's step", async () => {
    api.stageClick("start");
    const one = await stageShows("stop 1", atStep("s1"), 10_000, writer);
    assert.strictEqual(one.kind, "short");
    assert.strictEqual(one.position, "stop 1 of 4");
    assert.strictEqual(one.nextState, "planned", "the next stop is not written: Next is dim");
    assert.strictEqual(one.nextLabel, "Next");
    await until("the status bar item follows the place", () => api.tourStatus() === "▶ Tour 1/4 · writing 2/4");
    // the agent adds stop 2 while the reader is on stop 1
    tourCmd(["add", writer, node("reserve"), "--body-file", "-"], SHORT.reserve);
    const map = await aMapShows(writer, "stop 2 written on the map", (m) => m.cards.find((c) => c.id === "s2")?.planned === false);
    assert.deepStrictEqual(map.cards.map((c) => [c.id, c.planned, c.writing]), [["s1", false, false], ["s2", false, false], ["s3", true, true], ["s4", true, false]]);
    const route = await sectionShows("stop 2 written on the route", (d) => d.route.find((r) => r.id === "s2")?.mark === "ahead");
    assert.deepStrictEqual(route.foot, { text: "writing 3 of 4", stop: true });
    const now = await stageShows("still on stop 1, Next lit", (d) => d.nextState === "ready", 10_000, writer);
    assert.strictEqual(now.step, "s1", "the stage never jumps to the newest stop");
    assert.strictEqual(now.position, "stop 1 of 4");
    await barShows("the bar moved on", (b) => b.agent === "writing 3/4");
  });

  test("reading ahead onto a stop that is not written shows it being written, and its text fades in where the grey bars were when the agent adds it", async () => {
    api.stageClick("jump:s3");
    const ahead = await stageShows("stop 3, not written", atStep("s3"), 10_000, writer);
    assert.strictEqual(ahead.kind, "planned");
    assert.strictEqual(ahead.sentence, "The agent is writing this stop. You can read the code meanwhile.");
    assert.strictEqual(ahead.text, "", "no text yet");
    assert.ok(ahead.code !== null && ahead.code.text.includes("def charge"), "the code is readable meanwhile");
    assert.strictEqual(ahead.nextState, "planned", "stop 4 is not written either");
    assert.strictEqual(ahead.fade, false);
    // a stop that is not the one being written says where the agent is
    api.stageClick("jump:s4");
    const further = await stageShows("stop 4, not written", atStep("s4"), 10_000, writer);
    assert.strictEqual(further.sentence, "Not written yet: the agent is on stop 3.");
    assert.strictEqual(further.nextLabel, "Finish", "the last stop of the route: nothing after it");
    api.stageClick("jump:s3");
    await stageShows("back on stop 3", atStep("s3"), 10_000, writer);
    // the agent writes it: the same step now has the text, and it faded in
    tourCmd(["add", writer, node("charge"), "--body-file", "-"], SHORT.charge);
    const written = await stageShows("the text where the bars were", (d) => d.step === "s3" && d.kind === "short" && d.text === SHORT.charge.trim(), 10_000, writer);
    assert.strictEqual(written.fade, true, "it fades in");
    assert.strictEqual(written.sentence, null);
    await stageShows("the fade is over", (d) => d.step === "s3" && !d.fade, 10_000, writer);
    assert.deepStrictEqual(api.webviewErrors(), []);
  });

  test("■ on the bar is the core's halt: the bar, the foot and the stage say stopping until the agent answers, and stopped after its next add is refused", async () => {
    api.stageClick("jump:s4");
    await stageShows("stop 4", atStep("s4"), 10_000, writer);
    api.stageClick("menuStop"); // the ■ on the bar
    const stopping = await barShows("stopping", (b) => b.agent === "stopping" && !b.stop);
    assert.strictEqual(stopping.agent, "stopping");
    const d = await stageShows("the stage says it will stop", (x) => x.beat === "stopping", 10_000, writer);
    assert.strictEqual(d.sentence, "Not written yet: the agent stops after the stop it is on.");
    const route = await sectionShows("the foot says stopping", (x) => x.foot?.text.startsWith("stopping") === true);
    assert.strictEqual(route.foot?.stop, false, "no ■ after it was pressed");
    await until("the status bar item", () => api.tourStatus() === "▶ Tour 4/4 · stopping");
    assert.strictEqual(JSON.parse(tourCmd(["show", writer, "--json"])).state, "stopping", "the core's state, not the page's guess");
    // the agent's next command: refused, and that is when the tour is stopped
    let refused: { status?: number; stdout?: string; stderr?: string } | undefined;
    try {
      tourCmd(["add", writer, node("release"), "--body-file", "-"], SHORT.release);
    } catch (e) {
      refused = e as typeof refused;
    }
    assert.ok(refused, "the add was refused: it exited with an error");
    assert.strictEqual(refused.status, 1);
    assert.match(`${refused.stdout}${refused.stderr}`, /^stopped: the reader stopped this tour/m, "the core says the tour was stopped");
    await barShows("stopped", (b) => b.agent === "stopped");
    const stopped = await stageShows("the stage says stopped", (x) => x.beat === "stopped", 10_000, writer);
    assert.strictEqual(stopped.sentence, "Not written: the tour was stopped here. Ask the agent to continue it.");
    assert.strictEqual(stopped.step, "s4", "the reader stays where they were");
    const foot = await sectionShows("the foot says stopped", (x) => x.foot?.text.startsWith("stopped:") === true);
    assert.deepStrictEqual(foot.foot, { text: "stopped: 3 of 4 written", stop: false });
    await until("the status bar item", () => api.tourStatus() === "▶ Tour 4/4 · stopped");
    await until("the dot is gone", () => api.chips().find((c) => c.id === `tour:${writer}`)?.dot === undefined);
    api.stageClick("jump:s1");
    await stageShows("stop 1 again", atStep("s1"), 10_000, writer);
    assert.strictEqual(last()!.nextState, "ready", "the next stop is written");
  });

  test("a tour with no plan: the written count only, one dotted segment and a `◌ …` row, Next dim on the last written step, and it lights when the next stop is added", async function () {
    this.timeout(60_000);
    api.stageClick("closeTour");
    await dropTourContext();
    await sleep(1100);
    bare = JSON.parse(tourCmd(["start", "--title", "No plan", "--author", "the suite", "--json"])).tour;
    tourCmd(["add", bare, node("has_stock"), "--body-file", "-"], SHORT.has_stock);
    await sectionShows("the list has it", (d) => d.list.some((r) => r.id === bare), 30_000);
    api.toursClick(`row:${bare}`);
    await until("the tour's context", () => api.session().active === `tour:${bare}`);
    await toCover(bare);
    const cover = await stageShows("the cover", (d) => d.mode === "cover", 10_000, bare);
    assert.strictEqual(cover.beat, "writing");
    api.stageClick("start");
    const one = await stageShows("stop 1", atStep("s1"), 10_000, bare);
    assert.strictEqual(one.position, "stop 1 · 1 so far", "no `of`");
    assert.deepStrictEqual(one.segments, { total: 2, planned: 1, filled: 1 }, "the stop, and one dotted segment after it");
    assert.strictEqual(one.nextState, "adding");
    assert.strictEqual(one.nextLabel, "Next", "not Finish: the tour is not finished");
    assert.strictEqual(one.adding, "The agent is adding the next stop.");
    assert.strictEqual(one.text, SHORT.has_stock.trim(), "the step's words stay");
    const route = await sectionShows("the route with its `◌ …` row", (d) => d.mode === "route" && d.more);
    assert.deepStrictEqual(route.foot, { text: "writing · 1 stop so far", stop: true });
    assert.ok(route.header.includes("1 · 1 so far"), route.header);
    await barShows("the bar", (b) => b.agent === "writing · 1 so far" && b.stop);
    api.stageClick("next"); // nowhere to go: it does nothing, and the core is not asked to take the reader to the end card
    await sleep(600);
    assert.strictEqual(last()!.step, "s1");
    assert.strictEqual(JSON.parse(tourCmd(["where", bare, "--json"])).at, "s1");
    // the next stop arrives: Next lights, and nothing moves by itself
    tourCmd(["add", bare, node("reserve"), "--body-file", "-"], SHORT.reserve);
    const lit = await stageShows("Next lit", (d) => d.nextState === "ready", 10_000, bare);
    assert.strictEqual(lit.step, "s1");
    assert.strictEqual(lit.position, "stop 1 · 2 so far");
    assert.strictEqual(lit.adding, null);
    // the tour is finished: the written count is the size, and the words about the agent go
    tourCmd(["finish", bare, "--no-ids"]);
    const done = await stageShows("finished", (d) => d.beat === "finished", 10_000, bare);
    assert.strictEqual(done.position, "stop 1 of 2");
    assert.strictEqual(done.segments.total, 2);
    await barShows("nothing on the bar", (b) => b.agent === null && !b.stop);
    const after = await sectionShows("no foot", (d) => d.foot === null && !d.more);
    assert.strictEqual(after.foot, null);
    await until("the status bar item says only the place", () => api.tourStatus() === "▶ Tour 1/2");
  });

  test("a tour with no news for five minutes says so, and the agent writing again takes the words back without a word", async function () {
    this.timeout(60_000);
    api.stageClick("closeTour");
    await dropTourContext();
    await sleep(1100);
    const quiet = JSON.parse(tourCmd(["start", "--title", "Gone quiet", "--author", "the suite", "--plan", node("has_stock"), node("release"), "--json"])).tour;
    await sectionShows("the list has it", (d) => d.list.some((r) => r.id === quiet), 30_000);
    api.toursClick(`row:${quiet}`);
    await until("the tour's context", () => api.session().active === `tour:${quiet}`);
    await stageShows("the cover", (d) => d.mode === "cover", 10_000, quiet);
    // the file's time says nothing was added for six minutes
    const file = path.join(workspace(), ".vnoc", "local", "tours", `${quiet}.jsonl`);
    const old = new Date(Date.now() - 6 * 60_000);
    fs.utimesSync(file, old, old);
    await barShows("no news", (b) => b.agent === "no news" && !b.stop, 40_000);
    const d = await stageShows("the stage says it", (x) => x.beat === "quiet", 10_000, quiet);
    assert.strictEqual(d.mode, "cover");
    await until("the chip's dot stands still", () => api.chips().find((c) => c.id === `tour:${quiet}`)?.dot === "still");
    await until("the status bar item", () => api.tourStatus() === "▶ Tour · no news");
    const route = await sectionShows("the foot", (x) => x.foot?.text.startsWith("no news from the agent") === true);
    assert.strictEqual(route.foot?.stop, false);
    api.stageClick("start");
    const planned = await stageShows("stop 1, not written", atStep("s1"), 10_000, quiet);
    assert.strictEqual(planned.sentence, "Not written. The agent may have stopped: look at its terminal, or ask it to continue the tour.");
    // the agent writes again: no news is gone, with nothing said
    tourCmd(["add", quiet, node("has_stock"), "--body-file", "-"], SHORT.has_stock);
    await barShows("writing again", (b) => b.agent?.startsWith("writing") === true && b.stop, 40_000);
    await until("the dot pulses again", () => api.chips().find((c) => c.id === `tour:${quiet}`)?.dot === "pulsing");
    await stageShows("the text where the bars were", (x) => x.step === "s1" && x.kind === "short", 10_000, quiet);
  });

  test("a second tour that appears while one is open leaves the open one, and the list has it", async function () {
    this.timeout(60_000);
    const open = api.session().active;
    assert.ok(open.startsWith("tour:"), open);
    const before = api.toursNoticed().length;
    await sleep(1100);
    const next = JSON.parse(tourCmd(["start", "--title", "A second one", "--author", "the suite", "--plan", node("release"), "--json"])).tour;
    await sectionShows("the list has the new tour", (d) => d.list.some((r) => r.id === next), 30_000);
    assert.strictEqual(api.session().active, open, "the open tour stays");
    assert.strictEqual(api.toursNoticed().length, before, "the canvas is showing a tour: no notification");
  });

  test("W11: with the canvas visible and no tour open, a tour that appears and is being written becomes the tour context, at its cover", async function () {
    this.timeout(60_000);
    api.stageClick("closeTour");
    await dropTourContext();
    await vscode.commands.executeCommand("vnoc.openCanvas");
    await until("the canvas tab", () => canvasTabs().length > 0);
    await until("MAIN active", () => api.session().active === "main");
    const noticed = api.toursNoticed().length;
    await sleep(1100);
    const w11 = JSON.parse(tourCmd(["start", "--title", "Opened for you", "--author", "the suite", "--plan", node("has_stock"), node("release"), "--json"])).tour;
    await until("the tour's context, active", () => api.session().active === `tour:${w11}`, 30_000);
    const cover = await stageShows("the cover", (d) => d.mode === "cover", 20_000, w11);
    assert.strictEqual(cover.beat, "writing");
    assert.strictEqual(api.toursNoticed().length, noticed, "it was opened, not announced");
    // a second tour while this one is open: not replaced, and nothing opens
    await sleep(1100);
    const other = JSON.parse(tourCmd(["start", "--title", "Not opened", "--author", "the suite", "--plan", node("release"), "--json"])).tour;
    await sectionShows("the list has it", (d) => d.list.some((r) => r.id === other), 30_000);
    assert.strictEqual(api.session().active, `tour:${w11}`, "the open tour is not replaced");
  });

  test("the canvas hidden and no tour open: a tour that appears unfolds the section, is announced, and nothing opens", async function () {
    this.timeout(60_000);
    api.stageClick("closeTour");
    await dropTourContext();
    api.toursClick("fold");
    await sectionShows("folded", (d) => d.folded);
    await closeCanvas();
    const noticed = api.toursNoticed().length;
    await sleep(1100);
    const third = JSON.parse(tourCmd(["start", "--title", "A third one", "--author", "the suite", "--json"])).tour;
    await sectionShows("unfolded by the new tour", (d) => !d.folded && d.mode === "list" && d.list.some((r) => r.id === third), 30_000);
    await until("the notification", () => api.toursNoticed().length === noticed + 1);
    assert.ok(!Object.values(api.session().contexts).some((c) => c.kind === "tour"), "nothing was opened");
  });
});

// ── a tour that no longer matches the code (docs/26-tours/05-on-screen.md §3, design 05 §4) ─────────────────────────────────────────────────────────────────────
//
// A pinned tour shows a changed or gone step as it was written (`tourCode`), with the label and the switch; a tour that is not pinned shows the code as it is, the lit lines dashed. Every test
// edits `tourdemo/shop.py` on disk and puts it back with `git checkout`: nothing another suite reads stays changed.

suite("a tour that no longer matches the code", () => {
  let pinned = "";
  let other = "";
  const gitIn = (...args: string[]) => execFileSync("git", ["-c", "user.name=V-NOC test", "-c", "user.email=test@vnoc.invalid", "-c", "commit.gpgsign=false", ...args], { cwd: workspace(), stdio: "ignore" });
  const shop = () => path.join(workspace(), "tourdemo", "shop.py");
  /** the file, with `from` replaced by `to` (saved to disk: the core reads the file) */
  const edit = (from: string, to: string) => {
    const text = fs.readFileSync(shop(), "utf8");
    assert.ok(text.includes(from), `shop.py has ${from}`);
    fs.writeFileSync(shop(), text.replace(from, to));
  };
  const restore = () => gitIn("checkout", "--", "tourdemo/shop.py");
  const openTour = async (id: string) => {
    api.toursClick("fold"); // a click on a row needs the list: fold, then unfold, shows it whatever the section showed
    api.toursClick("unfold");
    await sectionShows("the list has the tour", (d) => d.mode === "list" && d.list.some((r) => r.id === id), 30_000);
    api.toursClick(`row:${id}`);
    await until("the tour's context", () => api.session().active === `tour:${id}`);
  };
  const routeRow = (id: string) => section()?.route.find((r) => r.kind === "stop" && r.id === id);
  const card = (tourId: string, id: string) => mapOfTour(tourId)?.cards.find((c) => c.id === id);

  suiteSetup(async function () {
    this.timeout(120_000);
    api.stageClick("closeTour");
    await dropTourContext();
    // a second file, for a tour that does not name shop.py
    fs.writeFileSync(path.join(workspace(), "tourdemo", "util.py"), "def double(n):\n    \"\"\"Twice n.\"\"\"\n    return n * 2\n");
    for (let waited = 0; ; waited += 500) {
      const dir = (await api.structure()).root.children.find((c) => c.id === "tourdemo");
      if (dir?.children.some((c) => c.id === "tourdemo/util.py" && c.children.length >= 1)) break;
      assert.ok(waited < 30_000, "the structure never held tourdemo/util.py");
      await sleep(500);
    }
    // the code is committed, so a finished tour can be pinned
    gitIn("add", "tourdemo");
    gitIn("commit", "-q", "-m", "the tour demo");
    const started = JSON.parse(tourCmd(["start", "--title", "Pinned shop", "--author", "the suite", "--json"])).tour;
    pinned = started;
    tourCmd(["add", pinned, "tourdemo/shop.py::checkout", "--body-file", "-"], CHECKOUT);
    tourCmd(["add", pinned, node("has_stock"), "--body-file", "-", "--from", "s1", "--line", "6"], SHORT.has_stock);
    tourCmd(["add", pinned, node("release"), "--body-file", "-"], SHORT.release);
    tourCmd(["finish", pinned, "--no-ids"]);
    tourCmd(["pin", pinned]);
    assert.ok(JSON.parse(tourCmd(["show", pinned, "--json"])).pinned, "the tour is pinned");
    other = JSON.parse(tourCmd(["start", "--title", "Util only", "--author", "the suite", "--json"])).tour;
    tourCmd(["add", other, "tourdemo/util.py::double", "--body-file", "-"], "double returns twice its argument, and does nothing else with it: read it as one step with nothing hidden behind it.\n");
    tourCmd(["finish", other, "--no-ids"]);
    tourCmd(["check", other]); // records the check: the next ask of it reads no source
  });

  teardown(() => {
    restore();
  });

  test("a pinned tour whose function was edited shows the step as it was written, with its label; Now shows the code as it is, dashed; the row and the card say changed", async function () {
    this.timeout(60_000);
    edit("    reserve(stock, order)\n", "    reserve(stock, order)  # held\n");
    await openTour(pinned);
    await toCover(pinned);
    api.stageClick("jump:s1.b2");
    const written = await stageShows("as written", (d) => d.step === "s1.b2" && d.asWritten !== null, 20_000, pinned);
    assert.strictEqual(written.asWritten!.commit.length, 7, "the label names the commit");
    assert.deepStrictEqual(written.lit, [9, 10, 11, 12, 13, 14], "the block's lines at the commit");
    assert.ok(written.code!.text.includes("    reserve(stock, order)\n"), written.code!.text);
    assert.ok(!written.code!.text.includes("# held"), "the old code, not the file's");
    assert.strictEqual(written.switch, "written", "As Written is where the step opens");
    assert.deepStrictEqual(written.asWritten!.lit, written.lit);
    // the stop is changed on the route and on the map
    await sectionShows("the route says changed", (d) => routeRow("s1")?.description === "changed");
    assert.strictEqual(routeRow("s1")?.struck, false);
    await aMapShows(pinned, "the card says changed", (m) => m.cards.find((c) => c.id === "s1")?.description === "changed");
    // Now: the file as it is, no label, the lit lines (dashed on the page)
    api.stageClick("choose:now");
    const now = await stageShows("now", (d) => d.step === "s1.b2" && d.switch === "now" && d.asWritten === null, 20_000, pinned);
    assert.ok(now.code!.text.includes("# held"), "the code as it is now");
    assert.deepStrictEqual(now.lit, [9, 10, 11, 12, 13, 14]);
    // leave the step and come back: As Written again, the choice is not kept
    api.stageClick("jump:s1.b1");
    const fresh = await stageShows("a step that did not change", (d) => d.step === "s1.b1", 20_000, pinned);
    assert.strictEqual(fresh.asWritten, null, "a fresh step of the same stop: as now");
    assert.strictEqual(fresh.switch, null);
    assert.deepStrictEqual(fresh.lit, [6, 7]);
    api.stageClick("jump:s1.b2");
    await stageShows("as written again", (d) => d.step === "s1.b2" && d.asWritten !== null && d.switch === "written", 20_000, pinned);
    // a detail of the changed block that did not change is fresh: not marked
    api.stageClick("jump:s1.b3");
    const detail = await stageShows("the detail", (d) => d.step === "s1.b3", 20_000, pinned);
    assert.strictEqual(detail.asWritten, null);
    assert.deepStrictEqual(detail.lit, [11, 12, 13, 14]);
    // a second tour that does not name shop.py: nothing of it was read for its check
    api.stageClick("closeTour");
    await dropTourContext();
    await openTour(other);
    const cover = await stageShows("the second tour's cover", (d) => d.mode === "cover", 20_000, other);
    assert.ok(cover.checkFrom === "memory" || cover.checkFrom === "record", `no source file was read for it: ${cover.checkFrom}`);
  });

  test("the same edit in a tour that is not pinned: the code as it is, no label, no switch", async function () {
    this.timeout(60_000);
    api.stageClick("closeTour");
    await dropTourContext();
    edit("    reserve(stock, order)\n", "    reserve(stock, order)  # held\n");
    await openTour(tour);
    await toCover(tour);
    api.stageClick("jump:s1.b2");
    const d = await stageShows("the code as it is", (x) => x.step === "s1.b2" && x.code !== null, 20_000, tour);
    assert.strictEqual(d.asWritten, null);
    assert.strictEqual(d.switch, null);
    assert.ok(d.code!.text.includes("# held"), "the file as it is");
    assert.deepStrictEqual(d.lit, [9, 10, 11, 12, 13, 14]);
    await sectionShows("the route says changed", (x) => routeRow("s1")?.description === "changed");
  });

  test("a function deleted: gone and struck through; pinned, the step is shown as written with no Now; not pinned, there is no code pane", async function () {
    this.timeout(60_000);
    api.stageClick("closeTour");
    await dropTourContext();
    edit("def has_stock(stock, order):\n    return stock.get(order[\"sku\"], 0) >= order[\"qty\"]\n\n\n", "");
    await openTour(pinned);
    await toCover(pinned);
    api.stageClick("jump:s2");
    const written = await stageShows("the gone stop as written", (d) => d.step === "s2" && d.asWritten !== null, 20_000, pinned);
    assert.strictEqual(written.switch, null, "a gone stop has no Now");
    assert.ok(written.code!.text.includes("def has_stock"), "its code at the commit");
    await sectionShows("the route says gone, struck", (d) => routeRow("s2")?.description === "gone" && routeRow("s2")?.struck === true);
    await aMapShows(pinned, "the card says gone, struck", (m) => m.cards.find((c) => c.id === "s2")?.description === "gone" && m.cards.find((c) => c.id === "s2")?.struck === true);
    assert.strictEqual(routeRow("s3")?.description, null, "release moved up: it is fresh");
    // the tour that is not pinned: the stop has no code pane
    api.stageClick("closeTour");
    await dropTourContext();
    await openTour(tour);
    await toCover(tour);
    api.stageClick("jump:s2");
    const bare = await stageShows("the gone stop, not pinned", (d) => d.step === "s2", 20_000, tour);
    assert.strictEqual(bare.asWritten, null);
    assert.strictEqual(bare.switch, null);
    assert.strictEqual(bare.code, null, "no code pane");
    await sectionShows("the route says gone, struck", (d) => routeRow("s2")?.description === "gone" && routeRow("s2")?.struck === true);
  });

  test("most of a pinned tour is about older code: the cover says so, and the route's foot says how many stops changed", async function () {
    this.timeout(60_000);
    api.stageClick("closeTour");
    await dropTourContext();
    edit("    reserve(stock, order)\n", "    reserve(stock, order)  # held\n");
    edit("    stock[order[\"sku\"]] += order[\"qty\"]\n", "    stock[order[\"sku\"]] += 2 * order[\"qty\"]\n");
    edit("def has_stock(stock, order):\n    return stock.get(order[\"sku\"], 0) >= order[\"qty\"]\n\n\n", "");
    await openTour(pinned);
    await toCover(pinned);
    const route = await sectionShows("the foot says how many changed", (d) => d.foot?.text === "3 of 3 stops changed since this was written", 20_000);
    assert.deepStrictEqual(route.route.filter((r) => r.kind === "stop").map((r) => [r.id, r.description, r.struck]), [["s1", "changed", false], ["s2", "gone", true], ["s3", "changed", false]]);
    assert.strictEqual(route.foot?.stop, false);
    api.stageClick("next");
    await stageShows("the first step", (d) => d.mode === "step", 20_000, pinned);
  });

  test("a core that starts again: the list says changed from the record, and a tour opened first is answered from the record", async function () {
    this.timeout(60_000);
    api.stageClick("closeTour");
    await dropTourContext();
    edit("    reserve(stock, order)\n", "    reserve(stock, order)  # held\n");
    // the check is made and recorded while the tour is open
    await openTour(pinned);
    await stageShows("the tour, checked", (d) => d.checkFrom !== null, 20_000, pinned);
    await sectionShows("the route says changed", (d) => routeRow("s1")?.description === "changed");
    api.stageClick("closeTour");
    await dropTourContext();
    tourCmd(["check", pinned]); // the record on disk is of the file as it is now, whatever the window's core had made of it by now
    await api.restartCore(); // the new core remembers nothing: the record on disk is all it has
    // opened with no list asked in between (a list that is asked for checks the rows it holds, and the core then remembers the answer: `memory`)
    const drawn = api.stageDrawn().length;
    api.dispatch({ type: "tourOpen", id: pinned }, "main");
    await until("the tour's context", () => api.session().active === `tour:${pinned}`);
    const after = () => api.stageDrawn().slice(drawn).filter((d) => d.tour === pinned && d.checkFrom !== null);
    await until("the reopened tour drawn", () => after().length > 0, 20_000);
    // the page and the host both ask for the tour: the first answer is from the record, a second asked while it was being answered may be the core's memory of it. Never a check
    await until("the record's answer", () => after().some((d) => d.checkFrom === "record"), 20_000);
    assert.ok(after().every((d) => d.checkFrom !== "check"), `no source file was read: ${JSON.stringify(after().map((d) => d.checkFrom))}`);
    api.stageClick("closeTour");
    await dropTourContext();
    await api.restartCore();
    const rows = await api.tours.list(); // the first list asks plain, then with the ids it holds: the words say changed
    assert.ok(rows.find((r) => r.id === pinned)?.words.includes("changed"), `the row says changed: ${JSON.stringify(rows.find((r) => r.id === pinned))}`);
    assert.ok(!rows.find((r) => r.id === other)?.words.includes("changed"), "a tour whose code did not change says nothing");
  });
});
