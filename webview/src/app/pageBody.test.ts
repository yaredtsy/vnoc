import assert from "node:assert/strict";
import { test } from "node:test";

import { type Body, type Status, pageBody } from "./pageBody.ts";

const rows: [Status, boolean, Body][] = [
  ["loading", false, "loading"],
  ["loading", true, "canvas"],
  ["ready", false, "loading"], // ready with nothing to draw is a moment between two states: still waiting
  ["ready", true, "canvas"],
  ["error", false, "error"],
  ["error", true, "canvas"], // a refresh failed over a good load: keep the cards
];

for (const [status, hasLayout, body] of rows) {
  test(`${status}, ${hasLayout ? "with" : "without"} a layout: ${body}`, () => assert.equal(pageBody(status, hasLayout), body));
}
