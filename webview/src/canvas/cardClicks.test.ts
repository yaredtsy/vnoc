import assert from "node:assert/strict";
import { test } from "node:test";

import { doubleClickIntent } from "./cardClicks.ts";

test("double-click on a structure card focuses it", () => assert.deepEqual(doubleClickIntent({ id: "a.py::f", role: "node", key: "a.py::f" }), { type: "focus", id: "a.py::f" }));
test("on a call card with a known callee it explores, and activates", () => assert.deepEqual(doubleClickIntent({ id: "call:[\"f\",\"g\"]", role: "call", key: "g" }), { type: "explore", call: "call:[\"f\",\"g\"]", activate: true }));
test("on a call card whose callee is unknown it only selects", () => assert.deepEqual(doubleClickIntent({ id: "call:[\"f\",\"?x\"]", role: "call", key: null }), { type: "select", id: "call:[\"f\",\"?x\"]" }));
