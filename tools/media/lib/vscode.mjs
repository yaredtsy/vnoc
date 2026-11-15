// Starts a clean VS Code with V-NOC installed, and reaches into its webviews (docs/18-release-0.2/01-media.md §4).
import { execFileSync } from "node:child_process";
import { mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { downloadAndUnzipVSCode, resolveCliArgsFromVSCodeExecutablePath } from "@vscode/test-electron";
import { _electron } from "playwright-core";

import { here, paths, repoRoot, vscodeVersion, window as size } from "../config.mjs";

/**
 * Launches VS Code on `workspace` in a fresh profile (`settings`: what this scene changes of tools/media/settings.json). `vsix`: the extension is installed from that file (what users get). `dev`: the extension is
 * loaded from this repository instead (for looking around while writing a scene: the title bar then says Extension Development Host).
 */
export async function launch({ workspace, vsix, dev = false, settings = {} }) {
  // the executable is the one the window suite uses, downloaded once into extension/.vscode-test
  const executablePath = await downloadAndUnzipVSCode({ version: vscodeVersion, cachePath: join(repoRoot, "extension/.vscode-test") });
  const [cli, ...cliArgs] = resolveCliArgsFromVSCodeExecutablePath(executablePath);
  const profile = join(paths.work, "profile");
  rmSync(profile, { recursive: true, force: true });
  const userData = join(profile, "user-data");
  const extensions = join(profile, "extensions");
  mkdirSync(join(userData, "User"), { recursive: true });
  mkdirSync(extensions, { recursive: true });
  // the settings of every picture (settings.json), and what one scene changes (`export const settings` of the scene)
  const base = JSON.parse(readFileSync(join(here, "settings.json"), "utf8"));
  writeFileSync(join(userData, "User/settings.json"), JSON.stringify({ ...base, ...settings }, null, 2));

  // ELECTRON_RUN_AS_NODE makes VS Code start as plain Node (docs/16-ui-redesign/10-build-order.md §1): a shell started from VS Code has it
  const env = { ...process.env };
  delete env.ELECTRON_RUN_AS_NODE;

  const args = [workspace, `--user-data-dir=${userData}`, `--extensions-dir=${extensions}`, "--skip-welcome", "--skip-release-notes", "--disable-workspace-trust", "--disable-updates"];
  if (dev) args.push(`--extensionDevelopmentPath=${join(repoRoot, "extension")}`);
  else execFileSync(cli, [...cliArgs, `--user-data-dir=${userData}`, `--extensions-dir=${extensions}`, "--install-extension", vsix], { env, stdio: "inherit" });

  const app = await _electron.launch({ executablePath, args, env });
  const page = await app.firstWindow();
  await app.evaluate(({ BrowserWindow }, { width, height }) => {
    const win = BrowserWindow.getAllWindows()[0];
    win.setContentSize(width, height);
    win.center();
  }, size);
  await page.waitForSelector(".monaco-workbench");
  return { app, page, userData };
}

/** All frames of the window that contain `selector`, innermost first (a webview is an iframe in an iframe; the nesting is not relied on). */
export async function framesWith(page, selector) {
  const found = [];
  for (const frame of page.frames()) {
    try {
      if (await frame.locator(selector).count()) found.push(frame);
    } catch {
      // a frame that is gone or not ready yet has no answer
    }
  }
  return found;
}

/** The frame that has `selector`, waiting for it. */
export async function webviewWith(page, selector, { timeout = 30_000 } = {}) {
  const deadline = Date.now() + timeout;
  for (;;) {
    const [frame] = await framesWith(page, selector);
    if (frame) return frame;
    if (Date.now() > deadline) throw new Error(`no webview has ${selector} after ${timeout} ms`);
    await page.waitForTimeout(250);
  }
}

/**
 * A command from the palette. It waits for the box, types the title, and presses Enter only when the first entry is that command: a keystroke that
 * missed the box would land in the editor or the terminal, and a title that matches nothing must be an error, not a guess. Not used while a GIF is recorded.
 */
export async function runCommand(page, title) {
  const widget = page.locator(".quick-input-widget");
  const input = widget.locator("input.input");
  for (let attempt = 0; attempt < 3; attempt++) {
    await page.keyboard.press("F1");
    try {
      await input.waitFor({ state: "visible", timeout: 2000 });
      break;
    } catch {
      if (attempt === 2) throw new Error(`the command palette did not open for "${title}"`);
    }
  }
  await input.fill(`>${title}`);
  const first = widget.locator(".quick-input-list .monaco-list-row").first();
  await first.waitFor({ state: "visible", timeout: 5000 });
  await page.waitForTimeout(250);
  const label = ((await first.getAttribute("aria-label")) ?? "").trim();
  if (!label.toLowerCase().includes(title.toLowerCase().replace(/^.*?: /, ""))) {
    await page.keyboard.press("Escape");
    throw new Error(`the palette's first entry for "${title}" is "${label}"`);
  }
  await page.keyboard.press("Enter");
  await widget.waitFor({ state: "hidden", timeout: 5000 }).catch(() => {});
}
