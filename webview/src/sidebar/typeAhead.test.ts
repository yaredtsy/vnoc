import assert from "node:assert/strict";
import { test } from "node:test";

import type { KeyFacts } from "./keys.ts";
import { TYPE_AHEAD_MS, type TypeAhead, isTyped, noTypeAhead, typeAhead } from "./typeAhead.ts";

const rows = ["alpha", "Alpine", "abc", "beta", "ab_test", "gamma"].map((name) => ({ id: name, node: { name } }));
const key = (k: string, mods: Partial<KeyFacts> = {}): KeyFacts => ({ key: k, ctrlKey: false, metaKey: false, altKey: false, shiftKey: false, ...mods });

/** Types `keys` with the gaps in ms before each, from `selected`; the row selected after each key (it stays when nothing matches). */
function type(keys: [string, number][], selected: string | null, state: TypeAhead = noTypeAhead): (string | null)[] {
  let now = 10_000;
  let current = selected;
  const out: (string | null)[] = [];
  for (const [k, gap] of keys) {
    now += gap;
    const r = typeAhead(state, key(k), now, rows, current);
    state = r.state;
    current = r.id ?? current;
    out.push(current);
  }
  return out;
}

// the table of 06 §7: what was typed in the last 500 ms, from where
const table: { name: string; keys: [string, number][]; from: string | null; expect: (string | null)[] }[] = [
  { name: "one letter: the first row that starts with it", keys: [["b", 0]], from: null, expect: ["beta"] },
  { name: "one letter looks from the row after the selected one", keys: [["a", 0]], from: "alpha", expect: ["Alpine"] },
  { name: "the same letter again, after the pause, goes on to the next row that starts with it", keys: [["a", 0], ["a", 800]], from: "gamma", expect: ["alpha", "Alpine"] },
  { name: "the search wraps", keys: [["a", 0]], from: "ab_test", expect: ["alpha"] },
  { name: "case does not matter", keys: [["A", 0]], from: null, expect: ["alpha"] },
  { name: "several letters within 500 ms are one prefix: `ab` finds abc", keys: [["a", 0], ["b", 200]], from: null, expect: ["alpha", "abc"] },
  { name: "…and the selected row stays when it still starts with them: `a` lands on ab_test, `ab` keeps it", keys: [["a", 0], ["b", 100]], from: "beta", expect: ["ab_test", "ab_test"] },
  { name: "a letter more than 500 ms after the last one starts again", keys: [["a", 0], ["b", TYPE_AHEAD_MS + 1]], from: null, expect: ["alpha", "beta"] },
  { name: "exactly 500 ms still counts as typed together", keys: [["a", 0], ["b", TYPE_AHEAD_MS]], from: null, expect: ["alpha", "abc"] },
  { name: "three letters narrow it: `ab_` finds ab_test", keys: [["a", 0], ["b", 50], ["_", 50]], from: null, expect: ["alpha", "abc", "ab_test"] },
  { name: "no row starts with it: the selection stays", keys: [["z", 0]], from: "beta", expect: ["beta"] },
  { name: "the same letter several times within 500 ms goes on cycling", keys: [["a", 0], ["a", 100], ["a", 100]], from: null, expect: ["alpha", "Alpine", "abc"] },
];
for (const t of table) test(`type-ahead: ${t.name}`, () => assert.deepEqual(type(t.keys, t.from), t.expect));

test("type-ahead: what is not a letter does not touch what was typed", () => {
  const first = typeAhead(noTypeAhead, key("a"), 1000, rows, null);
  for (const k of [key("ArrowDown"), key("Shift"), key(" "), key("Enter"), key("/"), key("a", { ctrlKey: true }), key("a", { metaKey: true }), key("a", { altKey: true })]) {
    const r = typeAhead(first.state, k, 1100, rows, "alpha");
    assert.deepEqual([r.state, r.id], [first.state, null], k.key + (k.ctrlKey ? "+ctrl" : k.metaKey ? "+meta" : k.altKey ? "+alt" : ""));
  }
  assert.equal(isTyped(key("A", { shiftKey: true })), true, "a capital is a letter");
  assert.equal(isTyped(key("_")), true);
});

test("type-ahead: no rows, nothing to select", () => assert.equal(typeAhead(noTypeAhead, key("a"), 0, [], null).id, null));
