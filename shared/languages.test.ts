import assert from "node:assert/strict";
import { test } from "node:test";

import { languageExtensions } from "./languages.ts";
import type { LanguageInfo } from "./messages.ts";

const language = (name: string, extensions: string[], over: Partial<LanguageInfo> = {}): LanguageInfo => ({ name, version: "0.3.0", path: `/d/vnoc-driver-${name}`, extensions, capabilities: { outline: true, callTree: true }, state: "running", ...over });

test("languageExtensions: every extension of a running driver, with the dot", () => {
  assert.deepEqual(languageExtensions([language("python", ["py"]), language("bend", ["bend"])]).all, [".bend", ".py"]);
  assert.deepEqual(languageExtensions([]), { all: [], callTree: [] });
});

test("languageExtensions: a failed driver has no files, and call trees follow the driver's own capability", () => {
  const got = languageExtensions([
    language("python", ["py"]),
    language("bend", ["bend"], { capabilities: { outline: true } }),
    language("ts", ["ts", "tsx"], { state: "failed", error: "not found" }),
  ]);
  assert.deepEqual(got.all, [".bend", ".py"]);
  assert.deepEqual(got.callTree, [".py"]);
});
