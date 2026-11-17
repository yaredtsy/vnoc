import assert from "node:assert/strict";
import { test } from "node:test";

import { SidebarSlot } from "./sidebarSlot.ts";

test("disposing the current view runs `closed` once, and the slot is empty and has not drawn", async () => {
  let closed = 0;
  const slot = new SidebarSlot<object>(() => closed++);
  const view = {};
  slot.adopt(view);
  slot.rendered(view);
  await slot.whenDrew(10); // drawn: at once
  slot.disposed(view);
  assert.equal(closed, 1);
  assert.equal(slot.view, undefined);
  slot.disposed(view);
  assert.equal(closed, 1, "once");
  // `sidebarDrew` is false again: a command that waits for a page waits (here: until its timeout)
  let waited = false;
  const waiting = slot.whenDrew(30).then(() => (waited = true));
  await new Promise((r) => setTimeout(r, 5));
  assert.equal(waited, false);
  await waiting;
});

test("disposing a view that was replaced changes nothing", () => {
  let closed = 0;
  const slot = new SidebarSlot<object>(() => closed++);
  const [old, current] = [{}, {}];
  slot.adopt(old);
  slot.adopt(current);
  slot.rendered(current);
  slot.disposed(old);
  assert.equal(closed, 0);
  assert.equal(slot.view, current);
  slot.rendered(old); // an old page's render is not the new one's
  slot.disposed(current);
  assert.equal(closed, 1);
});

test("`whenDrew` waits for the current page's render, and an old page's render does not release it", async () => {
  const slot = new SidebarSlot<object>(() => {});
  const [old, current] = [{}, {}];
  slot.adopt(old);
  slot.adopt(current);
  let released = false;
  const waiting = slot.whenDrew(2000).then(() => (released = true));
  slot.rendered(old);
  await new Promise((r) => setTimeout(r, 5));
  assert.equal(released, false);
  slot.rendered(current);
  await waiting;
  assert.equal(released, true);
});
