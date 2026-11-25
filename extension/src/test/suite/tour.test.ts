import * as assert from "assert";
import { execFileSync } from "child_process";
import * as fs from "fs";
import * as path from "path";
import * as vscode from "vscode";

import type { Notes, StructureResult } from "@vnoc/shared/messages";
import type { Session } from "@vnoc/shared/session.ts";
import { canvasTabs, closeCanvas, guarded, sleep, soft, until, waitFor } from "./util";

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
  tourChips(): Extract<Notes, { type: "tourChips" }>[];
  showTours(key: string, context: string): Promise<void>;
  request<T>(method: string, params?: object): Promise<T>;
  pinDialog(): Extract<Notes, { type: "pinDialog" }>[];
  pinDialogClick(key: string, text?: string): void;
  tourMessages(): { level: string; text: string; buttons: string[] }[];
  answerTourMessages(answers: (string | undefined)[] | null): void;
  answerTourPicks(labels: string[] | null): void;
  dirtyFiles(): string[];
}
let api: Api;

// the hooks of this file wait for the core: when it has stopped answering they stand aside (`soft`), and each test fails at once
const suiteSetup = (fn: (this: Mocha.Context) => unknown) => globalThis.suiteSetup(soft(fn));
const setup = (fn: (this: Mocha.Context) => unknown) => globalThis.setup(soft(fn));

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
/** the pinned tour of "a tour that no longer matches the code" (checkout, has_stock, release), for the suites after it */
let pinnedId = "";
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
  // a tour that was just opened is at its cover already: a click on "again" would send a move to the cover that can land after the test's own next move (two requests overtake each other)
  if (JSON.parse(tourCmd(["where", of, "--json"])).at === "cover" && last()?.tour === of && last()?.mode === "cover") return;
  api.stageClick("again");
  // the core has the cover before the next click is sent: two moves sent together are two requests that may overtake each other (and when the stage already shows the cover
  // there is nothing new to wait for on the page)
  await until("the core's place to be the cover", () => JSON.parse(tourCmd(["where", of, "--json"])).at === "cover");
  await stageShows("the cover", (d) => d.mode === "cover", 10_000, of);
};

suiteSetup(async () => {
  api = guarded(await vscode.extensions.getExtension("vnoc.vnoc")!.activate());
});

// ── every test starts clean ────────────────────────────────────────────────────────────────────────────────────────────────────────────────────

const gitIn = (...args: string[]) => execFileSync("git", ["-c", "user.name=V-NOC test", "-c", "user.email=test@vnoc.invalid", "-c", "commit.gpgsign=false", ...args], { cwd: workspace(), encoding: "utf8" });
const tomlFile = () => path.join(workspace(), ".vnoc", "vnoc.toml");
const stateDir = () => path.join(workspace(), ".vnoc", "local", "tour-state");
/** What the workspace was when the first test of a suite began (after its `suiteSetup`, which may commit what the suite is about): the head, `shop.py` and `vnoc.toml` (null: there was none). */
const starts = new Map<Mocha.Suite, { head: string; shop: string | null; toml: string | null }>();
/** The suites before this one in a whole run leave `[ids] default = "none"` in `vnoc.toml` (their first attach writes it), and the tour tests are made for a project with none: the first test removes it. */
let tomlCleared = false;
const readOrNull = (file: string) => (fs.existsSync(file) ? fs.readFileSync(file, "utf8") : null);

/**
 * Before each test of this file (and only this file's: a root hook is every suite's): no tour context is open, MAIN is active, the workspace is as it was when the suite began (`git reset --hard` to
 * the head then, `shop.py` and `vnoc.toml` as they were), and no tour has a place or a "seen" mark (the state files are gone). A test that needs more (a tour open, a place) sets it up itself.
 */
setup(async function (this: Mocha.Context) {
  const test = this.currentTest;
  if (test?.file !== __filename) return;
  this.timeout(30_000);
  const shop = path.join(workspace(), "tourdemo", "shop.py");
  if (!tomlCleared) {
    tomlCleared = true;
    fs.rmSync(tomlFile(), { force: true });
  }
  if (!starts.has(test.parent!)) starts.set(test.parent!, { head: gitIn("rev-parse", "HEAD").trim(), shop: readOrNull(shop), toml: readOrNull(tomlFile()) });
  const start = starts.get(test.parent!)!;
  api.answerTourMessages(null);
  api.answerTourPicks(null);
  api.pinDialogClick("cancel");
  for (const c of Object.values(api.session().contexts).filter((c) => c.kind === "tour")) api.dispatch({ type: "closeContext", id: c.id }, c.id);
  await until("no tour context", () => !Object.values(api.session().contexts).some((c) => c.kind === "tour"));
  if (api.session().active !== "main") api.dispatch({ type: "activate", id: "main" }, "main");
  gitIn("reset", "--hard", start.head);
  if (start.shop !== null && readOrNull(shop) !== start.shop) {
    fs.mkdirSync(path.dirname(shop), { recursive: true });
    fs.writeFileSync(shop, start.shop);
  }
  if (start.toml === null) fs.rmSync(tomlFile(), { force: true });
  else if (readOrNull(tomlFile()) !== start.toml) fs.writeFileSync(tomlFile(), start.toml);
  for (const f of fs.existsSync(stateDir()) ? fs.readdirSync(stateDir()) : []) fs.rmSync(path.join(stateDir(), f), { force: true });
});

