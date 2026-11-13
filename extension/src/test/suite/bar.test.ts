import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";
import * as vscode from "vscode";

import { canvasTabs, until } from "./util";

// The canvas's bar in a real window (docs/16-ui-redesign/02-top-bar.md §3 #4, §4). The suite cannot see the page, so it listens to what the page
// posts (`warned`, `rendered`), as the skeleton suite does. The second suite stops the core: it must stay the LAST one in index.ts.

interface Warned {
  level: "none" | "warning" | "error";
  text: string;
}
let api: {
  waitWarned(ready: (w: Warned) => boolean, ms: number): Promise<Warned>;
  webviewErrors(): string[];
  renderedPosts(): { visible: number; hidden: number }[];
  breakCore(): Promise<void>;
  emitIndexChanged(): void;
};

suiteSetup(async () => {
  api = await vscode.extensions.getExtension("vnoc.vnoc")!.activate();
});

const openCanvas = async () => {
  if (canvasTabs().length === 0) await vscode.commands.executeCommand("vnoc.openCanvas");
  await until("the canvas tab", () => canvasTabs().length > 0);
};

suite("the canvas bar", () => {
  const file = () => path.join(vscode.workspace.workspaceFolders![0].uri.fsPath, "unreadable.py");

  test("a file that cannot be read shows the ⚠, and it goes when the file is fixed", async () => {
    await openCanvas();
    try {
      fs.writeFileSync(file(), Buffer.from([0x64, 0x65, 0x66, 0x20, 0xff, 0xfe, 0x0a])); // not UTF-8: the driver fails on it
      const shown = await api.waitWarned((w) => w.level === "warning", 30_000);
      assert.match(shown.text, /1 file could not be read/);

      fs.writeFileSync(file(), "def fine():\n    pass\n");
      await api.waitWarned((w) => w.level === "none", 30_000);
      assert.deepStrictEqual(api.webviewErrors(), [], "the page drew the bar without crashing");
    } finally {
      fs.rmSync(file(), { force: true });
    }
  });

  test("the ⚠'s click target exists: `vnoc.attention.focus` runs without an error", async () => {
    await vscode.commands.executeCommand("vnoc.attention.focus");
  });
});

suite("the canvas bar when the core is gone (keep this suite last)", () => {
  test("a refresh that fails over a drawn canvas keeps the cards and turns the ⚠ red", async () => {
    await openCanvas();
    await api.waitWarned((w) => w.level === "none", 30_000);
    const drawn = api.renderedPosts().length;
    assert.ok(drawn > 0 && api.renderedPosts().at(-1)!.visible > 0, "cards are drawn before the core goes");

    await api.breakCore();
    api.emitIndexChanged(); // the page reads the structure again, and the core does not answer
    const shown = await api.waitWarned((w) => w.level === "error", 30_000);
    assert.match(shown.text, /^Could not read the code again/);

    assert.ok(api.renderedPosts().every((r) => r.visible > 0), "the page never drew an empty canvas over the cards");
    assert.deepStrictEqual(api.webviewErrors(), [], "and did not crash");
  });
});
