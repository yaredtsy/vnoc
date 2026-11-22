import assert from "node:assert/strict";
import { join } from "node:path";
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

import { type DriverFacts, chooseDrivers, isDriverFileName } from "./binaryChoice.ts";

const tree = (files: Record<string, string[]>) => (dir: string) => files[dir] ?? [];
const driverFacts = (over: Partial<DriverFacts> = {}): DriverFacts => ({ configured: [], developing: true, driversDir: join("/repo", "drivers"), binDir: join("/ext", "bin"), windows: false, list: tree({}), ...over });
const dev = (language: string) => join("/repo", "drivers", language, "target", "debug");

test("isDriverFileName: the core's rule, with the OS's suffix", () => {
  assert.ok(isDriverFileName("vnoc-driver-bend", false));
  assert.ok(isDriverFileName("vnoc-driver-my_lang-2", false));
  assert.ok(!isDriverFileName("vnoc-driver-bend.d", false));
  assert.ok(!isDriverFileName("vnoc-driver-bend.pdb", false));
  assert.ok(!isDriverFileName("vnoc-driver-", false));
  assert.ok(!isDriverFileName("vnoc-driver-Bend", false));
  assert.ok(!isDriverFileName("vnoc", false));
  assert.ok(isDriverFileName("vnoc-driver-bend.exe", true));
  assert.ok(!isDriverFileName("vnoc-driver-bend", true));
  assert.ok(!isDriverFileName("vnoc-driver-bend.pdb", true));
});

test("chooseDrivers: a list set by the user replaces finding", () => {
  assert.deepEqual(chooseDrivers(driverFacts({ configured: ["/mine/a", "/mine/b"] })), { paths: ["/mine/a", "/mine/b"] });
});

test("chooseDrivers: from source, every driver built in drivers/*/target/debug, and never bin/", () => {
  const list = tree({
    [join("/repo", "drivers")]: ["python", "bend", "typescript"],
    [dev("python")]: ["vnoc-driver-python", "vnoc-driver-python.d", "deps"],
    [dev("bend")]: ["vnoc-driver-bend", "build"],
    [join("/ext", "bin")]: ["vnoc-driver-old"],
  });
  assert.deepEqual(chooseDrivers(driverFacts({ list })), { paths: [join(dev("bend"), "vnoc-driver-bend"), join(dev("python"), "vnoc-driver-python")] });
});

test("chooseDrivers: from source with nothing built is an error that says how to build one", () => {
  const chosen = chooseDrivers(driverFacts({ list: tree({ [join("/repo", "drivers")]: ["python"] }) }));
  assert.ok("error" in chosen && /build a driver: `cargo build` in drivers\/<language>/.test(chosen.error), JSON.stringify(chosen));
});

test("chooseDrivers: installed, every driver in bin/ (the .exe on Windows)", () => {
  const bin = join("/ext", "bin");
  const list = tree({ [bin]: ["vnoc.exe", "vnoc-driver-python.exe", "vnoc-driver-bend.exe", "vnoc-driver-bend.pdb"], [dev("python")]: ["vnoc-driver-python.exe"] });
  assert.deepEqual(chooseDrivers(driverFacts({ developing: false, windows: true, list })), { paths: [join(bin, "vnoc-driver-bend.exe"), join(bin, "vnoc-driver-python.exe")] });
  const none = chooseDrivers(driverFacts({ developing: false }));
  assert.ok("error" in none && none.error.includes(bin));
});
