import assert from "node:assert/strict";
import { test } from "node:test";

import { type ChevronAction, chevronAction, heldBackTip } from "./chevronAction.ts";

// the table of 03 §6.1: (structure | call) × (held back | open | closed) → focus | explore | toggle. Open and closed differ only in `collapsed`, which the action does not read.
const rows: { role: "node" | "call"; state: string; heldBack: boolean; collapsed: boolean; action: ChevronAction }[] = [
  { role: "node", state: "held back", heldBack: true, collapsed: true, action: "focus" },
  { role: "node", state: "open", heldBack: false, collapsed: false, action: "toggle" },
  { role: "node", state: "closed", heldBack: false, collapsed: true, action: "toggle" },
  { role: "call", state: "held back", heldBack: true, collapsed: true, action: "explore" },
  { role: "call", state: "open", heldBack: false, collapsed: false, action: "toggle" },
  { role: "call", state: "closed", heldBack: false, collapsed: true, action: "toggle" },
];

for (const r of rows) {
  test(`${r.role === "node" ? "a structure card" : "a call card"}, ${r.state} → ${r.action}`, () => assert.equal(chevronAction({ role: r.role, heldBack: r.heldBack }), r.action));
}

test("the +N tooltip of a held-back card (03 §6.1)", () => {
  assert.equal(heldBackTip({ role: "node" }), "Not drawn: the canvas shows at most 200 cards. Click to focus on it.");
  assert.equal(heldBackTip({ role: "call" }), "Not drawn: the canvas shows at most 200 cards. Click to explore it.");
});
