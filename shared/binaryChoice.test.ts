import assert from "node:assert/strict";
import { test } from "node:test";

import { type BinaryFacts, chooseBinary } from "./binaryChoice.ts";

const facts = (over: Partial<BinaryFacts> & { have?: string[] } = {}): BinaryFacts => {
  const { have = [], ...rest } = over;
  return { configured: undefined, developing: true, fromSource: "/repo/target/debug/driver", packaged: "/ext/bin/driver", exists: (p) => have.includes(p), buildHint: "run `cargo build` in drivers/python", label: "The Python driver", ...rest };
};

const table: { name: string; facts: BinaryFacts; expect: { path: string } | RegExp }[] = [
  { name: "the setting wins, built or not, from source or installed", facts: facts({ configured: "/mine/driver" }), expect: { path: "/mine/driver" } },
  { name: "from source: the build folder", facts: facts({ have: ["/repo/target/debug/driver"] }), expect: { path: "/repo/target/debug/driver" } },
  { name: "from source with both: the build folder, not the old copy", facts: facts({ have: ["/repo/target/debug/driver", "/ext/bin/driver"] }), expect: { path: "/repo/target/debug/driver" } },
  { name: "from source, not built, an old copy in bin/: an error that says how to build, never the old copy", facts: facts({ have: ["/ext/bin/driver"] }), expect: /not built .*run `cargo build` in drivers\/python/ },
  { name: "from source, nothing anywhere: the same error", facts: facts(), expect: /The Python driver is not built/ },
  { name: "installed: the copy in bin/", facts: facts({ developing: false, have: ["/ext/bin/driver"] }), expect: { path: "/ext/bin/driver" } },
  { name: "installed with a build folder around: still bin/", facts: facts({ developing: false, have: ["/ext/bin/driver", "/repo/target/debug/driver"] }), expect: { path: "/ext/bin/driver" } },
  { name: "installed and missing: says where it should be", facts: facts({ developing: false }), expect: /The Python driver is not at \/ext\/bin\/driver/ },
];

for (const row of table) {
  test(`chooseBinary: ${row.name}`, () => {
    const chosen = chooseBinary(row.facts);
    if (row.expect instanceof RegExp) assert.match("error" in chosen ? chosen.error : `path ${chosen.path}`, row.expect);
    else assert.deepEqual(chosen, row.expect);
  });
}
