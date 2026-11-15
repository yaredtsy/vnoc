// What a scene needs to know about the canvas page, and nothing about a particular scene: whether the window is still working, the zoom, where the empty canvas is,
// putting a card where the setup wants it, closing the EXPLORE contexts. `cv` is the canvas's frame (`stage.canvasFrame()`).

/**
 * The window is still working: a spinner, `updating…`, a progress `role=status`, the progress badge on the V-NOC icon in the Activity Bar, or "Activating
 * Extensions…" in the status bar (shown while the docs page starts V-NOC's own code). It must be quiet for `quietMs` in a row: that text comes late.
 */
export async function settled(page, cv, quietMs = 0) {
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

/** The canvas's viewport as `[x, y, zoom]` (React Flow's transform). */
export const viewOf = (cv) =>
  cv.evaluate(() => {
    const m = /matrix\(([-\d.e]+), ([-\d.e]+), ([-\d.e]+), ([-\d.e]+), ([-\d.e]+), ([-\d.e]+)\)/.exec(getComputedStyle(document.querySelector(".react-flow__viewport")).transform);
    return m ? [Number(m[5]), Number(m[6]), Number(m[1])] : [0, 0, 1];
  });

/** Waits until the viewport has not moved for `quietMs` (a fit of the view is a 300 ms glide): state, not a sleep. Returns the viewport it stopped at. */
export async function viewSettled(page, cv, quietMs = 250) {
  let last = await viewOf(cv);
  let quietSince = performance.now();
  for (let i = 0; i < 100; i++) {
    await page.waitForTimeout(50);
    const now = await viewOf(cv);
    if (now.some((v, k) => Math.abs(v - last[k]) > 0.5 / (k === 2 ? 100 : 1))) quietSince = performance.now();
    else if (performance.now() - quietSince >= quietMs) return now;
    last = now;
  }
  return last;
}

/** The canvas's zoom (the scale of React Flow's viewport). */
export const zoomOf = (cv) => cv.evaluate(() => Number(/matrix\(([-\d.e]+)/.exec(getComputedStyle(document.querySelector(".react-flow__viewport")).transform)?.[1] ?? 1));

/**
 * A point of the canvas background, in page coordinates: where a hand takes hold of the canvas (`bias`, `yBias`: the part of the pane to prefer), or, with
 * `far`, the point with the most room around it (where the cursor rests at the end).
 */
export async function emptySpot(cv, bias, yBias = 0.5, far = false) {
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
export async function place(stage, cv, card, left, top) {
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
export async function inView(cv, card) {
  const pane = await (await cv.frameElement()).boundingBox();
  const box = await card.boundingBox();
  return !!box && box.x >= pane.x && box.y >= pane.y && box.x + box.width <= pane.x + pane.width && box.y + box.height <= pane.y + pane.height;
}

/** Closes every EXPLORE context: the chip's `×` in the canvas bar (the panel in the sidebar goes with it). */
export async function closeContexts(page, cv) {
  for (let i = 0; i < 6; i++) {
    const chip = cv.getByRole("button", { name: /^Close / });
    if ((await chip.count()) === 0) return;
    await chip.first().click();
    await page.waitForTimeout(450);
  }
  throw new Error("an EXPLORE context would not close");
}
