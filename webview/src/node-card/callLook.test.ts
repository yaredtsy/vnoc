import assert from "node:assert/strict";
import { test } from "node:test";

import type { Calls } from "@vnoc/shared/calls.ts";
import type { CallMark, CallTreeNode } from "@vnoc/shared/messages.ts";
import { type CallLook, callLook } from "./callLook.ts";

const child = (): CallTreeNode => ({ key: null, name: "x", qualname: "x", path: null, kind: "function", call_count: 1, call_sites: [], marks: [], children: [] });
const known = (n: number): Calls => ({ state: "known", children: Array.from({ length: n }, child) });

// the table of 05 §6: what each kind of call node shows
const table: { name: string; key: string | null; marks: CallMark[]; calls: Calls; open: boolean; expect: Partial<CallLook> }[] = [
  { name: "an ordinary call with calls of its own: a chevron and +N", key: "k", marks: [], calls: known(3), open: false, expect: { chevron: true, plus: 3, mark: null, unknown: false } },
  { name: "open: no +N", key: "k", marks: [], calls: known(3), open: true, expect: { chevron: true, plus: null } },
  { name: "an ordinary call that calls nothing: no chevron, no mark", key: "k", marks: [], calls: known(0), open: false, expect: { chevron: false, plus: null, mark: null } },
  { name: "recursive: shown, not entered: no chevron, ↻ recursive", key: "k", marks: ["recursive"], calls: known(0), open: false, expect: { chevron: false, mark: { text: "↻ recursive", title: "It is already above on this path: shown, not entered" } } },
  { name: "recursive wins even if something is under it", key: "k", marks: ["recursive"], calls: known(2), open: false, expect: { chevron: false, plus: null } },
  { name: "cut with no children: a chevron (opening asks for the callee's tree), … calls not loaded, no +N", key: "k", marks: ["cut"], calls: known(0), open: false, expect: { chevron: true, plus: null, mark: { text: "… calls not loaded", title: "The analysis stopped here. Open to load what this function calls." } } },
  { name: "cut, the callee's tree not asked for yet: the same", key: "k", marks: ["cut"], calls: { state: "unknown" }, open: false, expect: { chevron: true, plus: null } },
  { name: "cut, the callee probed: the chevron and the probe's count", key: "k", marks: ["cut"], calls: { state: "probed", count: 4 }, open: false, expect: { chevron: true, plus: 4 } },
  { name: "cut and open, the callee's tree arrived with 5 calls: still marked, +N gone", key: "k", marks: ["cut"], calls: known(5), open: true, expect: { chevron: true, plus: null } },
  { name: "incomplete with children: a chevron, … incomplete", key: "k", marks: ["incomplete"], calls: known(2), open: false, expect: { chevron: true, plus: 2, mark: { text: "… incomplete", title: "The analysis gave up on part of this" } } },
  { name: "likely: an ordinary card with a mark", key: "k", marks: ["likely"], calls: known(1), open: false, expect: { chevron: true, mark: { text: "≈ likely", title: "The engine's best guess for the function called here, not a certainty" } } },
  { name: "several marks: the first of recursive, cut, incomplete, likely is shown", key: "k", marks: ["likely", "incomplete", "cut"], calls: known(0), open: false, expect: { mark: { text: "… calls not loaded", title: "The analysis stopped here. Open to load what this function calls." } } },
  { name: "being fetched: the chevron stays", key: "k", marks: [], calls: { state: "loading" }, open: true, expect: { chevron: true, plus: null } },
  { name: "a callee the project does not know: unknown", key: null, marks: [], calls: known(0), open: false, expect: { unknown: true, chevron: false } },
];
for (const row of table) {
  test(`callLook: ${row.name}`, () => {
    const look = callLook({ key: row.key, marks: row.marks, calls: row.calls }, row.open);
    for (const [field, value] of Object.entries(row.expect)) assert.deepEqual(look[field as keyof CallLook], value, field);
  });
}
