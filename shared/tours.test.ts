import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

// `tours.ts` is types only, so what it declares is read from its source: every `export interface`, one property to a line, `name?: type;` optional.
// The Rust goldens are what the core prints (`crates/vnoc/tests/golden/`, `crates/vnoc/tests/fixtures/tours/v1/golden/`). For each object in them the test
// checks that every key the TypeScript type declares as required is there, and that no key is undeclared: the two cannot drift without this failing.
// What it cannot see: a key no golden carries (a stop's `as_written`, `now`, a row's `problem`) and a key's type; the keys are the part that drifts.

const here = new URL(".", import.meta.url);
const declared = parseInterfaces(readFileSync(new URL("tours.ts", here), "utf8"));

/** `Name -> { required, optional }` for each `export interface` of a source, from its properties written two spaces in. */
function parseInterfaces(source: string): Map<string, { required: Set<string>; optional: Set<string> }> {
  const found = new Map<string, { required: Set<string>; optional: Set<string> }>();
  for (const match of source.matchAll(/^export interface (\w+)[^{]*\{\n([\s\S]*?)\n\}/gm)) {
    const required = new Set<string>();
    const optional = new Set<string>();
    for (const line of match[2].split("\n")) {
      const property = /^ {2}(\w+)(\?)?:/.exec(line);
      if (property) (property[2] ? optional : required).add(property[1]);
    }
    found.set(match[1], { required, optional });
  }
  return found;
}

/** The interface of each object a golden holds below the one named: which key holds which, and whether it is a list. */
const NESTED: Record<string, Record<string, { type: string; list?: boolean }>> = {
  TourRow: { root: { type: "RootView" }, place: { type: "PlaceSummary" }, offers: { type: "OffersView" }, on: { type: "OnView" } },
  Place: { position: { type: "PositionView" } },
  TourView: { stops: { type: "TourStop", list: true }, place: { type: "Place" }, offers: { type: "OffersView" }, check: { type: "CheckInfo" } },
  TourStop: { blocks: { type: "BlockView", list: true }, reading: { type: "StepView", list: true }, now: { type: "NowView" }, as_written: { type: "AsWritten" } },
};

/** What the command line wraps around a view and the core's answer does not have. */
const ENVELOPE = ["schema", "command", "readable"];

const problems: string[] = [];

function check(type: string, value: unknown, where: string, skip: string[] = []): void {
  const shape = declared.get(type);
  if (!shape) return void problems.push(`${where}: tours.ts declares no ${type}`);
  if (typeof value !== "object" || value === null || Array.isArray(value)) return void problems.push(`${where}: expected an object for ${type}`);
  const keys = Object.keys(value).filter((k) => !skip.includes(k));
  for (const key of keys) if (!shape.required.has(key) && !shape.optional.has(key)) problems.push(`${where}: the golden has \`${key}\`, which ${type} does not declare`);
  for (const key of shape.required) if (!keys.includes(key)) problems.push(`${where}: ${type} requires \`${key}\`, which the golden does not have`);
  for (const [key, nested] of Object.entries(NESTED[type] ?? {})) {
    const child = (value as Record<string, unknown>)[key];
    if (child === undefined || child === null) continue;
    if (nested.list) (child as unknown[]).forEach((item, i) => check(nested.type, item, `${where}.${key}[${i}]`));
    else check(nested.type, child, `${where}.${key}`);
  }
}

const read = (path: string): any => JSON.parse(readFileSync(new URL(path, here), "utf8"));

const GOLDEN = "../crates/vnoc/tests/golden/";
const FROZEN = "../crates/vnoc/tests/fixtures/tours/v1/golden/";

test("the parser reads what tours.ts declares", () => {
  assert.deepEqual([...declared.get("TourStop")!.required].slice(0, 3), ["number", "id", "depth"]);
  assert.ok(declared.get("TourStop")!.required.has("key"), "a stop says its tree key");
  assert.ok(declared.get("TourStop")!.optional.has("as_written"));
  assert.ok(declared.get("TourRow")!.required.has("quiet") && !declared.get("TourRow")!.required.has("title"));
  for (const name of ["TourRow", "TourView", "TourStop", "StepView", "BlockView", "Place", "PositionView", "OnView", "AsWritten", "OffersView", "PlaceSummary", "RootView", "CheckInfo", "NowView"]) assert.ok(declared.has(name), name);
});

test("tour-show.json is a TourView", () => {
  check("TourView", read(`${GOLDEN}tour-show.json`), "tour-show.json", ENVELOPE);
  assert.deepEqual(problems, []);
});

test("tour-list.json and tour-list-on.json are lists of TourRow", () => {
  for (const name of ["tour-list.json", "tour-list-on.json"]) {
    const golden = read(`${GOLDEN}${name}`);
    assert.ok(golden.tours.length > 0, `${name} has rows`);
    golden.tours.forEach((row: unknown, i: number) => check("TourRow", row, `${name}.tours[${i}]`));
  }
  assert.ok(read(`${GOLDEN}tour-list-on.json`).tours.some((row: any) => row.on), "a row says how the tour is on the node");
  assert.deepEqual(problems, []);
});

test("the schema 1 fixtures' show goldens are TourViews, a stop's key among them", () => {
  for (const name of ["show-shared.json", "show-private.json", "show-imported.json"]) {
    const golden = read(`${FROZEN}${name}`);
    check("TourView", golden, name, ENVELOPE);
    assert.ok(golden.stops.length > 0 && golden.stops.every((s: any) => "key" in s), `${name}: every stop has a key`);
  }
  assert.deepEqual(problems, []);
});

test("the goldens carry a stop's key as a tree key, or null when the stop is planned", () => {
  const keys = read(`${FROZEN}show-shared.json`).stops.map((s: any) => s.key);
  assert.deepEqual(keys, ["billing/charge.py::retry_charge", "billing/invoice.py::Invoice.total", "billing/tax.py::apply_tax"]);
});
