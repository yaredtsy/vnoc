import assert from "node:assert/strict";
import { test } from "node:test";

import { KEEP, MAX_SCALE, fitScale, fitView, kept, naturalSize, panBy, wheelFactor, zoomAt } from "./diagramView.ts";

const room = { width: 908, height: 649 };

test("the natural size is the viewBox's third and fourth numbers", () => {
  assert.deepEqual(naturalSize('<svg id="a" viewBox="0 0 566.5 1782" style="max-width: 566px">'), { width: 566.5, height: 1782 });
  assert.equal(naturalSize("<svg>"), null);
  assert.equal(naturalSize('<svg viewBox="0 0 0 10">'), null);
});

test("a diagram 3,000 px tall fits a 650 px overlay whole, with its aspect kept, and is centred", () => {
  const natural = { width: 600, height: 3000 };
  const view = fitView(natural, room);
  assert.ok(natural.height * view.scale <= room.height, `${natural.height * view.scale} px high`);
  assert.ok(natural.width * view.scale <= room.width);
  assert.equal(view.scale, room.height / natural.height, "the height is what limits it");
  // centred: the same margin on both sides
  assert.ok(Math.abs(view.x - (room.width - natural.width * view.scale) / 2) < 1e-9);
  assert.equal(view.y, 0);
  // the aspect is the one scale for both: nothing is stretched
  assert.equal(fitScale({ width: 600, height: 3000 }, room), view.scale);
});

test("a wide diagram is limited by the width, a small one is shown up to twice its size and not more", () => {
  assert.equal(fitScale({ width: 1816, height: 200 }, room), 0.5);
  assert.equal(fitScale({ width: 100, height: 100 }, room), 2);
  assert.equal(fitScale({ width: 400, height: 300 }, room), 2, "908/400 and 649/300 are both over 2");
  assert.equal(fitScale({ width: 600, height: 500 }, room), room.width / 600 > room.height / 500 ? room.height / 500 : room.width / 600);
});

test("the room is not measured yet: the diagram is not scaled", () => {
  assert.equal(fitScale({ width: 600, height: 3000 }, { width: 0, height: 0 }), 1);
});

test("zooming keeps the point under the pointer where it is", () => {
  const natural = { width: 600, height: 3000 };
  const view = fitView(natural, room);
  const at = { x: 454, y: 300 };
  // the diagram's own point under `at`, before and after
  const under = (v: typeof view) => ({ x: (at.x - v.x) / v.scale, y: (at.y - v.y) / v.scale });
  const zoomed = zoomAt(view, 2, at, natural, room);
  assert.ok(Math.abs(zoomed.scale - view.scale * 2) < 1e-9);
  assert.ok(Math.abs(under(zoomed).x - under(view).x) < 1e-6);
  assert.ok(Math.abs(under(zoomed).y - under(view).y) < 1e-6);
});

test("the scale stays between half the fit and eight times", () => {
  const natural = { width: 600, height: 3000 };
  const view = fitView(natural, room);
  const centre = { x: 454, y: 325 };
  let v = view;
  for (let i = 0; i < 60; i++) v = zoomAt(v, 1.25, centre, natural, room);
  assert.equal(v.scale, MAX_SCALE);
  for (let i = 0; i < 120; i++) v = zoomAt(v, 0.8, centre, natural, room);
  assert.ok(Math.abs(v.scale - fitScale(natural, room) / 2) < 1e-9);
});

test("a pan can never take the diagram out of reach: a strip of it stays in the overlay", () => {
  const natural = { width: 600, height: 3000 };
  const zoomed = zoomAt(fitView(natural, room), 4, { x: 454, y: 325 }, natural, room);
  const far = panBy(zoomed, -1e6, -1e6, natural, room);
  assert.equal(far.x, KEEP - natural.width * zoomed.scale);
  assert.equal(far.y, KEEP - natural.height * zoomed.scale);
  const back = panBy(zoomed, 1e6, 1e6, natural, room);
  assert.equal(back.x, room.width - KEEP);
  assert.equal(back.y, room.height - KEEP);
  // a diagram smaller than the strip is kept whole
  const tiny = kept({ scale: 1, x: -500, y: 900 }, { width: 20, height: 20 }, room);
  assert.deepEqual(tiny, { scale: 1, x: 0, y: 629 });
});

test("a wheel turn of 100 px is one step, the other way is the same step back", () => {
  assert.ok(Math.abs(wheelFactor(100) * wheelFactor(-100) - 1) < 1e-12);
  assert.ok(wheelFactor(-100) > 1 && wheelFactor(100) < 1);
});
