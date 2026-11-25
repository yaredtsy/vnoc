import assert from "node:assert/strict";
import { test } from "node:test";

import type { TourRow } from "@vnoc/shared/tours.ts";
import { type Tomb, afterOf, withTombs } from "./deleted.ts";

const row = (id: string): TourRow => ({ id, folder: "private", readable: true }) as TourRow;
const tomb = (ids: string[], after: string | null): Tomb => ({ key: ids.join("+"), ids, tokens: ids.map((i) => `tok-${i}`), words: "Deleted.", after });
const shape = (entries: ReturnType<typeof withTombs>) => entries.map((e) => (e.type === "row" ? e.row.id : `✗${e.tomb.ids.join("+")}`));

test("the row above a deleted one is what its place is told by; the first row has none", () => {
  const rows = ["a", "b", "c", "d"].map(row);
  assert.equal(afterOf(rows, ["c"]), "b");
  assert.equal(afterOf(rows, ["a"]), null);
  assert.equal(afterOf(rows, ["b", "c"]), "a", "several deleted together: the one above the first of them");
  assert.equal(afterOf(rows, ["a", "b"]), null);
  assert.equal(afterOf(rows, ["x"]), null, "not in the list: the top");
});

test("a deleted row is drawn in its place, whether the list was asked again (the row gone) or not (the row still there)", () => {
  const t = tomb(["c"], "b");
  assert.deepEqual(shape(withTombs(["a", "b", "d"].map(row), [t])), ["a", "b", "✗c", "d"], "the list was asked again");
  assert.deepEqual(shape(withTombs(["a", "b", "c", "d"].map(row), [t])), ["a", "b", "✗c", "d"], "it was not: the row is not drawn twice");
});

test("a deleted first row is drawn at the top; one whose row above is gone too goes to the top", () => {
  assert.deepEqual(shape(withTombs(["b", "c"].map(row), [tomb(["a"], null)])), ["✗a", "b", "c"]);
  assert.deepEqual(shape(withTombs(["c", "d"].map(row), [tomb(["b"], "a")])), ["✗b", "c", "d"]);
});

test("Delete Finished Tours is one entry for all of its tours, where the first was", () => {
  assert.deepEqual(shape(withTombs(["a", "d"].map(row), [tomb(["b", "c"], "a")])), ["a", "✗b+c", "d"]);
  assert.deepEqual(shape(withTombs(["a", "b", "c", "d"].map(row), [tomb(["b", "c"], "a")])), ["a", "✗b+c", "d"]);
});

test("nothing deleted: the rows as they are", () => {
  assert.deepEqual(shape(withTombs(["a", "b"].map(row), [])), ["a", "b"]);
});
