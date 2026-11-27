import assert from "node:assert/strict";
import { test } from "node:test";

import { depthOf, formFor, formFromTour, paramsOf, shortName, sizeAt } from "./ask.ts";

test("the request is the node and every part as it stands, with no depth while the agent decides", () => {
  assert.deepEqual(paramsOf({ ...formFor("a.py::f"), question: "why?", detail: "quick" }), { key: "a.py::f", question: "why?", detail: "quick" });
  assert.deepEqual(paramsOf(formFor(null)), { question: "", detail: "agent" });
});

test("a depth is sent when the agent does not decide, and only for a node", () => {
  const form = { ...formFor("a.py::f"), agentDepth: false, depth: 2 };
  assert.deepEqual(paramsOf(form), { key: "a.py::f", question: "", depth: 2, detail: "agent" });
  assert.deepEqual(paramsOf({ ...form, depth: 0 }), { key: "a.py::f", question: "", depth: 0, detail: "agent" });
  // the chip removed: a depth is of a node, so none
  assert.deepEqual(paramsOf({ ...form, about: null }), { question: "", detail: "agent" });
});

test("a row's tour fills the form with what the core answered, and the tour stays the node while the chip is there", () => {
  const form = formFromTour("t-1", { key: "a.py::f", question: "why twice?", detail: "quick" });
  assert.deepEqual(form, { about: "a.py::f", tour: "t-1", question: "why twice?", agentDepth: true, depth: 1, detail: "quick" });
  assert.deepEqual(paramsOf(form), { tour: "t-1", question: "why twice?", detail: "quick" });
  assert.deepEqual(paramsOf({ ...form, agentDepth: false, depth: 3 }), { tour: "t-1", question: "why twice?", depth: 3, detail: "quick" });
  // the chip removed: no node, no tour
  assert.deepEqual(paramsOf({ ...form, about: null, tour: null }), { question: "why twice?", detail: "quick" });
  // a tour with no written stop has no root to show
  assert.deepEqual(formFromTour("t-2", { detail: "agent" }), { about: null, tour: null, question: "", agentDepth: true, depth: 1, detail: "agent" });
});

test("a typed depth is a whole number from 0 to 6, else the old one stays", () => {
  assert.equal(depthOf("3", 1), 3);
  assert.equal(depthOf("0", 1), 0);
  assert.equal(depthOf("9", 1), 6);
  assert.equal(depthOf("-2", 1), 0);
  assert.equal(depthOf("", 2), 2);
  assert.equal(depthOf("x", 2), 2);
  assert.equal(depthOf("1.5", 2), 2);
});

test("the size beside the number is the core's words for that depth", () => {
  const sizes = [
    { depth: 0, stops: 1, minutes: 1, words: "1 stop · about 1 min" },
    { depth: 1, stops: 9, minutes: 6, words: "9 stops · about 6 min" },
  ];
  assert.equal(sizeAt(sizes, 1), "9 stops · about 6 min");
  assert.equal(sizeAt(sizes, 4), null);
  assert.equal(sizeAt(null, 1), null);
});

test("the chip says the last part of an address", () => {
  assert.equal(shortName("orders/service.py::OrderService.parse_order"), "OrderService.parse_order");
  assert.equal(shortName("orders/service.py"), "service.py");
  assert.equal(shortName("orders/"), "orders");
  assert.equal(shortName("app"), "app");
});
