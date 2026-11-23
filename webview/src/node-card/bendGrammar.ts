// Bend's colours: Python's TextMate grammar without the marks Python puts on what it thinks is an error. Bend 2 has no grammar of its own, and its own docs fence its code as python.
// Pure (the Python grammar comes in as an argument), so a test can load it; monaco.ts hands it to Shiki.

import type { LanguageRegistration } from "shiki/core";

/** Python's `&&`, `||` (and the few other things it calls illegal) are Bend's logical operators. */
const OPERATOR = "keyword.operator.logical.bend";
/** the `n` of `1n`, which Python flags as a bad number suffix, is part of a Bend number */
const NUMBER = "constant.numeric.bend";

/** What a scope name of Python's becomes: another name, or nothing (the rule stays, only its name goes). */
function rename(scope: string): string | null {
  if (!scope.startsWith("invalid.")) return scope;
  if (scope.startsWith("invalid.illegal.operator.")) return OPERATOR;
  if (scope.startsWith("invalid.illegal.name.") || scope.startsWith("invalid.illegal.dec.")) return NUMBER;
  return null;
}

/** The rules of a grammar, copied, with every `name` that is an `invalid.*` scope renamed or dropped. */
function clean(node: unknown): unknown {
  if (Array.isArray(node)) return node.map(clean);
  if (node === null || typeof node !== "object") return node;
  const out: Record<string, unknown> = {};
  for (const [key, value] of Object.entries(node)) {
    if (key === "name" && typeof value === "string") {
      const name = rename(value);
      if (name !== null) out[key] = name;
    } else out[key] = clean(value);
  }
  return out;
}

/** `law name:` starts a law at the left margin: a keyword like `def`. */
const LAW = { match: "^law\\b", name: "storage.type.function.bend" };

/** The grammar `bend` (scope `source.bend`), made from a copy of Python's; `python` itself is not touched. */
export function bendGrammar(python: LanguageRegistration): LanguageRegistration {
  const copy = clean(python) as LanguageRegistration;
  return { ...copy, name: "bend", scopeName: "source.bend", aliases: undefined, patterns: [LAW, ...(copy.patterns ?? [])] };
}
