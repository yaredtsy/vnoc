import * as assert from "assert";
import { execFileSync } from "child_process";
import * as fs from "fs";
import * as path from "path";
import * as vscode from "vscode";

import type { Notes } from "@vnoc/shared/messages";
import { canvasTabs, guarded, soft, until } from "./util";

// A core whose first build is held (`VNOC_HOLD_FIRST_BUILD_MS`) for longer than a request may wait (`VNOC_WAIT_LIMIT_MS`): the core refuses what needs the index with `data.askAgain`, and the host asks
// again every 2 s. Its count of a tour's calls never ends (`VNOC_HOLD_TOUR_CALLS_MS`): the stage draws the code of a stop without the `+N`. This suite runs in a window of its own (`runTest.ts` starts it with
// those variables); the rest of a run has a core that is never held.

interface Api {
  stageDrawn(): Extract<Notes, { type: "stageDrawn" }>[];
  stageClick(button: string): void;
  toursDrawn(): Extract<Notes, { type: "toursDrawn" }>[];
  webviewErrors(): string[];
  cli: { binaryPath(): string | undefined; driverPaths(): string[] };
  request<T>(method: string, params?: object): Promise<T>;
  tourMessages(): { level: string; text: string; buttons: string[] }[];
  session(): { active: string };
}
let api: Api;
const suiteSetup = (fn: (this: Mocha.Context) => unknown) => globalThis.suiteSetup(soft(fn));
const workspace = () => vscode.workspace.workspaceFolders![0].uri.fsPath;
const vnoc = (args: string[], input = "") =>
  execFileSync(api.cli.binaryPath()!, ["--driver", api.cli.driverPaths().find((p) => p.includes("python"))!, ...args], { cwd: workspace(), encoding: "utf8", input, stdio: ["pipe", "pipe", "pipe"], env: { ...process.env, VNOC_DRIVER: "", VNOC_DRIVERS: "" } });

suiteSetup(async () => {
  api = guarded(await vscode.extensions.getExtension("vnoc.vnoc")!.activate());
});

