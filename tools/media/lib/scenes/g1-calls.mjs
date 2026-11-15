// G1 calls.gif: see what a function calls, follow a call to its own file (docs/18-release-0.2/01-media.md §5.1, §5.2, §7.2).
// Setup, off camera: the sidebar narrowed, the canvas focused on `Client` (a focused function has no chevron and opens at once; as one of Client's methods
// `_send_handling_redirects` is a closed card with a chevron, which is the scene), zoom 0.83 and the position set so that three columns fit, the scene run once so
// the host has every answer ready, and everything put back (§5.4). Then, with the mouse only: the chevron of the closed card, the chevron of `_send_single_request`,
// a double-click on a function in another file (Explore), its docs. No drag while recording. The recording is not timed exactly: it marks what happens
// (`rec.during`, `rec.hold`, the cursor's glides) and `edit.mjs` cuts the waits and speeds up the slow parts afterwards.
import { Recording } from "../record.mjs";

export const id = "g1";
export const kind = "gif";
export const output = "calls.gif";
export const target = 12; // seconds, the budget of the edit (§5.1 says 13; 10 – 12 was asked for)

const FN = "_send_handling_redirects";
const OPEN = "_send_single_request";
const CALLEE = "handle_request";

/** the card of the method `FN`, a node card of the canvas focused on Client (labels.ts: `function <name>, collapsed`) */
const nodeCard = (cv) => cv.locator(`[aria-label^="function ${FN},"]`).first();
/** the call card of `name`, by its accessible name (labels.ts: `call to function <name>, called …`) */
const callCard = (cv, name) => cv.locator(`[aria-label^="call to function ${name},"]`).first();
/** `handle_request` is called on the base class (an abstract method in base.py) and on the real transport: the second one has the doc */
const calleeCard = (cv) => cv.locator(`[aria-label^="call to function ${CALLEE},"]`).filter({ hasText: "default.py" }).first();

/**
 * The window is still working: a spinner, `updating…`, a progress `role=status`, the progress badge on the V-NOC icon in the Activity Bar, or "Activating
 * Extensions…" in the status bar (shown while the docs page starts V-NOC's own code). It must be quiet for `quietMs` in a row: that text comes late.
 */
async function settled(page, cv, quietMs = 0) {
  let quietSince = performance.now();
  for (let i = 0; i < 200; i++) {
    const busy =
      (cv ? (await cv.locator("[role=status]").count()) + (await cv.getByText(/updating…/).count()) : 0) +
      (await page.locator(".activitybar .progress-badge").count()) +
      (await page.locator(".statusbar", { hasText: /Activating Extensions/ }).count());
    if (busy > 0) quietSince = performance.now();
    else if (performance.now() - quietSince >= quietMs) return;
    await page.waitForTimeout(100);
  }
}

