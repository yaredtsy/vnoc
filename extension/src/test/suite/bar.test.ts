import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";
import * as vscode from "vscode";

import { canvasTabs, until } from "./util";

// The canvas's bar in a real window (docs/16-ui-redesign/02-top-bar.md §3 #4): a file the driver cannot read makes the page show the ⚠.
// The suite cannot see the page, so it listens to what the page posts (`warned`), as the skeleton suite does with `rendered`.

interface Warned {
  level: "none" | "warning" | "error";
  text: string;
}
let api: { waitWarned(ready: (w: Warned) => boolean, ms: number): Promise<Warned>; webviewErrors(): string[] };
let file: string;

suiteSetup(async () => {
  api = await vscode.extensions.getExtension("vnoc.vnoc")!.activate();
  file = path.join(vscode.workspace.workspaceFolders![0].uri.fsPath, "unreadable.py");
});

suiteTeardown(() => {
  fs.rmSync(file, { force: true });
});

suite("the canvas bar", () => {
  test("a file that cannot be read shows the ⚠, and it goes when the file is fixed", async () => {
    if (canvasTabs().length === 0) await vscode.commands.executeCommand("vnoc.openCanvas");
    await until("the canvas tab", () => canvasTabs().length > 0);

    fs.writeFileSync(file, Buffer.from([0x64, 0x65, 0x66, 0x20, 0xff, 0xfe, 0x0a])); // not UTF-8: the driver fails on it
    const shown = await api.waitWarned((w) => w.level === "warning", 30_000);
    assert.match(shown.text, /1 file could not be read/);

    fs.writeFileSync(file, "def fine():\n    pass\n");
    await api.waitWarned((w) => w.level === "none", 30_000);
    assert.deepStrictEqual(api.webviewErrors(), [], "the page drew the bar without crashing");
  });
});
