import assert from "node:assert/strict";
import { test } from "node:test";

import { CARD_W, COL_W } from "../canvas/layout.ts";
import { CHEVRON, CHIP_PAD, GAP, HEADER_PAD, KIND_ICON, NAME_X } from "./rows.ts";

test("the meta line and the footer start where the name starts: the header's padding, chevron, icon and their gaps", () => {
  assert.equal(NAME_X, HEADER_PAD + CHEVRON + GAP + KIND_ICON + GAP);
  assert.equal(NAME_X, 44);
});

test("a footer chip's text is under the name: the footer starts one chip padding earlier", () => {
  assert.equal(NAME_X - CHIP_PAD, 38);
});

test("a card is 300 px wide (names get about 160 px) and a column leaves 40 px for the edge", () => {
  assert.equal(CARD_W, 300);
  assert.equal(COL_W - CARD_W, 40);
});
