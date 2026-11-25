import assert from "node:assert/strict";
import { test } from "node:test";

import type { TourRow } from "@vnoc/shared/tours.ts";
import { isOpen, listItem } from "./list.ts";

const row = (over: Partial<TourRow> = {}): TourRow => ({ id: "t-1", folder: "private", readable: true, title: "Retry charge", question: "what happens when it drops", quiet: false, root: { kind: "function", name: "retry_charge" }, place: null, offers: null, words: "stop 3 · 9 stops", ...over });

test("a row draws the core's words unchanged, the title, the question, and the icon of its first stop", () => {
  assert.deepEqual(listItem(row()), { key: "t-1:private", id: "t-1", title: "Retry charge", words: "stop 3 · 9 stops", question: "what happens when it drops", kind: "function", clickable: true });
  // whatever the core says is what is drawn: a state word, `shared ·`, a size, in its order
  for (const words of ["writing · 6 of 10 stops", "shared · changed · 3 stops", "no news · 1 stop", "seen 2 of 9 · 9 stops", "new · 1 stop", "finished · 4 stops"]) assert.equal(listItem(row({ words })).words, words);
});

test("a tour with no question or no stop yet draws neither", () => {
  const item = listItem(row({ question: undefined, root: null }));
  assert.deepEqual([item.question, item.kind], [null, null]);
});

test("a tour that cannot be read shows its problem after the core's words, and opens nothing", () => {
  const item = listItem(row({ readable: false, title: undefined, problem: "the first line is not JSON", words: "cannot be read", question: "ignored" }));
  assert.deepEqual([item.title, item.words, item.question, item.clickable], ["t-1", "cannot be read: the first line is not JSON", null, false]);
});

test("the same id in both folders is two rows, and only the shared one is the open tour's", () => {
  const shared = row({ folder: "project" });
  const copy = row({ folder: "private", private_copy: true });
  assert.notEqual(listItem(shared).key, listItem(copy).key);
  assert.deepEqual([isOpen(shared, "t-1"), isOpen(copy, "t-1"), isOpen(shared, null), isOpen(shared, "t-2")], [true, false, false, false]);
});