/**
 * Opens a tour by **Open Tour…**'s command with the tour's id (it asks nothing: no QuickPick, no keyboard focus), and waits for its context and the canvas. The core must know the tour (an
 * agent's command just wrote it): that is waited for first.
 */
const openByCommand = async (id = tour) => {
  await waitFor(`the core to list tour ${id}`, async () => (await api.tours.list()).some((r) => r.id === id), 30_000);
  await vscode.commands.executeCommand("vnoc.openTour", id);
  await until(`the context of tour ${id}`, () => api.session().active === `tour:${id}`);
  await until("the canvas tab", () => canvasTabs().length > 0);
};
/** {@link openByCommand}, then the stage's own drawing of the cover: one drawn after this call, not a stale one of an earlier test. */
const openAtCover = async (id = tour) => {
  const drawn = api.stageDrawn().length;
  await openByCommand(id);
  await until("the core's place to be the cover", () => JSON.parse(tourCmd(["where", id, "--json"])).at === "cover");
  await until("the cover drawn anew", () => api.stageDrawn().slice(drawn).some((d) => d.tour === id && d.mode === "cover"), 30_000);
};

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

  test("Open Tour… lists the tour, and its command with the tour's id opens the tour's context on the cover", async () => {
    const rows = await api.tours.list();
    assert.strictEqual(rows[0]?.id, tour, `the core lists the tour first (newest first): ${JSON.stringify(rows.map((r) => [r.id, r.words]))}`);
    assert.ok(rows[0].words.startsWith("new · 4 stops"), `a row says its words: ${rows[0].words}`);
    assert.strictEqual(api.session().active, "main", "no tour is open when the test starts");
    await vscode.commands.executeCommand("vnoc.openTour", tour); // the command with the id: no pick
    await until("the tour's context", () => api.session().active === `tour:${tour}`);
    await until("the canvas tab", () => canvasTabs().length > 0);
    const cover = await stageShows("the cover", (d) => d.mode === "cover");
    assert.strictEqual(cover.title, "How the shop checks out");
    assert.strictEqual(cover.next, "Start");
    assert.strictEqual(cover.code, null, "the cover is one column");
    assert.strictEqual(api.session().contexts[`tour:${tour}`].tour, tour);
    assert.strictEqual(api.session().contexts[`tour:${tour}`].selected, "tourdemo/shop.py::checkout", "the first stop is the selection at the cover");
  });

  suite("on the open tour", () => {
  setup(async function () {
    this.timeout(60_000);
    await openAtCover();
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

  /** the tour open from its row, with the route in the section (a click on a row is what the reader does) */
  const openFromItsRow = async (id = tour) => {
    await sectionShows("the list", (d) => d.mode === "list" && d.list.some((r) => r.id === id), 30_000);
    api.toursClick(`row:${id}`);
    await until("the tour's context", () => api.session().active === `tour:${id}`);
    await toCover(id);
    await sectionShows("the route", (d) => d.mode === "route");
  };

  test("‹ Tours shows the list and the tour stays open; its row has the bar and a click on it brings the route back", async () => {
    await openFromItsRow();
    for (let i = 0; i < 6; i++) api.stageClick("next"); // s1, s1.b1, s1.b2, s1.b3, s2, s4
    await stageStep("s4");
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
    await openFromItsRow();
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
    await openFromItsRow();
    api.toursClick("stop:s2"); // reached from s1 at line 6, which block s1.b1 (6-7) holds
    await stageStep("s2");
    api.stageClick("calledFrom");
    const step = await stageStep("s1.b1");
    assert.strictEqual(step.lit.join(","), "6,7");
  });

  test("the keys: ↓ ↑ move the focus, Enter opens or jumps, Escape in the route goes back to the list", async () => {
    await openFromItsRow();
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
  setup(async function () {
    this.timeout(60_000);
    await openAtCover();
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
  setup(async function () {
    this.timeout(60_000);
    await openAtCover();
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
  });

  setup(async function () {
    this.timeout(60_000);
    await openAtCover(md);
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
    api.stageClick("start");
    await stageShows("stop 1", atStep("s1"), 30_000, md);
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

  /** A tour being written, the way the tests after the first need it: the plan of four stops, `n` of them written (in the plan's order), and the tour open on its cover. */
  const writing = async (n: 1 | 2 | 3): Promise<string> => {
    await sleep(1100); // told apart by the second they were started in
    const id = JSON.parse(tourCmd(["start", "--title", "Written as it goes", "--question", "what keeps the stock right", "--author", "the suite", "--plan", node("has_stock"), node("reserve"), node("charge"), node("release"), "--json"])).tour;
    for (const name of (["has_stock", "reserve", "charge"] as const).slice(0, n)) tourCmd(["add", id, node(name), "--body-file", "-"], SHORT[name]);
    await openAtCover(id);
    return id;
  };

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

  test("the stops of the plan are on the route and the map at once, not written; the bar, the foot, the chip and the status bar item say the agent is writing", async function () {
    this.timeout(60_000);
    writer = await writing(1);
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

  test("an add puts the stop on the route and the map with no reload, and the stage stays on the reader's step", async function () {
    this.timeout(60_000);
    writer = await writing(1);
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

  test("reading ahead onto a stop that is not written shows it being written, and its text fades in where the grey bars were when the agent adds it", async function () {
    this.timeout(60_000);
    writer = await writing(2);
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

  test("■ on the bar is the core's halt: the bar, the foot and the stage say stopping until the agent answers, and stopped after its next add is refused", async function () {
    this.timeout(60_000);
    writer = await writing(3);
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

  test("a second tour that appears while one is open leaves the open one, the list has it, and one notification says so", async function () {
    this.timeout(60_000);
    writer = await writing(1); // a tour is open
    const open = api.session().active;
    assert.ok(open.startsWith("tour:"), open);
    const before = api.toursNoticed().length;
    await sleep(1100);
    const next = JSON.parse(tourCmd(["start", "--title", "A second one", "--author", "the suite", "--plan", node("release"), "--json"])).tour;
    await sectionShows("the list has the new tour", (d) => d.list.some((r) => r.id === next), 30_000);
    assert.strictEqual(api.session().active, open, "the open tour stays");
    await until("the notification for the second tour", () => api.toursNoticed().includes(next));
    assert.strictEqual(api.toursNoticed().length, before + 1, "one notification, though the canvas is showing a tour");
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
    await until("its one notification", () => api.toursNoticed().includes(other));
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
    pinnedId = started;
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

// ── the tours of the selected node, `▶ N`, `+N` (docs/26-tours/design/07-tour-list.md §4, §6, §8; 04 §3) ────────────────────────────────────────────────────
//
// The list's second view follows the active context's selection and says, for each tour, how it is on the node (the core's words). A node on tours shows `▶ N` on its card and its row.
// A tour's card shows the node's calls that are not on the tour. Nothing here edits a source file: the tours are made with the built commands, over `tourdemo/shop.py` as it is
// committed and `tourdemo/lone.py`, a file of its own that this suite adds and takes away.

suite("the tours of the selected node", () => {
  const LONE = "def nobody():\n    return 0\n\n\ndef solo():\n    return 1\n";
  const lone = (name: string) => `tourdemo/lone.py::${name}`;
  const ONE = "It does one small thing, and the code around it relies on that: read it as a single step, with nothing hidden behind a second paragraph of explanation.\n";
  const sidebarChips = () => api.tourChips().filter((c) => c.surface === "sidebar").at(-1)?.chips ?? [];
  const canvasChips = () => api.tourChips().filter((c) => c.surface === "canvas").at(-1)?.chips ?? [];
  const counts = async (): Promise<Record<string, number>> => (await api.request<{ counts: Record<string, number> }>("vnoc/tourCounts")).counts;
  const coreRows = async (on: string): Promise<{ id: string; state_word: string; words: string; on: { how: string; words: string; first: string } }[]> => (await api.request<{ tours: any[] }>("vnoc/tours", { on })).tours;
  /** MAIN is the active context with `key` selected, the section is showing the second view */
  const selectOn = async (key: string | null) => {
    api.dispatch({ type: "activate", id: "main" }, "main");
    api.dispatch({ type: "select", id: key }, "main");
    api.dispatch({ type: "tourView", view: "onSelection" });
    api.toursClick("unfold");
    await until("MAIN selects it", () => api.session().active === "main" && api.session().contexts.main.selected === key);
  };
  let solo1 = "";

  suiteSetup(async function () {
    this.timeout(120_000);
    api.stageClick("closeTour");
    await dropTourContext();
    fs.writeFileSync(path.join(workspace(), "tourdemo", "lone.py"), LONE);
    for (let waited = 0; ; waited += 500) {
      const dir = (await api.structure()).root.children.find((c) => c.id === "tourdemo");
      if (dir?.children.some((c) => c.id === "tourdemo/lone.py" && c.children.length >= 2)) break;
      assert.ok(waited < 30_000, "the structure never held tourdemo/lone.py");
      await sleep(500);
    }
  });

  suiteTeardown(async () => {
    api.dispatch({ type: "tourView", view: "all" });
    fs.rmSync(path.join(workspace(), "tourdemo", "lone.py"), { force: true });
  });

  test("On the selected node lists the tours through a function with the core's words; a click opens at that stop, or at the cover for a tour that starts there", async function () {
    this.timeout(90_000);
    await selectOn(node("has_stock"));
    const rows = await coreRows(node("has_stock"));
    const all = (await api.request<{ tours: unknown[] }>("vnoc/tours")).tours.length;
    assert.ok(rows.length >= 2 && rows.length < all, `some of the tours go through has_stock, not all: ${rows.length} of ${all}`);
    const drawn = await sectionShows("the second view", (d) => d.view === "on" && d.list.length === rows.length && d.buttons[1] === `On has_stock ${rows.length}`, 30_000);
    assert.deepStrictEqual(drawn.buttons, [`All ${all}`, `On has_stock ${rows.length}`]);
    assert.deepStrictEqual(drawn.list.map((r) => [r.id, r.line2]), rows.map((r) => [r.id, r.on.words]), "line 2 is the core's words");
    assert.strictEqual(drawn.list.find((r) => r.id === pinnedId)?.line2, "stop 2 of 3");
    // the row keeps its state at the right of line 1: the core's `state_word`, the word its own `words` starts with (after `shared ·`)
    assert.deepStrictEqual(drawn.list.map((r) => [r.id, r.stateWord]), rows.map((r) => [r.id, r.state_word]), "the state word is the core's");
    assert.ok(rows.every((r) => r.state_word !== "" && r.words.replace(/^shared · /, "").startsWith(r.state_word)), JSON.stringify(rows.map((r) => [r.state_word, r.words])));
    assert.strictEqual(drawn.empty, null);
    // a click opens at the stop the core named
    api.toursClick(`row:${pinnedId}`);
    await until("the tour's context", () => api.session().active === `tour:${pinnedId}`);
    const at = await stageShows("the stage at has_stock", (d) => d.stop === "s2" && d.mode === "step", 20_000, pinnedId);
    assert.strictEqual(at.stop, "s2");
    api.stageClick("closeTour");
    await dropTourContext();
    // checkout: the first stop of two tours: "starts here", and a click goes to the cover
    await selectOn(node("has_stock").replace("has_stock", "checkout"));
    const starts = await sectionShows("starts here", (d) => d.view === "on" && d.list.some((r) => r.id === tour && r.line2 === "starts here"), 30_000);
    assert.ok(starts.list.every((r) => r.line2 === "starts here" || /^stops? /.test(r.line2)), JSON.stringify(starts.list.map((r) => r.line2)));
    api.toursClick(`row:${tour}`);
    await until("the tour's context", () => api.session().active === `tour:${tour}`);
    await stageShows("the cover", (d) => d.mode === "cover", 20_000, tour);
    api.stageClick("closeTour");
    await dropTourContext();
  });

  test("a file lists the tours with stops inside it, and says how many", async function () {
    this.timeout(60_000);
    await selectOn("tourdemo/shop.py");
    const rows = await coreRows("tourdemo/shop.py");
    const drawn = await sectionShows("the file's tours", (d) => d.view === "on" && d.list.length === rows.length && rows.length > 0, 30_000);
    assert.deepStrictEqual(drawn.list.map((r) => r.line2), rows.map((r) => r.on.words));
    assert.strictEqual(drawn.list.find((r) => r.id === pinnedId)?.line2, "3 stops inside", "three stops of the pinned tour are in shop.py, none is the file");
    assert.ok(drawn.buttons[1].startsWith("On shop.py "), drawn.buttons[1]);
  });

  test("a node no tour goes through says so, with Show All; with nothing selected On selection is disabled and the list is All; ← → switch", async function () {
    this.timeout(60_000);
    await selectOn(lone("nobody"));
    const all = (await api.request<{ tours: unknown[] }>("vnoc/tours")).tours.length;
    const empty = await sectionShows("the empty state", (d) => d.view === "on" && d.empty !== null && d.buttons[1] === "On nobody 0", 30_000);
    assert.strictEqual(empty.empty, "No tour goes through nobody.");
    assert.deepStrictEqual(empty.list, []);
    // Show All goes back
    api.toursClick("showAll");
    const back = await sectionShows("All", (d) => d.view === "all" && d.list.length === all, 20_000);
    assert.strictEqual(back.empty, null);
    assert.strictEqual(api.session().tourList.view, "all");
    // the keys on the buttons
    api.toursClick("viewkey:ArrowRight");
    await sectionShows("→ is the second view", (d) => d.view === "on");
    api.toursClick("viewkey:ArrowLeft");
    await sectionShows("← is All", (d) => d.view === "all");
    api.toursClick("view:on");
    await sectionShows("the button", (d) => d.view === "on");
    // nothing selected: the button is disabled, the list is All, and what was chosen is kept
    api.dispatch({ type: "select", id: null }, "main");
    const none = await sectionShows("no selection", (d) => d.view === "all" && d.buttons[1] === "On selection", 20_000);
    assert.deepStrictEqual(none.buttons, [`All ${all}`, "On selection"]);
    assert.strictEqual(api.session().tourList.view, "onSelection", "the choice is kept");
    api.toursClick("viewkey:ArrowRight");
    await sleep(300);
    assert.strictEqual(section()!.view, "all", "→ goes nowhere: the second button is disabled");
  });

  test("▶ N is on the card and the row of a node that is on tours, from the core's count; a click shows its tours; deleting a tour lowers it, and at 0 it goes", async function () {
    this.timeout(120_000);
    api.dispatch({ type: "tourView", view: "all" });
    const make = async (title: string) => {
      await sleep(1100);
      const id = JSON.parse(tourCmd(["start", "--title", title, "--author", "the suite", "--json"])).tour;
      tourCmd(["add", id, lone("solo"), "--body-file", "-"], ONE);
      tourCmd(["finish", id, "--no-ids"]);
      return id;
    };
    solo1 = await make("Solo once");
    const solo2 = await make("Solo twice");
    // the node is shown on the canvas and in the structure: a reveal opens what is above it
    api.dispatch({ type: "activate", id: "main" }, "main");
    api.dispatch({ type: "reveal", id: lone("solo"), canvas: true }, "main");
    const chip = (where: () => { id: string; tours: number; tip: string }[]) => where().find((c) => c.id === lone("solo"));
    await until("the card and the row say ▶ 2", () => chip(canvasChips)?.tours === 2 && chip(sidebarChips)?.tours === 2, 30_000);
    assert.strictEqual((await counts())[lone("solo")], 2, "the same number as the core's");
    assert.strictEqual(chip(sidebarChips)?.tip, "on 2 tours: click to see them");
    assert.strictEqual(chip(canvasChips)?.tip, "on 2 tours: click to see them");
    assert.strictEqual(chip(sidebarChips)?.id && sidebarChips().some((c) => c.id === lone("nobody")), false, "a node no tour goes through shows nothing");
    // a click on the chip: the node is selected, TOURS shows the second view
    api.dispatch({ type: "select", id: null }, "main");
    api.toursClick("fold");
    await api.showTours(lone("solo"), "main");
    await until("selected, and the second view chosen", () => api.session().contexts.main.selected === lone("solo") && api.session().tourList.view === "onSelection");
    const shown = await sectionShows("the section shows the node's tours", (d) => !d.folded && d.mode === "list" && d.view === "on" && d.list.length === 2, 30_000);
    assert.deepStrictEqual(shown.list.map((r) => r.id).sort(), [solo1, solo2].sort());
    assert.deepStrictEqual(shown.list.map((r) => r.line2), ["starts here", "starts here"]);
    // Show Tours in the node's menu does the same
    api.dispatch({ type: "tourView", view: "all" });
    api.dispatch({ type: "select", id: null }, "main");
    api.toursClick("fold");
    await vscode.commands.executeCommand("vnoc.showTours", { nodeKey: lone("solo"), contextId: "main" });
    await until("the menu command did the same", () => api.session().contexts.main.selected === lone("solo") && api.session().tourList.view === "onSelection");
    await sectionShows("the section shows it", (d) => !d.folded && d.view === "on" && d.list.length === 2, 30_000);
    // a tour deleted from a terminal: the count drops on the card and the row, and at 0 they go
    tourCmd(["delete", solo2]);
    await until("▶ 1", () => chip(canvasChips)?.tours === 1 && chip(sidebarChips)?.tours === 1, 30_000);
    assert.strictEqual(chip(sidebarChips)?.tip, "on 1 tour: click to see it");
    assert.strictEqual((await counts())[lone("solo")], 1);
    await sectionShows("the section has one tour", (d) => d.view === "on" && d.list.length === 1 && d.buttons[1] === "On solo 1", 30_000);
    tourCmd(["delete", solo1]);
    await until("no chip", () => chip(canvasChips) === undefined && chip(sidebarChips) === undefined, 30_000);
    assert.strictEqual((await counts())[lone("solo")], undefined);
    const empty = await sectionShows("the empty state", (d) => d.view === "on" && d.empty === "No tour goes through solo.", 30_000);
    assert.deepStrictEqual(empty.list, []);
  });

  test("a tour card shows +N: its node's calls that are not on the tour, as the core counts them", async function () {
    this.timeout(60_000);
    api.dispatch({ type: "tourView", view: "all" });
    api.toursClick("fold");
    api.toursClick("unfold");
    await sectionShows("the list has the tour", (d) => d.mode === "list" && d.list.some((r) => r.id === tour), 30_000);
    api.toursClick(`row:${tour}`);
    await until("the tour's context", () => api.session().active === `tour:${tour}`);
    const off = (await api.request<{ off: Record<string, number> }>("vnoc/tourCalls", { id: tour })).off;
    // checkout calls has_stock (a stop of this tour), reserve, charge and release (not stops)
    assert.strictEqual(JSON.stringify(off), JSON.stringify({ s1: 3 }));
    const map = await aMapShows(tour, "the cards say +N", (m) => m.cards.some((c) => c.off > 0));
    assert.deepStrictEqual(map.cards.map((c) => [c.id, c.off]), map.cards.map((c) => [c.id, off[c.id] ?? 0]), "each card says what the core said for its stop");
    // a move is a `tour` event: the card still says it
    api.stageClick("next");
    await sleep(500);
    assert.strictEqual(mapOfTour(tour)!.cards.find((c) => c.id === "s1")!.off, 3);
    api.stageClick("closeTour");
    await dropTourContext();
  });
});

// ── pin and IDs by a click (docs/26-tours/05-on-screen.md §3, §5 "Decided for TR22a") ───────────────────────────────────────────────────────────────────────────────
//
// The cover says why a tour is not pinned and offers what the settings allow; **Commit and Pin…** is one dialog; **Write IDs for Its Stops** saves first and says what it wrote; **Pin** and
// **Tour Settings…** are commands. The suite cannot click in a webview or answer a message: it presses the page's buttons through the host and queues the answers. Each test makes its own
// tour of its own edit, and puts the workspace back: git reset to the commit it had at the start of the suite, the files this test made removed, `.vnoc/vnoc.toml` as it was.

suite("pin and IDs by a click", () => {
  const gitOut = (...args: string[]) => execFileSync("git", ["-c", "user.name=V-NOC test", "-c", "user.email=test@vnoc.invalid", "-c", "commit.gpgsign=false", ...args], { cwd: workspace(), encoding: "utf8" });
  const shop = () => path.join(workspace(), "tourdemo", "shop.py");
  const toml = () => path.join(workspace(), ".vnoc", "vnoc.toml");
  const untracked = () => gitOut("status", "--porcelain", "--untracked-files=all").split("\n").filter((l) => l.startsWith("??")).map((l) => l.slice(3));
  let head0 = "";
  let toml0: string | null = null;
  let untracked0 = new Set<string>();
  const dialog = () => api.pinDialog().at(-1)?.dialog ?? null;
  const dialogShows = async (what: string, ready: (d: NonNullable<ReturnType<typeof dialog>>) => boolean, ms = 30_000) => {
    await until(what, () => {
      const d = dialog();
      return d !== null && ready(d);
    }, ms);
    return dialog()!;
  };
  /** the tour the way an agent writes it, of what shop.py is now: checkout and has_stock, finished with no IDs (the settings decide the rest) */
  const finished = (title: string): string => {
    const id = JSON.parse(tourCmd(["start", "--title", title, "--author", "the suite", "--json"])).tour;
    tourCmd(["add", id, "tourdemo/shop.py::checkout", "--body-file", "-"], CHECKOUT);
    tourCmd(["add", id, node("has_stock"), "--body-file", "-", "--from", "s1", "--line", "6"], SHORT.has_stock);
    tourCmd(["finish", id, "--no-ids"]);
    return id;
  };
  /** work that is not committed: a note after the last function (no stop of the tours moves) */
  const leaveWork = () => fs.appendFileSync(shop(), "\n# not committed yet\n");
  const openTour = async (id: string) => {
    api.toursClick("fold");
    api.toursClick("unfold");
    await sectionShows("the list has the tour", (d) => d.mode === "list" && d.list.some((r) => r.id === id), 30_000);
    api.toursClick(`row:${id}`);
    await until("the tour's context", () => api.session().active === `tour:${id}`);
    await toCover(id);
  };
  const messagesFrom = (n: number) => api.tourMessages().slice(n);

  suiteSetup(async function () {
    this.timeout(60_000);
    api.stageClick("closeTour");
    await dropTourContext();
    head0 = gitOut("rev-parse", "HEAD").trim();
    toml0 = fs.existsSync(toml()) ? fs.readFileSync(toml(), "utf8") : null;
  });

  setup(() => {
    untracked0 = new Set(untracked());
    api.answerTourMessages([]);
  });

  teardown(async function () {
    this.timeout(30_000);
    api.answerTourMessages(null);
    api.answerTourPicks(null);
    api.pinDialogClick("cancel");
    api.stageClick("closeTour");
    await dropTourContext();
    await vscode.window.tabGroups.close(vscode.window.tabGroups.all.flatMap((g) => g.tabs).filter((t) => t.input instanceof vscode.TabInputText));
    gitOut("reset", "--hard", head0);
    for (const f of untracked()) if (!untracked0.has(f) && !f.startsWith(".vnoc/")) fs.rmSync(path.join(workspace(), f), { force: true });
    if (toml0 === null) fs.rmSync(toml(), { force: true });
    else fs.writeFileSync(toml(), toml0);
  });

  test("a tour of uncommitted work: the cover says not pinned, why, and what it offers; Commit and Pin… lists exactly those files, and Cancel commits nothing", async function () {
    this.timeout(90_000);
    leaveWork();
    const id = finished("Uncommitted shop");
    await openTour(id);
    const cover = await stageShows("not pinned, with its reason", (d) => d.mode === "cover" && d.pin === "not pinned" && d.reason !== null, 30_000, id);
    assert.strictEqual(cover.reason, "1 file has changes that are not committed", "the core's words");
    assert.deepStrictEqual(cover.offers, ["2 stops have no ID: the tour will not follow a rename. | Write IDs", "1 file has changes that are not committed. | Commit and Pin…"]);
    // the dry pin names the files, and wrote nothing
    const dry = await api.request<{ state: string; reasons: { code: string; files: { path: string }[]; words: string }[] }>("vnoc/tourPin", { id, dry: true });
    assert.deepStrictEqual(dry.reasons[0].files.map((f) => f.path), ["tourdemo/shop.py"]);
    assert.ok(!JSON.parse(tourCmd(["show", id, "--json"])).pinned, "asking why did not pin it");
    // the dialog lists exactly them, with the message the core would use
    const status = gitOut("status", "--porcelain");
    const commits = gitOut("log", "--oneline");
    api.stageClick("commitAndPin");
    const open = await dialogShows("the dialog", (d) => d.tour === id && d.files.length > 0);
    assert.deepStrictEqual(open.files, ["tourdemo/shop.py"]);
    assert.strictEqual(open.message, 'Code for the tour "Uncommitted shop"');
    assert.strictEqual(open.always, false);
    assert.strictEqual(open.error, null);
    // Cancel: nothing is committed, nothing is staged
    api.pinDialogClick("cancel");
    await until("the dialog closed", () => dialog() === null);
    assert.strictEqual(gitOut("status", "--porcelain"), status, "git status is as it was");
    assert.strictEqual(gitOut("log", "--oneline"), commits, "git log is as it was");
    assert.strictEqual(gitOut("diff", "--cached", "--name-only"), "", "nothing staged");
  });

  test("Commit and Pin commits those files and the cover says pinned at …; the check box writes commit = always and changes nothing else in vnoc.toml", async function () {
    this.timeout(90_000);
    leaveWork();
    const id = finished("Commit and pin");
    await openTour(id);
    await stageShows("not pinned", (d) => d.mode === "cover" && d.pin === "not pinned", 30_000, id);
    api.stageClick("commitAndPin");
    await dialogShows("the dialog", (d) => d.tour === id && d.files.length > 0);
    api.pinDialogClick("message", "Code for the pin suite");
    api.pinDialogClick("always");
    const typed = await dialogShows("message and check box", (d) => d.message === "Code for the pin suite" && d.always);
    assert.deepStrictEqual(typed.files, ["tourdemo/shop.py"]);
    const before = toml0 ?? "";
    api.pinDialogClick("commit");
    await until("the dialog closed", () => dialog() === null, 30_000);
    assert.strictEqual(gitOut("log", "-1", "--format=%s").trim(), "Code for the pin suite");
    assert.strictEqual(gitOut("show", "--name-only", "--format=", "HEAD").trim(), "tourdemo/shop.py", "exactly the tour's file");
    const pinned = await stageShows("pinned at the new commit", (d) => d.mode === "cover" && d.pin?.startsWith("pinned at ") === true, 30_000, id);
    assert.strictEqual(pinned.pin, `pinned at ${gitOut("rev-parse", "--short=7", "HEAD").trim()}`);
    assert.strictEqual(pinned.reason, null);
    // the setting: `commit = "always"` in `[tours]`, and every other line of the file as it was
    const after = fs.readFileSync(toml(), "utf8");
    assert.ok(/^\[tours\]/m.test(after) && /^commit = "always"$/m.test(after), after);
    const was = new Set(before.split("\n"));
    const added = after.split("\n").filter((l) => !was.has(l) && l.trim() !== "");
    assert.deepStrictEqual(added.filter((l) => l !== "[tours]" && l !== 'commit = "always"'), [], `nothing else changed: ${after}`);
    assert.strictEqual((await api.request<{ commit: { value: string } }>("vnoc/tourSettings")).commit.value, "always");
  });

  test("with never the cover offers nothing but still says not pinned, and the menu's Commit and Pin… still works", async function () {
    this.timeout(90_000);
    await api.request("vnoc/tourSettings", { ids: "never", commit: "never" });
    leaveWork();
    const id = finished("Never offered");
    await openTour(id);
    const cover = await stageShows("not pinned and nothing offered", (d) => d.mode === "cover" && d.pin === "not pinned" && d.reason !== null, 30_000, id);
    assert.deepStrictEqual(cover.offers, []);
    assert.strictEqual(cover.reason, "1 file has changes that are not committed");
    await vscode.commands.executeCommand("vnoc.tourCommitAndPin", { tour: id });
    const open = await dialogShows("the dialog from the menu", (d) => d.tour === id && d.files.length > 0);
    assert.deepStrictEqual(open.files, ["tourdemo/shop.py"]);
    api.pinDialogClick("cancel");
    await until("the dialog closed", () => dialog() === null);
  });

  test("Pin on a tour with files not committed is refused in the core's words, and its message offers Commit and Pin…", async function () {
    this.timeout(90_000);
    leaveWork();
    const id = finished("Refused pin");
    const said = api.tourMessages().length;
    api.answerTourMessages(["Commit and Pin…"]);
    await vscode.commands.executeCommand("vnoc.tourPin", { tour: id });
    assert.deepStrictEqual(messagesFrom(said)[0], { level: "warn", text: "Not pinned: 1 file has changes that are not committed.", buttons: ["Commit and Pin…"] });
    const open = await dialogShows("the dialog the message opened", (d) => d.tour === id && d.files.length > 0);
    assert.deepStrictEqual(open.files, ["tourdemo/shop.py"]);
    api.pinDialogClick("cancel");
    await until("the dialog closed", () => dialog() === null);
  });

  test("Pin on a tour that can be pinned pins it, at HEAD", async function () {
    this.timeout(90_000);
    leaveWork();
    const id = finished("Pinnable");
    assert.ok(!JSON.parse(tourCmd(["show", id, "--json"])).pinned);
    gitOut("commit", "-qam", "the work the tour is about");
    const said = api.tourMessages().length;
    await vscode.commands.executeCommand("vnoc.tourPin", { tour: id });
    const head = gitOut("rev-parse", "HEAD").trim();
    assert.deepStrictEqual(messagesFrom(said).map((m) => m.text), [`Pinned at ${head.slice(0, 7)}.`]);
    const shown = JSON.parse(tourCmd(["show", id, "--json"]));
    assert.strictEqual(shown.pinned, true);
    assert.strictEqual(shown.pin, head);
  });

  test("Write IDs for Its Stops: a dirty stop file is saved first, after a question; then the IDs are written, and a rename in the source keeps the stop found", async function () {
    this.timeout(120_000);
    const id = finished("IDs for the stops");
    await sleep(1500); // the editor's file watcher has told it of the last test's reset: a buffer made dirty before that would meet a file it thinks changed under it
    const doc = await vscode.workspace.openTextDocument(shop());
    const editor = await vscode.window.showTextDocument(doc);
    await editor.edit((b) => b.insert(new vscode.Position(doc.lineCount, 0), "\n# unsaved\n"));
    assert.ok(doc.isDirty);
    await until("the host knows the file is dirty", () => api.dirtyFiles().includes("tourdemo/shop.py"));
    // Cancel: nothing is saved and nothing is written
    let said = api.tourMessages().length;
    api.answerTourMessages(["Cancel"]);
    await vscode.commands.executeCommand("vnoc.tourWriteIds", { tour: id });
    assert.deepStrictEqual(messagesFrom(said), [{ level: "warn", text: "Save 1 file first? The IDs are written into it.", buttons: ["Save and Write IDs", "Cancel"] }]);
    assert.ok(doc.isDirty, "not saved");
    assert.ok(!/ID: /.test(fs.readFileSync(shop(), "utf8")), "no ID written");
    // Save and Write IDs
    said = api.tourMessages().length;
    api.answerTourMessages(["Save and Write IDs", undefined]);
    await vscode.commands.executeCommand("vnoc.tourWriteIds", { tour: id });
    const sent = messagesFrom(said);
    assert.strictEqual(sent[0].text, "Save 1 file first? The IDs are written into it.");
    assert.ok(/^IDs written into \d+ files?\.$/.test(sent[1].text), sent[1].text);
    assert.deepStrictEqual(sent[1].buttons, ["Show Changes", "Always in This Project"]);
    assert.ok(!doc.isDirty, "saved first");
    const written = fs.readFileSync(shop(), "utf8");
    assert.ok(written.includes("# unsaved"), "what was typed was saved, not lost");
    assert.ok((written.match(/ID: [0-9A-Z]+/g) ?? []).length >= 2, `an ID line for each stop: ${written}`);
    // a rename in the source: the stop is found by its ID
    fs.writeFileSync(shop(), written.replace("def has_stock(", "def in_stock("));
    // what `vnoc tour check` finds by itself, reading the files: the stop is found by its ID, under its new name (it would be gone without one)
    const checked = JSON.parse(tourCmd(["check", id, "--json"])).tours[0].stops.find((x: { id: string }) => x.id === "s2");
    assert.ok(checked && checked.state !== "gone", `the stop follows the rename: ${JSON.stringify(checked)}`);
    // and the running core, once it has the file: `now` says where the stop is, by its ID
    let found: { check?: string; now?: { qualname: string } } | undefined;
    for (let waited = 0; waited < 30_000; waited += 500) {
      const view = await api.request<{ stops: { id: string; check?: string; now?: { qualname: string } }[] }>("vnoc/tour", { id });
      found = view.stops.find((x) => x.id === "s2");
      if (found?.now?.qualname === "in_stock") break;
      await sleep(500);
    }
    assert.ok(found?.now?.qualname === "in_stock" && found.check !== "gone", `the running core follows the rename: ${JSON.stringify(found)}`);
  });

  test("Write IDs in a project that writes no IDs shows the core's refusal", async function () {
    this.timeout(60_000);
    const id = finished("No IDs here");
    const original = fs.existsSync(toml()) ? fs.readFileSync(toml(), "utf8") : "";
    fs.mkdirSync(path.dirname(toml()), { recursive: true });
    fs.writeFileSync(toml(), `${original}\n[ids]\ndefault = "none"\n`);
    const said = api.tourMessages().length;
    await vscode.commands.executeCommand("vnoc.tourWriteIds", { tour: id });
    const sent = messagesFrom(said);
    assert.strictEqual(sent.length, 1, JSON.stringify(sent));
    assert.strictEqual(sent[0].level, "error");
    assert.ok(sent[0].text.includes('[ids] default = "none"'), sent[0].text);
  });

  test("Tour Settings… picks a setting and then a value, and writes it: ids = never", async function () {
    this.timeout(60_000);
    api.answerTourPicks(["IDs for the stops", "never"]);
    await vscode.commands.executeCommand("vnoc.tourSettings");
    const after = fs.readFileSync(toml(), "utf8");
    assert.ok(/^ids = "never"$/m.test(after), after);
    assert.strictEqual((await api.request<{ ids: { value: string } }>("vnoc/tourSettings")).ids.value, "never");
  });
});
