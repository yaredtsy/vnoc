// G1 calls.gif: "What does httpx.get() do?" (docs/18-release-0.2/04-gifs-v2.md §4, as rewritten after gate 1). It starts in the code, which a viewer knows: `get` in
// `_api.py`. Right-click → Show Calls opens the canvas beside it; the canvas is maximised; the chevron of `request` opens one level; F fits the view (the four cards,
// larger); `request` in `_client.py` is explored with Enter. The film ends on Explore, the largest and clearest picture.
//
// The scene records freely (§6.1): it waits for state and holds 1.5 s after every change, and never thinks about length. Every beat begins with `rec.beat(name)`,
// which is a named mark in a film and a picture in `--stills`. Speed is the edit's job, afterwards.
//
// Setup, off camera: the window 960 × 600, the Activity Bar and the side bar closed, Focus on `get`, `_api.py` open with `def get(` a third of the way down, nothing
// selected or highlighted in the code, no canvas, the arrow at rest right of the signature, on no text. No warm-up and no put-back: a slow first fetch is cut in the
// edit. Only the mouse for what a new user clicks (a menu, a chevron, a card) and keys for what a user does with keys, each with its badge (§3.6): ⌘K ⌘M, F, Enter.
import { emptySpot, settled, viewOf, viewSettled, zoomOf } from "../canvas.mjs";
import { recordingFor } from "../stills.mjs";

export const id = "g1";
export const kind = "gif";
export const output = "calls.gif";
export const stills = true; // `--stills` works for this scene
// the Activity Bar is closed: G1 does not need it, and the sidebar's change when a card is opened (04 §1.1 #4) is then out of the picture. Nothing in the code is
// highlighted: not the other places a word occurs, not the word that is selected, not the bracket the cursor is next to (the line's own box stays: it points at `get`)
export const settings = { "workbench.activityBar.location": "hidden", "editor.occurrencesHighlight": "off", "editor.selectionHighlight": false, "editor.matchBrackets": "never" };

const HOLD = 1500; // after every change (§6.1); the edit trims it
const END = 3000;

/** the node card of a function, by its accessible name (labels.ts: `function <name>, …`) */
const nodeCard = (cv, name) => cv.locator(`[aria-label^="function ${name},"]`).first();
/** the call card of a function (labels.ts: `call to function <name>, called …`); `file` tells apart two calls of the same name (`request` is in `_api.py` and in `_client.py`) */
const callCard = (cv, name, file) => {
  const card = cv.locator(`[aria-label^="call to function ${name},"]`);
  return (file ? card.filter({ hasText: file }) : card).first();
};

/** Everything before the first frame. Returns what the beats need. */
async function setup(stage) {
  const { page, cursor } = stage;
  await stage.openRow("folder httpx,");
  await stage.openRow("file _api.py,");
  const row = stage.row("function get,");
  await row.click(); // the canvas is closed: the code, at the function
  // Focus on `get` before the film (04 §4 [check]): without it the canvas of beat 2 is the whole project, a wall of cards with `get` and `request` at their right edge
  await row.hover();
  await row.locator('[aria-label="Focus"]').click();
  const line = page.locator(".editor-group-container .view-line", { hasText: "def get(" }).first();
  await line.waitFor({ timeout: 15_000 });
  await stage.command("View: Toggle Primary Side Bar Visibility");
  await page.locator(".part.sidebar").waitFor({ state: "hidden", timeout: 5000 });
  await page.mouse.move(200, 20);

  // the name of the function, a third of the way down the window: the reveal puts it in the middle, so the editor is scrolled (setup only)
  const name = page.locator(".view-line", { hasText: "def get(" }).locator("span", { hasText: /^get$/ }).first();
  let box = await cursor.aim(name);
  const wanted = 200; // px from the window's top: the tabs and the breadcrumbs above it take about 90
  for (let round = 0; round < 6 && Math.abs(box.y - wanted) > 15; round++) {
    await page.mouse.move(box.x + 300, box.y);
    await page.mouse.wheel(0, box.y - wanted);
    await page.mouse.move(200, 20);
    box = await cursor.aim(name);
  }
  if (Math.abs(box.y - wanted) > 40) throw new Error(`the name of get is ${Math.round(box.y)} px from the top, not about ${wanted}`);

  // nothing selected: the reveal leaves `def` highlighted. A click on the empty part of the signature's line puts the cursor there and clears the selection, and
  // the arrow rests on the same spot: right of the signature, on no text
  const rest = [box.x + 260, box.y + box.height / 2];
  await page.mouse.click(rest[0], rest[1]);
  await page.mouse.move(200, 20);

  // the first frame: a caption, and the arrow at rest
  await stage.caption("What does httpx.get() do?");
  await cursor.moveToPoint(rest[0], rest[1]);
  return { name };
}

