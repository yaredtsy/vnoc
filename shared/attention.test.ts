import assert from "node:assert/strict";
import { test } from "node:test";

import { notReadRows, shortReason } from "./attention.ts";

const fail = (path: string, error: string) => ({ path, error });

test("nothing failed, nothing cut: no rows", () => assert.deepEqual(notReadRows({ failed: [], truncated: false }), []));

test("one file: its path, a short reason, the full error as the tooltip", () => {
  const error = "cannot read app/legacy.py: stream did not contain valid UTF-8";
  assert.deepEqual(notReadRows({ failed: [fail("app/legacy.py", error)], truncated: false }), [{ kind: "file", path: "app/legacy.py", label: "app/legacy.py", reason: "not UTF-8", tooltip: error }]);
});

test("two files: two rows, in the order given", () => {
  const rows = notReadRows({ failed: [fail("b.py", "cannot read b.py: permission denied"), fail("a.py", "boom")], truncated: false });
  assert.deepEqual(rows.map((r) => r.label), ["b.py", "a.py"]);
  assert.deepEqual(rows.map((r) => (r.kind === "file" ? r.reason : "")), ["permission denied", "boom"]);
});

test("truncated only: one row that says the structure is cut, with how many files were read", () => {
  const rows = notReadRows({ failed: [], truncated: true, stats: { files: 10000 } });
  assert.equal(rows.length, 1);
  assert.equal(rows[0].kind, "truncated");
  assert.equal(rows[0].label, "too many files: the structure is cut");
  assert.match(rows[0].tooltip, /^10000 files were read; V-NOC stopped there/);
});

test("both: the files first, the cut last", () => {
  const rows = notReadRows({ failed: [fail("a.py", "boom")], truncated: true });
  assert.deepEqual(rows.map((r) => r.kind), ["file", "truncated"]);
  assert.doesNotMatch(rows[1].tooltip, /files were read/, "no count when the structure did not say it");
});

test("a reason is one short line", () => {
  assert.equal(shortReason("cannot read x.py: stream did not contain valid UTF-8"), "not UTF-8");
  assert.equal(shortReason("cannot read x.py: permission denied\nmore detail"), "permission denied");
  assert.equal(shortReason(""), "could not be read");
  assert.equal(shortReason("x".repeat(100)).length, 60);
  assert.ok(shortReason("x".repeat(100)).endsWith("…"));
});