/** The canvas's zoom (the scale of React Flow's viewport). */
const zoomOf = (cv) => cv.evaluate(() => Number(/matrix\(([-\d.e]+)/.exec(getComputedStyle(document.querySelector(".react-flow__viewport")).transform)?.[1] ?? 1));

/**
 * A point of the canvas background, in page coordinates: where a hand takes hold of the canvas (`bias`, `yBias`: the part of the pane to prefer), or, with
 * `far`, the point with the most room around it (where the cursor rests at the end).
 */
async function emptySpot(cv, bias, yBias = 0.5, far = false) {
  const inFrame = await cv.evaluate(({ bias, yBias, far }) => {
    const pane = document.querySelector(".react-flow").getBoundingClientRect();
    const cards = [...document.querySelectorAll(".react-flow__node")].map((n) => n.getBoundingClientRect());
    const room = (x, y) => Math.min(...cards.map((r) => Math.hypot(Math.max(r.left - x, 0, x - r.right), Math.max(r.top - y, 0, y - r.bottom))), 1e6);
    let best;
    for (let x = pane.left + 80; x < pane.right - 40; x += 40) {
      for (let y = pane.top + 40; y < pane.bottom - 40; y += 30) {
        if (document.elementFromPoint(x, y)?.classList.contains("react-flow__pane")) {
          const d = far ? -room(x, y) : Math.hypot(x - (pane.left + pane.width * bias), y - (pane.top + pane.height * yBias));
          if (!best || d < best.d) best = { x, y, d };
        }
      }
    }
    return best;
  }, { bias, yBias, far });
  if (!inFrame) throw new Error("no empty spot on the canvas to take hold of");
  const frame = await (await cv.frameElement()).boundingBox();
  return [frame.x + inFrame.x, frame.y + inFrame.y];
}

const clamp = (v, lo, hi) => Math.max(lo, Math.min(hi, v));

/**
 * Moves the canvas so that `card` is at (`left`, `top`) from the pane's top left. Only in setup (and in a cut): the camera is never moved while the film runs.
 * One drag moves what fits in the pane, so it is repeated, measuring the card each time.
 */
async function place(stage, cv, card, left, top) {
  const pane = await (await cv.frameElement()).boundingBox();
  for (let round = 0; round < 5; round++) {
    const box = await card.boundingBox();
    const dx = pane.x + left - box.x;
    const dy = pane.y + top - box.y;
    if (Math.abs(dx) < 8 && Math.abs(dy) < 8) return;
    const from = await emptySpot(cv, dx < 0 ? 0.8 : 0.2, dy < 0 ? 0.85 : 0.15);
    const to = [clamp(from[0] + dx, pane.x + 40, pane.x + pane.width - 40), clamp(from[1] + dy, pane.y + 40, pane.y + pane.height - 40)];
    await stage.cursor.drag(from, to, 400);
    await stage.page.waitForTimeout(150);
  }
  throw new Error("the canvas could not be moved to put the card where the scene wants it");
}

/** The card is fully inside the canvas pane. */
async function inView(cv, card) {
  const pane = await (await cv.frameElement()).boundingBox();
  const box = await card.boundingBox();
  return !!box && box.x >= pane.x && box.y >= pane.y && box.x + box.width <= pane.x + pane.width && box.y + box.height <= pane.y + pane.height;
}

/** Closes every EXPLORE context: the chip's `×` in the canvas bar (the panel in the sidebar goes with it). */
async function closeContexts(page, cv) {
  for (let i = 0; i < 6; i++) {
    const chip = cv.getByRole("button", { name: /^Close / });
    if ((await chip.count()) === 0) return;
    await chip.first().click();
    await page.waitForTimeout(450);
  }
  throw new Error("an EXPLORE context would not close");
}

const PANE_LEFT = 30;
const PANE_TOP = 110; // room above for the card before it: the canvas is seen to continue

/** Everything before the first frame (§5.1, §5.2, §5.4). Returns the canvas page. */
async function setup(stage) {
  const { page } = stage;
  await stage.resizeSidebar(260);
  await stage.openRow("folder httpx,");
  await stage.openRow("file _client.py,");
  await stage.openRow("class Client,");
  const row = stage.row("class Client,");
  await row.hover();
  await row.locator('[aria-label="Focus"]').click();
  await page.locator('[aria-label="Open Canvas"]').first().click();
  await page.mouse.move(200, 20); // off the button: its tooltip stays while the pointer is inside a page
  const cv = await stage.canvasFrame();
  await nodeCard(cv).waitFor({ timeout: 15_000 });
  await settled(page, cv, 500);

  // the camera: zoom 0.83 (the card revealed at most 1.2, then two steps of the `−` button), the card at the top left, three columns across the pane
  await nodeCard(cv).locator(".node-header").first().click();
  await cv.getByRole("button", { name: "Reveal the selection" }).click();
  await page.waitForTimeout(700);
  for (let i = 0; i < 6 && (await zoomOf(cv)) > 0.86; i++) {
    await cv.locator(".react-flow__controls-zoomout").click();
    await page.waitForTimeout(450);
  }
  const zoom = await zoomOf(cv);
  if (zoom < 0.78 || zoom > 0.86) throw new Error(`the zoom is ${zoom.toFixed(2)}, not 0.8 – 0.85`);
  await place(stage, cv, nodeCard(cv), PANE_LEFT, PANE_TOP);

  // the warm-up: the whole scene once, so the host has every answer ready. The third column must fit, or the scene has to change.
  await nodeCard(cv).getByRole("button", { name: `Expand ${FN}` }).click();
  await callCard(cv, OPEN).waitFor({ timeout: 15_000 });
  await callCard(cv, OPEN).getByRole("button", { name: `Expand ${OPEN}` }).click();
  await calleeCard(cv).waitFor({ timeout: 15_000 });
  if (!(await inView(cv, calleeCard(cv)))) throw new Error(`the third column does not fit: ${CALLEE} (default.py) is not fully in view at zoom ${zoom.toFixed(2)}`);
  await calleeCard(cv).locator(".node-header").first().dblclick();
  await cv.getByRole("button", { name: "Docs of the selected node" }).click();
  await cv.getByText("Where httpx meets httpcore").first().waitFor({ timeout: 15_000 });
  await settled(page, cv, 800);

  // …and put back (§5.4): the docs panel (the same button), the EXPLORE context, the open cards, and every context the clicks above opened
  await cv.getByRole("button", { name: "Docs of the selected node" }).click();
  await page.waitForTimeout(300);
  await closeContexts(page, cv);
  await callCard(cv, OPEN).getByRole("button", { name: `Collapse ${OPEN}` }).click(); // selecting a call card opens its context again: closed next
  await page.waitForTimeout(400);
  await closeContexts(page, cv);
  await nodeCard(cv).getByRole("button", { name: `Collapse ${FN}` }).click();
  await page.waitForTimeout(400);
  await closeContexts(page, cv);
  await place(stage, cv, nodeCard(cv), PANE_LEFT, PANE_TOP); // a no-op unless something moved it
  const rest = await emptySpot(cv, 0, 0, true);
  await page.mouse.click(rest[0], rest[1]); // no button keeps a focus highlight
  await page.waitForTimeout(300);
  await stage.command("V-NOC: Clear Selection"); // the session owns the selection: a click on the canvas does not clear it
  await page.waitForTimeout(300);
  const stale = await cv.locator('[aria-label*=", selected"]').count();
  if (stale > 0) throw new Error(`${stale} card(s) are still selected at the first frame`);
  const label = await nodeCard(cv).getAttribute("aria-label");
  if (!label?.endsWith("collapsed")) throw new Error(`the card of ${FN} is not closed at the first frame: "${label}"`);
  await page.mouse.move(200, 20);
  await settled(page, cv, 500);
  return cv;
}

// Holds in ms (§5.1, §5.2): 0.5 s on the first frame, 1.0 s after each reveal, 1.2 s after Explore, 0.6 s after an action, 2.5 s on the last frame. The recording
// does not try to keep the film from standing still around them: the edit cuts the waits and caps what the marks miss.
const FIRST = 500;
const READ = 1000;
const EXPLORED = 1200;
const AFTER = 600;
const END = 2500;

export async function run(stage, { debug }) {
  const { page, cursor } = stage;
  const cv = await setup(stage);
  const rec = new Recording(page, id);
  cursor.attach(rec);
  const beat = async (name) => {
    if (process.env.MEDIA_TIMES) console.log(`  ${rec.elapsed.toFixed(1)} s  ${name}`);
    if (debug) await stage.shot(`${debug}/${id}-${name}.png`, { cursor: true });
  };

  await rec.start(); // returns when the capture loop is warm
  try {
    await rec.hold(FIRST, "first"); // the closed card of `_send_handling_redirects`, the sidebar showing MAIN
    await beat("0-start");

    // its chevron: the calls it makes fan out with dashed edges
    await cursor.click(nodeCard(cv).getByRole("button", { name: `Expand ${FN}` }));
    await rec.during("reveal", "calls", () => callCard(cv, OPEN).waitFor({ timeout: 10_000 }));
    await beat("1-calls");
    await rec.hold(READ, "read");

    // open `_send_single_request`: its calls appear in a third column, `handle_request · default.py` among them
    await cursor.click(callCard(cv, OPEN).getByRole("button", { name: `Expand ${OPEN}` }));
    await rec.during("reveal", "third column", () => calleeCard(cv).waitFor({ timeout: 10_000 }));
    await beat("2-third-column");
    await rec.hold(READ, "read");

    // Explore the function in another file: the EXPLORE panel opens in the sidebar, a chip in the bar, the breadcrumb changes
    await cursor.dblclick(calleeCard(cv).locator(".node-header").first());
    await rec.during("wait", "Explore", async () => {
      // the new context is framed by the product with a zoom past 100 %: bring it to a normal size and to the left, so the film shows it settled
      await cv.getByRole("button", { name: /^Close / }).first().waitFor({ timeout: 10_000 });
      await settled(page, cv, 300);
      for (let i = 0; i < 8 && (await zoomOf(cv)) > 1.0; i++) {
        await cv.locator(".react-flow__controls-zoomout").click();
        await page.waitForTimeout(450);
      }
      await place(stage, cv, cv.locator(`[aria-label^="function ${CALLEE},"]`).first(), PANE_LEFT, 80);
      await page.mouse.move(200, 20);
      await settled(page, cv, 300);
    });
    await rec.during("reveal", "Explore", () => page.waitForTimeout(300));
    await beat("3-explored");
    await rec.hold(EXPLORED, "read");

    // its docs: the panel opens with the doc written for this function
    await cursor.click(cv.getByRole("button", { name: "Docs of the selected node" }));
    await rec.during("reveal", "docs", () => cv.getByText("Where httpx meets httpcore").first().waitFor({ timeout: 10_000 }));
    await beat("4-docs");
    await rec.hold(AFTER, "after");

    // the cursor leaves the card, to the empty canvas; the end hold
    const spot = await emptySpot(cv, 0, 0, true);
    await cursor.moveToPoint(spot[0], spot[1]);
    await beat("5-end");
    await rec.hold(END, "end");
  } catch (error) {
    await rec.stop().catch(() => {});
    throw error;
  }
  return { rec, stats: await rec.stop() };
}
