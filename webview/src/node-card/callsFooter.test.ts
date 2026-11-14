import assert from "node:assert/strict";
import { test } from "node:test";

import type { Calls } from "@vnoc/shared/calls.ts";
import type { CallTreeNode } from "@vnoc/shared/messages.ts";
import { type CallsFooter, callsFooter } from "./callsFooter.ts";

const child = (): CallTreeNode => ({ key: null, name: "x", qualname: "x", path: null, kind: "function", call_count: 1, call_sites: [], marks: [], children: [] });
const known = (n: number): Calls => ({ state: "known", children: Array.from({ length: n }, child) });

const table: { name: string; calls: Calls; open: boolean; expect: CallsFooter }[] = [
  { name: "nothing known yet: nothing said, so nothing shifts when it arrives", calls: { state: "unknown" }, open: false, expect: null },
  { name: "probed 0: calls nothing", calls: { state: "probed", count: 0 }, open: false, expect: { state: "nothing" } },
  { name: "probed 4, closed: calls 4", calls: { state: "probed", count: 4 }, open: false, expect: { state: "count", n: 4 } },
  { name: "probed 4, open (the tree is on its way): nothing", calls: { state: "probed", count: 4 }, open: true, expect: null },
  { name: "known 0: calls nothing, open or not", calls: known(0), open: true, expect: { state: "nothing" } },
  { name: "known 3, closed: calls 3", calls: known(3), open: false, expect: { state: "count", n: 3 } },
  { name: "known 3, open: the cards say it", calls: known(3), open: true, expect: null },
  { name: "being fetched: a spinner, open or closed", calls: { state: "loading" }, open: true, expect: { state: "loading" } },
  { name: "failed: couldn't read, with Retry", calls: { state: "failed", message: "boom" }, open: false, expect: { state: "failed", message: "boom" } },
];
for (const row of table) test(`callsFooter: ${row.name}`, () => assert.deepEqual(callsFooter(row.calls, row.open), row.expect));
