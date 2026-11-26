import assert from "node:assert/strict";
import { test } from "node:test";

import { formFor, formFromTour, paramsOf, shortName } from "./ask.ts";

test("the request is the node and every part as it stands", () => {
  assert.deepEqual(paramsOf({ ...formFor("a.py::f"), question: "why?", reach: "calls", detail: "quick" }), { key: "a.py::f", question: "why?", reach: "calls", detail: "quick" });
  assert.deepEqual(paramsOf(formFor(null)), { question: "", reach: "agent", detail: "agent" });
});

test("a row's tour fills the form with what the core answered, and the tour stays the node while the chip is there", () => {
  const form = formFromTour("t-1", { key: "a.py::f", question: "why twice?", detail: "quick" });
  assert.deepEqual(form, { about: "a.py::f", tour: "t-1", question: "why twice?", reach: "agent", detail: "quick" });
  assert.deepEqual(paramsOf(form), { tour: "t-1", question: "why twice?", reach: "agent", detail: "quick" });
  // the chip removed: no node, no tour
  assert.deepEqual(paramsOf({ ...form, about: null, tour: null }), { question: "why twice?", reach: "agent", detail: "quick" });
  // a tour with no written stop has no root to show
  assert.deepEqual(formFromTour("t-2", { detail: "agent" }), { about: null, tour: null, question: "", reach: "agent", detail: "agent" });
});

test("the chip says the last part of an address", () => {
  assert.equal(shortName("orders/service.py::OrderService.parse_order"), "OrderService.parse_order");
  assert.equal(shortName("orders/service.py"), "service.py");
  assert.equal(shortName("orders/"), "orders");
  assert.equal(shortName("app"), "app");
});
