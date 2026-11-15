// What every scene needs from a running VS Code: the demo opened, V-NOC's sidebar and canvas found by their accessible names, the cursor, one way to take a
// shot. Rows and cards are found by `aria-label` (webview/src/node-card/labels.ts), never by CSS classes.
import { mkdirSync } from "node:fs";
import { dirname } from "node:path";

import { Cursor } from "./cursor.mjs";
import { launch, runCommand, webviewWith } from "./vscode.mjs";
import { prepareDemo } from "./prepare.mjs";

const TREE = '[role=tree][aria-label="Structure"]';

export class Stage {
  constructor(app, page) {
    this.app = app;
    this.page = page;
    this.cursor = new Cursor(page);
  }

  static async open({ vsix, dev = false, settings } = {}) {
    const { app, page } = await launch({ workspace: prepareDemo(), vsix, dev, settings });
    const stage = new Stage(app, page);
    await page.waitForTimeout(2500);
    await stage.command("V-NOC: Focus on Structure View");
    stage.side = await webviewWith(page, TREE);
    stage.tree = stage.side.locator(TREE);
    await stage.cursor.install();
    await stage.tidy();
    return stage;
  }

  command(title) {
    return runCommand(this.page, title);
  }

  /**
   * Nothing the pictures must not show: no notification, no panel, no Chat (the secondary side bar), and Needs Attention folded away. A fresh profile has the
   * panel and the Chat bar closed; this closes them if something opened one, and says so (a picture with one in it is wrong).
   */
  async tidy() {
    const visible = (selector) => this.page.locator(selector).first().isVisible();
    if (await visible(".part.auxiliarybar:not(.hidden)")) await this.command("View: Toggle Secondary Side Bar Visibility");
    if (await visible(".part.panel:not(.hidden)")) await this.command("View: Toggle Panel Visibility");
    if (await visible(".notifications-toasts.visible")) await this.command("Notifications: Clear All Notifications");
    await this.attention(false);
  }

  /** The side bar's width in CSS pixels, by dragging its edge (setup only: a GIF never resizes). */
  async resizeSidebar(width) {
    const edge = await this.page.evaluate(() => {
      const sashes = [...document.querySelectorAll(".monaco-sash.vertical")].map((e) => e.getBoundingClientRect()).filter((r) => r.left > 250 && r.left < 420);
      return sashes[0] && { x: sashes[0].left + sashes[0].width / 2, y: sashes[0].top + sashes[0].height / 2 };
    });
    if (!edge) throw new Error("the side bar's edge was not found");
    const activityBar = 48; // the Activity Bar is 48 px wide: the side bar starts there
    await this.page.mouse.move(edge.x, edge.y);
    await this.page.mouse.down();
    await this.page.mouse.move(activityBar + width, edge.y, { steps: 8 });
    await this.page.mouse.up();
    await this.page.waitForTimeout(400);
  }

  /** Needs Attention open or folded (it takes half the side bar: folded in every picture but S6). */
  async attention(open) {
    const header = this.page.locator(".pane-header", { hasText: /needs attention/i }).first();
    if ((await header.getAttribute("aria-expanded")) === String(open)) return;
    await header.click();
    await this.page.waitForTimeout(400);
  }

  /** A sidebar row by the start of its accessible name: `row("class Client,")`. */
  row(label) {
    return this.tree.locator(`[role=treeitem][aria-label^="${label}"]`).first();
  }

  /**
   * Opens a sidebar row with the keyboard (a click on a file would open it in the editor and take the keys), and waits until the row says it is open
   * (`aria-expanded`): the first keystroke can come before the page is ready for it, so it is repeated.
   */
  async openRow(label) {
    const row = this.row(label);
    for (let attempt = 0; attempt < 5; attempt++) {
      if ((await row.getAttribute("aria-expanded").catch(() => null)) === "true") return;
      await row.click();
      await this.tree.focus();
      await this.page.keyboard.press("ArrowRight");
      await this.page.waitForTimeout(500);
    }
    throw new Error(`the row "${label}" did not open`);
  }

  /** The canvas page: the frame that has React Flow's root. */
  async canvasFrame(timeout = 30_000) {
    return webviewWith(this.page, ".react-flow", { timeout });
  }

  async shot(file, { clip, cursor = false } = {}) {
    mkdirSync(dirname(file), { recursive: true });
    if (!cursor) await this.cursor.hide(true);
    await this.page.screenshot({ path: file, scale: "device", clip });
    await this.cursor.hide(false);
  }

  async close() {
    await this.app.close();
  }
}
