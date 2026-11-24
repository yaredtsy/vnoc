import assert from "node:assert/strict";
import { test } from "node:test";

import python from "@shikijs/langs/python";
import { createHighlighterCore } from "shiki/core";
import { createJavaScriptRegexEngine } from "shiki/engine/javascript";
import { bendGrammar } from "./bendGrammar.ts";

const [py] = python;
const before = JSON.stringify(py);
const bend = bendGrammar(py);

test("no invalid scope is left in Bend's grammar, and Python's own is not changed", () => {
  assert.equal(JSON.stringify(bend).includes("invalid."), false);
  assert.equal(JSON.stringify(py), before);
  assert.equal(JSON.stringify(py).includes("invalid.illegal.operator.python"), true);
  assert.equal(bend.name, "bend");
  assert.equal(bend.scopeName, "source.bend");
  assert.equal(py.scopeName, "source.python");
});

test("&&, || and the n of 1n are scoped as Bend's, `law` at the margin is a keyword", async () => {
  const highlighter = await createHighlighterCore({ themes: [import("@shikijs/themes/github-dark")], langs: [bend], engine: createJavaScriptRegexEngine() });
  const scopesOf = (line: string, token: string): string[] => {
    const { tokens } = highlighter.codeToTokens(line, { lang: "bend", theme: "github-dark", includeExplanation: true });
    const found = tokens[0].find((t) => t.content.includes(token));
    assert.ok(found, `${token} is a token of ${line}`);
    return found.explanation!.flatMap((e) => e.scopes.map((s) => s.scopeName));
  };
  assert.ok(scopesOf("  a && b", "&&").includes("keyword.operator.logical.bend"));
  assert.ok(scopesOf("  a || b", "||").includes("keyword.operator.logical.bend"));
  assert.ok(scopesOf("  x = 1n", "1n").some((s) => s.startsWith("constant.numeric")));
  assert.ok(scopesOf("law you_cant_win:", "law").includes("storage.type.function.bend"));
  for (const line of ["  a && b", "  x = 11n + 2n", "law w:", "  y = x ? z"]) {
    const { tokens } = highlighter.codeToTokens(line, { lang: "bend", theme: "github-dark", includeExplanation: true });
    const scopes = tokens.flat().flatMap((t) => t.explanation!.flatMap((e) => e.scopes.map((s) => s.scopeName)));
    assert.equal(scopes.some((s) => s.startsWith("invalid.")), false, line);
  }
});
