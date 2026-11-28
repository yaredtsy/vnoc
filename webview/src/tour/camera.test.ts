import assert from "node:assert/strict";
import { test } from "node:test";

import { MARGIN, cameraMove, centre } from "./camera.ts";

const size = { width: 800, height: 600 };
const card = (x: number, y: number) => ({ x, y, width: 300, height: 80 });
const at = (x = 0, y = 0, zoom = 1) => ({ x, y, zoom });

test("a card that is whole in view does not move the map", () => {
  assert.equal(cameraMove(card(100, 100), at(), size), null);
  assert.equal(cameraMove(card(0, 0), at(), size), null, "touching the edge is still in view");
  assert.equal(cameraMove(card(500, 520), at(), size), null, "the last pixel in view");
  assert.equal(cameraMove(card(300, 200), at(-100, -50, 0.5), size), null, "zoom and pan are counted");
});

test("a card that is cut is panned by the least that shows it whole, with a 24 px margin; the zoom is not touched", () => {
  // cut at the right: its right edge goes to 800 - 24
  assert.deepEqual(cameraMove(card(600, 100), at(), size), { x: 800 - MARGIN - 900, y: 0, zoom: 1 });
  // cut at the bottom
  assert.deepEqual(cameraMove(card(100, 560), at(), size), { x: 0, y: 600 - MARGIN - 640, zoom: 1 });
  // cut at the left and the top
  assert.deepEqual(cameraMove(card(-50, -20), at(), size), { x: MARGIN + 50, y: MARGIN + 20, zoom: 1 });
  // whole but inside the margin: it is in view, so nothing moves
  assert.equal(cameraMove(card(10, 10), at(), size), null);
  // the zoom is kept
  assert.equal(cameraMove(card(2000, 100), at(0, 0, 0.5), size)?.zoom, 0.5);
});

test("a card more than a screen away, or too big for the pane, is centred instead", () => {
  const far = cameraMove(card(5000, 4000), at(), size)!;
  assert.deepEqual(far, centre(card(5000, 4000), 1, size));
  assert.equal(far.x + (5000 + 150) * 1, 400, "its middle is the pane's middle");
  assert.equal(far.y + (4000 + 40) * 1, 300);
  // exactly one screen of pan is a pan; more is a centring
  const near = cameraMove(card(1000, 100), at(), size)!;
  assert.deepEqual(near, { x: 800 - MARGIN - 1300, y: 0, zoom: 1 }, "a pan of 524 px is under a screen");
  assert.deepEqual(cameraMove(card(2000, 100), at(), size), centre(card(2000, 100), 1, size), "1524 px is over a screen");
  // a card bigger than the pane (zoomed in) is centred, not panned to an edge
  const tall = { x: 0, y: 0, width: 300, height: 80 };
  assert.deepEqual(cameraMove(tall, at(0, 0, 10), size), centre(tall, 10, size));
});
