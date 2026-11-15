// G2 docs.gif: write a doc, rename the function, the doc stays (docs/18-release-0.2/01-media.md §5.1, §5.3).
// Setup, off camera: `_client.py` on the left at `def _send_handling_redirects`, an empty editor group on the right and active, the sidebar showing the function's
// row, the canvas closed. Then, with the mouse and the typing only: Add Doc… on the row, the title box, a line in the doc, a rename in the code.
// Where the plan said "right-click the name in the editor → V-NOC ▸ Add Doc…": a right-click makes the left group (the code) active, and the doc then opens as a
// tab on top of the code (checked: `showTextDocument` uses the active group). The row's own Add Doc… button runs the same command and leaves the focus in the
// right group, so the doc opens beside the code. The recording is not timed exactly: it marks what happens and `edit.mjs` cuts the waits afterwards (§7.2).
import { Recording } from "../record.mjs";

export const id = "g2";
export const kind = "gif";
export const output = "docs.gif";
// Two editor groups next to a side bar wide enough for the row's "1 doc" leave each group about 400 px: at 14 px the long `ID:` line does not fit, the editor
// scrolls sideways to show it, and the function's name leaves the screen. At 12 px it fits.
export const settings = { "editor.fontSize": 12, "editor.lineHeight": 20 };

const FN = "_send_handling_redirects";
const RENAMED = "_follow_redirects";
const LINE = "Follows redirects: each hop is one `_send_single_request`.";
// The text is typed, key by key, never pasted, and fast ("write with speed"): 25 ms between keys is about 40 characters a second, twice the plan's 20. With no pause at
// all the whole line is typed in half a second and the capture, which stalls while VS Code is busy, sees almost none of it. The edit speeds up what is still long (§7.2).
const TYPING_MS = 25;

// Holds in ms (§5.1, §5.3): 0.5 s on the first frame, 0.6 s after an action, 1.2 s where there is something new to read, 2.5 s on the last frame.
const FIRST = 500;
const AFTER = 600;
const READ = 1200;
const END = 2500;

const left = (page) => page.locator(".editor-group-container").first();

/** The row of the function in the sidebar: its name changes with the rename, so it is found by the one thing that does not: its place in the tree. */
const rowNamed = (stage, name) => stage.row(`function ${name},`);

async function setup(stage) {
  const { page } = stage;
  // wide enough for a row four levels deep with its name, its doc count and its three buttons: "1 doc" has to be readable on the row (§5.3)
  await stage.resizeSidebar(400);
  await stage.openRow("folder httpx,");
  await stage.openRow("file _client.py,");
  await stage.openRow("class Client,");
  await rowNamed(stage, FN).click(); // the canvas is closed: the code, at the function
  await left(page).locator(".view-line", { hasText: `def ${FN}(` }).first().waitFor({ timeout: 15_000 });
  await stage.command("View: New Editor Group to the Right"); // empty, and active: the doc opens there
  await page.waitForTimeout(500);
  const active = page.locator(".editor-group-container.active");
  if ((await active.count()) !== 1 || (await active.locator(".tab").count()) !== 0) throw new Error("the active editor group is not the empty one on the right");
  await page.mouse.move(200, 20);
  const name = left(page).locator(".view-line", { hasText: `def ${FN}(` }).locator("span", { hasText: new RegExp(`^${FN}$`) }).first();
  return { name, nameBox: await name.boundingBox() };
}

/** Waits for a function row to say `text` (its accessible name starts with it). */
async function rowSays(stage, name, text, timeout = 15_000) {
  const row = rowNamed(stage, name);
  const start = performance.now();
  while (performance.now() - start < timeout) {
    if (((await row.getAttribute("aria-label").catch(() => null)) ?? "").includes(text)) return;
    await stage.page.waitForTimeout(60);
  }
  throw new Error(`the row of ${name} never said "${text}"`);
}

