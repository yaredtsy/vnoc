import assert from "node:assert/strict";
import { test } from "node:test";

import type { Calls } from "@vnoc/shared/calls.ts";
import type { CallTreeNode } from "@vnoc/shared/messages.ts";
import { type CallsFooter, callsFooter } from "./callsFooter.ts";

const child = (): CallTreeNode => ({ key: null, name: "x", qualname: "x", path: null, kind: "function", call_count: 1, call_sites: [], marks: [], children: [] });
const known = (n: number): Calls => ({ state: "known", children: Array.from({ length: n }, child) });

const table: { name: string; calls: Calls; expect: CallsFooter }[] = [
  { name: "nothing known yet: nothing said, so nothing shifts when it arrives", calls: { state: "unknown" }, expect: null },
  { name: "probed 0: calls nothing", calls: { state: "probed", count: 0 }, expect: { state: "nothing" } },
  { name: "probed 4: nothing here (a closed card's +N says it, an open one draws them)", calls: { state: "probed", count: 4 }, expect: null },
  { name: "known 0: calls nothing, open or not", calls: known(0), expect: { state: "nothing" } },
  { name: "known 3: nothing here either", calls: known(3), expect: null },
  { name: "being fetched: a spinner, open or closed", calls: { state: "loading" }, expect: { state: "loading" } },
  { name: "failed: couldn't read, with Retry", calls: { state: "failed", message: "boom" }, expect: { state: "failed", message: "boom" } },
];
for (const row of table) test(`callsFooter: ${row.name}`, () => assert.deepEqual(callsFooter(row.calls), row.expect));
