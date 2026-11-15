// G1 calls.gif: see what a function calls, follow one to where it lives (docs/18-release-0.2/01-media.md §5).
// Only the mouse is used while recording, as a person would: the sidebar's Focus and Open Canvas buttons, then the chevrons of the call cards down to a
// function in another file (the transport), a double-click to Explore it, and its docs. It starts focused on `send` (the plan started at `Client`, whose 20
// methods are too small to read on a canvas). The setup before the first frame only opens the tree down to `Client`.
import { Recording } from "../record.mjs";

export const id = "g1";
export const kind = "gif";
export const output = "calls.gif";

const CHAIN = ["_send_handling_auth", "_send_handling_redirects", "_send_single_request"];
const CALLEE = "handle_request";
// `handle_request` is called on the base class (an abstract method in base.py) and on the real transport: the second one has the doc
const calleeCard = (cv) => cv.locator(`[aria-label^="call to function ${CALLEE},"]`).filter({ hasText: "default.py" }).first();

/** the call card of `name` in the canvas page, by its accessible name (labels.ts: `call to function <name>, called …`) */
const callCard = (cv, name) => cv.locator(`[aria-label^="call to function ${name},"]`).first();

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

/**
 * A point of the canvas background (the pane, not a card, an edge or a control), in page coordinates: where a hand can take hold of the canvas to move it.
 * `bias` and `yBias` (0 = left or top, 1 = right or bottom) are the part of the pane to prefer, so a drag has room to go the other way.
 */
async function emptySpot(cv, bias, yBias = 0.5) {
  const inFrame = await cv.evaluate(({ bias, yBias }) => {
    const pane = document.querySelector(".react-flow").getBoundingClientRect();
    let best;
    for (let x = pane.left + 80; x < pane.right - 40; x += 40) {
      for (let y = pane.top + 40; y < pane.bottom - 40; y += 30) {
        if (document.elementFromPoint(x, y)?.classList.contains("react-flow__pane")) {
          const d = Math.hypot(x - (pane.left + pane.width * bias), y - (pane.top + pane.height * yBias));
          if (!best || d < best.d) best = { x, y, d };
        }
      }
    }
    return best;
  }, { bias, yBias });
  if (!inFrame) throw new Error("no empty spot on the canvas to take hold of");
  const frame = await (await cv.frameElement()).boundingBox();
  return [frame.x + inFrame.x, frame.y + inFrame.y];
}

const clamp = (v, lo, hi) => Math.max(lo, Math.min(hi, v));

/**
 * Drags the canvas so that `card` ends up at the top left, with room to the right for what it opens (the camera does not follow an expansion). One drag
 * moves the canvas at most what fits in the pane, so it is repeated, measuring the card each time, until the card is there.
 */
async function bringToLeft(stage, cv, card) {
  const pane = await (await cv.frameElement()).boundingBox();
  for (let round = 0; round < 4; round++) {
    const box = await card.boundingBox();
    const dx = pane.x + 50 - box.x;
    const dy = pane.y + 110 - box.y;
    if (Math.abs(dx) < 60 && Math.abs(dy) < 60) return;
    const from = await emptySpot(cv, dx < 0 ? 0.8 : 0.2, dy < 0 ? 0.85 : 0.15);
    const to = [clamp(from[0] + dx, pane.x + 40, pane.x + pane.width - 40), clamp(from[1] + dy, pane.y + 40, pane.y + pane.height - 40)];
    await stage.cursor.drag(from, to, 600);
    await stage.page.waitForTimeout(200);
  }
  throw new Error("the canvas could not be moved to bring the card to the left");
}

export async function run(stage, { debug }) {
  const { page, cursor } = stage;

  // before the first frame: the tree open down to the methods of `Client`, the way a person has left it
  await stage.openRow("folder httpx,");
  await stage.openRow("file _client.py,");
  await stage.openRow("class Client,");
  await stage.row("function send,").waitFor();
  await page.mouse.move(900, 500);
  await settled(page);

  const rec = new Recording(page, id);
  let cv;
  const beat = async (name) => {
    await settled(page, cv);
    if (process.env.MEDIA_TIMES) console.log(`  ${rec.elapsed.toFixed(1)} s  ${name}`);
    if (debug) await stage.shot(`${debug}/${id}-${name}.png`, { cursor: true });
  };

  rec.start();
  try {
    await rec.hold(700);
    await beat("0-sidebar");

    // narrow down to `send`, and open the canvas
    const send = stage.row("function send,");
    const focus = send.locator('[aria-label="Focus"]');
    await cursor.moveTo(send); // a row's buttons appear on hover
    await focus.waitFor({ state: "visible", timeout: 5000 });
    await cursor.click(focus);
    await rec.hold(350);
    await cursor.click(page.locator('[aria-label="Open Canvas"]').first());
    await page.mouse.move(200, 20); // off the button (its tooltip stays while the pointer is inside a page), on a part of the title bar with no tooltip of its own
    rec.pause(); // the page takes a moment to start: a cut, not two seconds of an empty canvas
    cv = await stage.canvasFrame();
    await callCard(cv, CHAIN[0]).waitFor({ timeout: 15_000 });
    await settled(page, cv, 500);
    rec.resume();
    await beat("1-canvas");
    await rec.hold(800);

    // follow the chain: a chevron shows what that function calls, one level to the right; the canvas is dragged so the next card is at the left
    for (const [i, name] of CHAIN.entries()) {
      const chevron = callCard(cv, name).getByRole("button", { name: `Expand ${name}` });
      for (let attempt = 0; ; attempt++) {
        await cursor.click(chevron);
        if (await cv.getByRole("button", { name: `Collapse ${name}` }).first().waitFor({ timeout: 2500 }).then(() => true, () => false)) break;
        if (attempt === 2) throw new Error(`the chevron of ${name} did not open it`);
      }
      if (process.env.MEDIA_TIMES) console.log(`    ${rec.elapsed.toFixed(1)} s  ${name}: chevron clicked`);
      const next = i + 1 < CHAIN.length ? callCard(cv, CHAIN[i + 1]) : calleeCard(cv);
      await next.waitFor({ timeout: 15_000 });
      await settled(page, cv);
      if (process.env.MEDIA_TIMES) console.log(`    ${rec.elapsed.toFixed(1)} s  ${name}: next card is there and settled`);
      await bringToLeft(stage, cv, next);
      await beat(`${i + 2}-${name}`);
      await rec.hold(300);
    }

    // Explore the callee in another file: a double-click opens its own context (a panel in the sidebar, a chip in the bar) and the canvas shows its file
    await cursor.dblclick(calleeCard(cv).locator(".node-header").first());
    await page.waitForTimeout(800);
    await beat("5-explored");
    await rec.hold(1100);

    // its docs
    await cursor.click(cv.getByRole("button", { name: "Docs of the selected node" }));
    rec.pause();
    await cv.getByText("Where httpx meets httpcore").first().waitFor({ timeout: 10_000 });
    await settled(page, cv, 800);
    rec.resume();
    // the panel took half of the canvas after the card was framed: Fit View frames it again in what is left
    await cursor.click(cv.locator(".react-flow__controls-fitview"));
    await page.waitForTimeout(500);
    await beat("6-docs");
    await rec.hold(2200);
  } catch (error) {
    await rec.stop().catch(() => {});
    throw error;
  }
  return { rec, stats: await rec.stop() };
}
