import assert from "node:assert/strict";
import { test } from "node:test";

import { type SidebarBody, type Status, sidebarBody } from "./sidebarBody.ts";

const some = { root: { children: [{}] } };
const none = { root: { children: [] } };

// the table of 06 §8: folder × status × what was read
const table: { folder: boolean; status: Status; structure: typeof some | null; expect: SidebarBody }[] = [
  { folder: false, status: "loading", structure: null, expect: "noFolder" },
  { folder: false, status: "ready", structure: some, expect: "noFolder" },
  { folder: true, status: "loading", structure: null, expect: "loading" },
  { folder: true, status: "error", structure: null, expect: "error" },
  { folder: true, status: "ready", structure: none, expect: "empty" },
  { folder: true, status: "ready", structure: some, expect: "ready" },
  { folder: true, status: "loading", structure: some, expect: "ready" }, // a reload over rows already drawn: they stay
  { folder: true, status: "error", structure: some, expect: "ready" }, // a failed refresh over rows already drawn: they stay
  { folder: true, status: "loading", structure: none, expect: "empty" },
];
for (const t of table) test(`folder ${t.folder}, ${t.status}, ${t.structure ? `${t.structure.root.children.length} nodes` : "nothing read"} → ${t.expect}`, () => assert.equal(sidebarBody(t.folder, t.status, t.structure), t.expect));
