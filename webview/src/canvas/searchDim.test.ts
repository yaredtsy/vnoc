import assert from "node:assert/strict";
import { test } from "node:test";

import { type Dim, dimmedCards, keysOf } from "./searchDim.ts";

const card = (id: string, key: string | null = id) => ({ id, key });
const dim = (asked: string[], hits: string[]): Dim => ({ asked: new Set(asked), hits: new Set(hits) });

test("the keys to ask about are the cards' own, once each, and a call outside the index has none", () => {
  assert.deepEqual(keysOf([card("a"), card("call:1", "b"), card("call:2", null), card("call:3", "b")]), ["a", "b"]);
  assert.deepEqual(keysOf([]), []);
});

test("a card the query does not find is dimmed; a hit is not; a call with no key always is", () => {
  const cards = [card("a"), card("b"), card("c"), card("call:1", null), card("call:2", "a")];
  assert.deepEqual(dimmedCards(cards, dim(["a", "b", "c"], ["a"]), null), ["b", "c", "call:1"]);
});

test("the selected card is never dimmed, hit or not, with a key or without", () => {
  const cards = [card("a"), card("b"), card("call:1", null)];
  assert.deepEqual(dimmedCards(cards, dim(["a", "b"], []), "b"), ["a", "call:1"]);
  assert.deepEqual(dimmedCards(cards, dim(["a", "b"], []), "call:1"), ["a", "b"]);
});

test("no answer yet dims nothing, and a card the question did not cover is left alone until the next answer", () => {
  const cards = [card("a"), card("new")];
  assert.deepEqual(dimmedCards(cards, null, null), []);
  assert.deepEqual(dimmedCards(cards, dim(["a"], []), null), ["a"]);
});