export async function run(stage, { debug, stills: onlyStills }) {
  const { page, cursor } = stage;
  const { name } = await setup(stage);
  const rec = recordingFor(stage, id, { stills: onlyStills });
  cursor.attach(rec);
  const shot = async (label) => debug && stage.shot(`${debug}/${id}-${label}.png`, { cursor: true });

  await rec.start(); // returns when the capture loop is warm
  let cv;
  try {
    // 1 · the code of `get`, and the question
    await rec.beat("start");
    await rec.hold(HOLD, "first");

    // 2 · right-click `get` → V-NOC ▸ Show Calls: the canvas opens beside the code, `get` open with its one call, `request`
    await rec.beat("show-calls");
    await stage.caption("Right-click → Show Calls");
    await cursor.click(name, { button: "right" });
    await cursor.click(page.getByRole("menuitem", { name: "V-NOC" }));
    await cursor.click(page.getByRole("menuitem", { name: "Show Calls" }));
    await rec.during("wait", "Show Calls", async () => {
      cv = await stage.canvasFrame();
      await callCard(cv, "request", "_api.py").waitFor({ timeout: 30_000 });
      await settled(page, cv, 500);
    });
    await rec.during("reveal", "calls", () => nodeCard(cv, "get").waitFor({ timeout: 10_000 }));
    await shot("show-calls");
    await rec.hold(HOLD);

    // 3 · maximise the canvas: ⌘K ⌘M (Toggle Maximize Editor Group, the default binding in 1.146), the canvas takes the window
    await rec.beat("maximise");
    await stage.caption("Maximise the canvas");
    await stage.key("Meta+k Meta+m");
    await rec.during("wait", "maximise", async () => {
      for (let i = 0; i < 100 && (await cv.evaluate(() => innerWidth)) < 850; i++) await page.waitForTimeout(100);
      await settled(page, cv, 300);
    });
    await shot("maximise");
    await rec.hold(HOLD);

    // 4 · the chevron of `request`: `Client` and `request` in `_client.py`
    await rec.beat("open-request");
    await stage.caption("Open a call: one level deeper");
    await cursor.click(callCard(cv, "request", "_api.py").getByRole("button", { name: "Expand request" }));
    await rec.during("reveal", "one level deeper", () => callCard(cv, "request", "_client.py").waitFor({ timeout: 15_000 }));
    await settled(page, cv, 300);
    await shot("open-request");
    await rec.hold(HOLD);

    // 5 · F fits the view: the four cards centred, and larger (the product's own 300 ms glide). The chevron just clicked has the focus, and F is not a button's key
    // (04 §8 P5). The size of the cards before and after is measured and written under the picture: 04 §4 says drop this beat if they are not larger
    await rec.beat("fit");
    await stage.caption("F: fit the view");
    const request = callCard(cv, "request", "_client.py");
    const before = { view: await viewOf(cv), width: (await request.boundingBox()).width };
    await stage.key("f");
    await rec.during("wait", "Fit View", async () => {
      for (let i = 0; i < 60 && (await viewOf(cv)).every((v, k) => Math.abs(v - before.view[k]) < 0.01); i++) await page.waitForTimeout(50);
      await viewSettled(page, cv);
    });
    const after = { view: await viewOf(cv), width: (await request.boundingBox()).width };
    const sizes = `Fit View: zoom ${(before.view[2] * 100).toFixed(0)} % → ${(after.view[2] * 100).toFixed(0)} %; a card is ${Math.round(before.width)} → ${Math.round(after.width)} px wide`;
    rec.note(sizes);
    console.log(`  ${sizes}`);
    if (after.view.every((v, k) => Math.abs(v - before.view[k]) < 0.01)) throw new Error("F did not change the view: Fit View did not run (04 §8 P5: a chevron has the focus)");
    await shot("fit");
    await rec.hold(HOLD);

    // 6 · click `request` in `_client.py`, then Enter (Explore): the canvas follows the call to its file
    await rec.beat("explore");
    await stage.caption("Enter: follow it to its file");
    await cursor.click(request.locator(".node-header").first());
    await stage.key("Enter");
    await rec.during("wait", "Explore", async () => {
      await callCard(cv, "send").waitFor({ timeout: 15_000 });
      await settled(page, cv, 300);
      await viewSettled(page, cv);
    });
    // 04 §8 P1: a new context opens at 100 % at most
    const zoom = await zoomOf(cv);
    rec.note(`zoom after Explore: ${(zoom * 100).toFixed(0)} %`);
    console.log(`  zoom after Explore: ${(zoom * 100).toFixed(0)} %`);
    if (zoom > 1.001) throw new Error(`a new EXPLORE context opened at ${(zoom * 100).toFixed(0)} %, not at 100 % at most (04 §8 P1): is the VSIX built from the commit "canvas: a new context never opens zoomed past 100 %"?`);
    await shot("explore");
    await rec.hold(HOLD);

    // 7 · the end: the arrow on empty canvas, 3 s. The caption answers the first one, and it is what the picture shows: `request` builds a request and `send` sends it
    await rec.beat("end");
    await stage.caption("It builds a request and sends it");
    const far = await emptySpot(cv, 0, 0, true);
    await cursor.moveToPoint(far[0], far[1]);
    await shot("end");
    await rec.hold(END, "end");
  } catch (error) {
    await rec.stop().catch(() => {});
    throw error;
  }
  return { rec, stats: await rec.stop() };
}