export async function run(stage, { debug }) {
  const { page, cursor } = stage;
  const { name, nameBox } = await setup(stage);
  const rec = new Recording(page, id);
  cursor.attach(rec);
  const beat = async (label) => {
    if (process.env.MEDIA_TIMES) console.log(`  ${rec.elapsed.toFixed(1)} s  ${label}`);
    if (debug) await stage.shot(`${debug}/${id}-${label}.png`, { cursor: true });
  };
  const row = rowNamed(stage, FN);

  await rec.start(); // returns when the capture loop is warm
  try {
    await rec.hold(FIRST, "first"); // the code, and the row with no doc on it
    await beat("0-start");

    // Add Doc… on the function's row: the title box says Overview
    await cursor.moveTo(row);
    const plus = row.locator('[aria-label="Add Doc…"]');
    await plus.waitFor({ state: "visible", timeout: 15_000 });
    await cursor.click(plus, { box: await cursor.aim(plus) }); // the button is at the edge of the side bar: the pointer's own click, not Playwright's hit-target check
    const input = page.locator(".quick-input-widget input.input");
    await rec.during("reveal", "title box", () => input.waitFor({ state: "visible", timeout: 15_000 }));
    await beat("1-title-box");
    await rec.hold(AFTER, "after");
    await cursor.click(input); // into the box, then Enter: the title stays Overview
    await page.keyboard.press("Enter");

    // the core writes the ID line, V-NOC saves the file, the doc opens on the right
    const idLine = left(page).locator(".view-line", { hasText: /ID:\s*[0-9A-Z]{26}/ }).first();
    await rec.during("wait", "Add Doc", async () => {
      await idLine.waitFor({ timeout: 15_000 });
      await page.locator(".tab .label-name", { hasText: "overview.md" }).waitFor({ timeout: 15_000 });
      // the status bar says "Activating Extensions…" while the doc's editor starts, and it comes about half a second after the tab: wait until it has been gone for 1.5 s (a wait is cut by the edit, so this costs nothing)
      let quietSince = performance.now();
      while (performance.now() - quietSince < 1500) {
        if ((await page.locator(".statusbar", { hasText: /Activating Extensions/ }).count()) > 0) quietSince = performance.now();
        await page.waitForTimeout(100);
      }
    });
    await rec.during("reveal", "id-line", () => page.waitForTimeout(400)); // the frame where the ID: line appears: never cut
    await beat("2-id-line");
    await cursor.moveTo(idLine); // the cursor goes to the line that was just written (and the real pointer leaves the row: its hover buttons would hide the doc count)
    await rec.hold(READ, "read");

    // one line in the doc, typed (not pasted), and saved
    await rec.during("typing", "the doc's line", () => page.keyboard.type(LINE, { delay: TYPING_MS }));
    await rec.hold(300, "after");
    await page.keyboard.press("Meta+s");
    await rec.during("wait", "index", () => rowSays(stage, FN, "1 doc"));
    await rec.during("reveal", "1 doc", () => page.waitForTimeout(300));
    await beat("3-one-doc");
    await rec.hold(AFTER, "after");

    // rename the function in the code: double-click the name (it selects the word), type the new one, save
    await cursor.dblclick(name, { box: nameBox });
    await rec.during("typing", "the new name", () => page.keyboard.type(RENAMED, { delay: TYPING_MS }));
    await rec.hold(300, "after");
    await page.keyboard.press("Meta+s");

    // the row follows the rename and still has its doc; the doc is still open on the right
    await rec.during("wait", "index", () => rowSays(stage, RENAMED, "1 doc"));
    await rec.during("reveal", "renamed", () => page.waitForTimeout(300));
    await beat("4-renamed");
    await rec.hold(READ, "read");

    // the cursor leaves the code: the end hold
    await cursor.moveToPoint(1000, 640);
    await beat("5-end");
    await rec.hold(END, "end");
  } catch (error) {
    await rec.stop().catch(() => {});
    throw error;
  }
  return { rec, stats: await rec.stop() };
}
