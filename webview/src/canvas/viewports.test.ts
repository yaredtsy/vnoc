import assert from "node:assert/strict";
import test from "node:test";

import { type Viewports, forgetClosed, moved, viewportFor } from "./viewports.ts";

const none: Viewports = {};
const a = { x: 10, y: 20, zoom: 0.5 };
const b = { x: -5, y: 0, zoom: 1.2 };

test("a context seen for the first time has no viewport: the canvas fits its view", () => {
  assert.equal(viewportFor(none, "main", "0:app"), null);
});

test("leaving a context saved its viewport, and coming back restores it; the other context's is its own", () => {
  let held = moved(none, "main", "0:app", a);
  held = moved(held, "explore:x", "0:reserve", b);
  assert.deepEqual(viewportFor(held, "main", "0:app"), a);
  assert.deepEqual(viewportFor(held, "explore:x", "0:reserve"), b);
});

test("panning again replaces it; the same viewport is the same object", () => {
  const held = moved(none, "main", "0:app", a);
  assert.equal(moved(held, "main", "0:app", { ...a }), held);
  assert.deepEqual(viewportFor(moved(held, "main", "0:app", b), "main", "0:app"), b);
});

test("a context that was refocused, or whose structure was read again, is fitted: its old pan means another drawing", () => {
  const held = moved(none, "main", "0:app", a);
  assert.equal(viewportFor(held, "main", "0:other"), null, "another root");
  assert.equal(viewportFor(held, "main", "1:app"), null, "another load");
});

test("a closed context's viewport is forgotten; one that is open is kept; nothing to forget is the same object", () => {
  const held = moved(moved(none, "main", "0:app", a), "explore:x", "0:reserve", b);
  const after = forgetClosed(held, ["main"]);
  assert.equal(viewportFor(after, "explore:x", "0:reserve"), null);
  assert.deepEqual(viewportFor(after, "main", "0:app"), a);
  assert.equal(forgetClosed(after, ["main", "explore:x"]), after);
  // opened again later: it is new, so it fits
  assert.equal(viewportFor(forgetClosed(held, ["main"]), "explore:x", "0:reserve"), null);
});