suite("a core whose first build is held", () => {
  test("the core refuses with data.askAgain; the host asks again, so the list and a tour's cover come when the build ends, with no error shown", async function () {
    this.timeout(120_000);
    // the host's own requests are not the guarded ones: they wait for the build (about 12 s), and the guard gives a request 10 s
    const host: { tours: { list(): Promise<{ id: string; words: string }[]> }; request<T>(method: string, params?: object): Promise<T> } = await vscode.extensions.getExtension("vnoc.vnoc")!.activate();

    // the core, asked directly while the build is held: a refusal with the words and the flag (what the host and the pages read; the words are not)
    const refused = await host.request("vnoc/tourCounts").then(() => null, (e: { message: string; code: number; data?: unknown }) => e);
    assert.ok(refused, "the first build must still be held here (12 s from the core's start): the core answered instead of refusing");
    console.log(`refusal: ${JSON.stringify({ code: refused.code, data: refused.data, message: refused.message })}`);
    assert.deepStrictEqual(refused.data, { askAgain: true });
    assert.ok(refused.message.endsWith("ask again"), refused.message);

    // a tour, written by the command line (it reads the code with a driver of its own, so the held core does not matter to it)
    const dir = path.join(workspace(), "tourdemo");
    fs.mkdirSync(dir, { recursive: true });
    fs.writeFileSync(path.join(dir, "shop.py"), "def checkout(order):\n    return order\n");
    const id: string = JSON.parse(vnoc(["tour", "start", "--title", "A held tour", "--question", "does it wait", "--author", "the suite", "--json"])).tour;
    vnoc(["tour", "add", id, "tourdemo/shop.py::checkout", "--body-file", "-"], "checkout returns the order it was given.\n");
    vnoc(["tour", "finish", id, "--no-ids"]);
    // its code changes after the tour was recorded: what the list and the tour say about it needs a real check, and a real check needs the index
    fs.appendFileSync(path.join(dir, "shop.py"), "\n\ndef later():\n    return 1\n");

    // the list as Open Tour… asks it, and a tour opened by its command, both while the build is held
    const listing = host.tours.list();
    await vscode.commands.executeCommand("vnoc.openTour", id);
    await until("the tour's context", () => api.session().active === `tour:${id}`);
    await until("the canvas tab", () => canvasTabs().length > 0);

    // the build ends: the list is the core's, with the tour in it (it did not fail with the refusal), and the cover is drawn
    const rows = await listing;
    assert.ok(rows.some((r) => r.id === id), `the list has the tour: ${JSON.stringify(rows)}`);
    await until("the cover", () => api.stageDrawn().some((d) => d.tour === id && d.mode === "cover"), 30_000);
    // the sidebar's TOURS section (its page starts when the structure is there) lists it too
    await vscode.commands.executeCommand("vnoc.structure.focus");
    await until("the TOURS section to list the tour", () => api.toursDrawn().at(-1)?.list.some((r) => r.id === id) === true, 30_000);
    assert.strictEqual(api.toursDrawn().at(-1)!.waiting, null, "no waiting line is left");
    assert.deepStrictEqual(api.webviewErrors(), [], "no page showed an error");
    assert.deepStrictEqual(api.tourMessages().filter((m) => m.level === "error"), [], "no message said an error");
    assert.deepStrictEqual(await host.request("vnoc/tourCounts").then(() => "answered", () => "refused"), "answered", "the build is over");
  });

  test("the stage draws the code of a stop in a big file at once, though the count of the tour's calls never ends", async function () {
    this.timeout(120_000);
    const host: { request<T>(method: string, params?: object): Promise<T> } = await vscode.extensions.getExtension("vnoc.vnoc")!.activate();
    // a file of 3,000 lines with the stop at its end, and a small one for the first stop: the arrival in the big file is the first
    const dir = path.join(workspace(), "tourbig");
    fs.mkdirSync(dir, { recursive: true });
    fs.writeFileSync(path.join(dir, "small.py"), "def first(x):\n    return x\n");
    const filler = Array.from({ length: 1000 }, (_, i) => `def filler_${i}(x):\n    return x + ${i}\n\n`).join("");
    fs.writeFileSync(path.join(dir, "big.py"), `${filler}def last_stop(order):\n    total = 0\n    for line in order:\n        total += line\n    return total\n`);
    const id: string = JSON.parse(vnoc(["tour", "start", "--title", "A big file", "--question", "does it draw", "--author", "the suite", "--json"])).tour;
    vnoc(["tour", "add", id, "tourbig/small.py::first", "--body-file", "-"], "first is only here to be left.\n");
    vnoc(["tour", "add", id, "tourbig/big.py::last_stop", "--body-file", "-"], "last_stop adds up the lines of an order, in a file of three thousand lines.\n");
    vnoc(["tour", "finish", id, "--no-ids"]);
    await vscode.commands.executeCommand("vnoc.openTour", id);
    await until("the cover", () => api.stageDrawn().some((d) => d.tour === id && d.mode === "cover"), 30_000);
    api.stageClick("start");
    await until("the first stop", () => api.stageDrawn().some((d) => d.tour === id && d.stop === "s1" && d.code?.text.includes("def first") === true), 30_000);
    // the count of the calls is still going on: it has not answered, and it will not for 15 minutes
    const count = await host.request("vnoc/tourCalls", { id }).then(() => "answered", (e: { data?: { askAgain?: boolean } }) => (e.data?.askAgain ? "asks again" : "failed"));
    assert.strictEqual(count, "asks again");
    const drawn = api.stageDrawn().length;
    const before = Date.now();
    api.stageClick("jump:s2");
    let took = -1;
    while (Date.now() - before < 10_000) {
      if (api.stageDrawn().slice(drawn).some((d) => d.tour === id && d.stop === "s2" && d.code?.text.includes("def last_stop") === true)) {
        took = Date.now() - before;
        break;
      }
      await new Promise((r) => setTimeout(r, 20));
    }
    console.log(`the code of s2 (the first arrival in a 3,000-line file) was drawn after ${took} ms`);
    assert.ok(took >= 0 && took < 1000, `the code of the stop was drawn after ${took} ms (-1: never in 10 s); the stage drew ${JSON.stringify(api.stageDrawn().slice(drawn).slice(-4).map((d) => [d.mode, d.stop, d.step, d.code?.first, d.code?.text.slice(0, 30), d.header]))}; the core says ${vnoc(["tour", "where", id])}`);
    assert.deepStrictEqual(api.webviewErrors(), [], "no page showed an error");
  });
});
