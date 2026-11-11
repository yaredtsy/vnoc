import * as assert from "assert";
import * as vscode from "vscode";

// helpers for the suites that drive the real window

export const sleep = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));
export const canvasTabs = () => vscode.window.tabGroups.all.flatMap((g) => g.tabs).filter((t) => t.input instanceof vscode.TabInputWebview && t.input.viewType.endsWith("vnoc.canvas"));

export async function until(what: string, check: () => boolean, ms = 20_000) {
  for (let waited = 0; !check(); waited += 100) {
    if (waited >= ms) assert.fail(`timed out waiting for ${what}`);
    await sleep(100);
  }
}

/** Waits for an async check, polling every 200 ms. */
export async function waitFor(what: string, check: () => Promise<boolean> | boolean, ms = 20_000) {
  for (let waited = 0; !(await check()); waited += 200) {
    assert.ok(waited < ms, `timed out waiting for ${what}`);
    await sleep(200);
  }
}

export async function closeCanvas() {
  await vscode.window.tabGroups.close(canvasTabs());
  await until("the canvas tab to close", () => canvasTabs().length === 0);
}

