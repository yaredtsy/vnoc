#!/usr/bin/env python3
"""Generates tests/golden_data.rs: the expected node IDs, written ONCE and committed as literals.

This script is the independent oracle: it shares no code with the Rust implementation. It uses the Python `blake3`
package (keyed derive mode) and a hand-written Crockford base32 encoder, and it takes qualnames from Python's own
compiler (`co_qualname`), not from V-NOC's parser.

  python3 -m venv /tmp/v && /tmp/v/bin/pip install blake3 && /tmp/v/bin/python gen_golden.py

The spec it implements (VS-D79):
  input  = repo_path \\0 qualname \\0 occurrence(decimal) [\\0 counter(decimal, from 2, only after a collision)]
  hash   = blake3 derive_key, context "v-noc node id v1"; take the first 16 bytes as a big-endian 128-bit integer
  text   = Crockford base32, 26 chars, uppercase; the 2 spare bits sit in the FIRST char (so it is 0-7)
"""
import os, sys, blake3

ALPHABET = "0123456789ABCDEFGHJKMNPQRSTVWXYZ"
CONTEXT = "v-noc node id v1"


def crockford(n: int) -> str:
    assert 0 <= n < 2**128
    return "".join(ALPHABET[(n >> (5 * (25 - i))) & 31] for i in range(26))


def node_id(path: str, qualname: str, occurrence: int, counter: int = 0) -> str:
    data = path.encode() + b"\0" + qualname.encode() + b"\0" + str(occurrence).encode()
    if counter:
        data += b"\0" + str(counter).encode()
    digest = blake3.blake3(data, derive_key_context=CONTEXT).digest()
    return crockford(int.from_bytes(digest[:16], "big"))


def definitions(source: str):
    """(qualname, occurrence) of every def and class, in source order, from Python's own compiler."""
    found = []

    def walk(code):
        for c in code.co_consts:
            if hasattr(c, "co_qualname"):
                if not c.co_name.startswith("<"):  # skip lambdas and comprehensions
                    found.append((c.co_firstlineno, c.co_qualname))
                walk(c)

    walk(compile(source, "fixture", "exec"))
    found.sort()
    seen, out = {}, []
    for _, q in found:
        seen[q] = seen.get(q, 0) + 1
        out.append((q, seen[q]))
    return out


here = os.path.dirname(os.path.abspath(__file__))
fixture_path = "pkg/nested.py"  # the repo-relative path the Rust tests pretend the fixture has
cases = [
    ("src/orders.py", "OrderService.parse_order", 1, 0),
    ("src/orders.py", "OrderService.parse_order", 2, 0),
    ("src/orders.py", "OrderService.parse_order", 1, 2),
    ("src/orders.py", "OrderService.parse_order", 1, 3),
    ("src/orders.py", "", 1, 0),
    ("src/a.py", "top.<locals>.inner.<locals>.deepest", 1, 0),
    ("my dir/mod.py", "f", 1, 0),
    ("src/big.py", "f", 12, 0),
    ("café/é.py", "f", 1, 0),  # precomposed (NFC), as git records it on macOS
    ("café/é.py", "f", 1, 0),  # decomposed (NFD): a DIFFERENT repo_path, so a different ID
]
ints = [0, 1, 31, 32, 2**125 - 1, 2**125, 2**128 - 1, 0x0123456789ABCDEF0123456789ABCDEF]

out = ["// GENERATED ONCE by tests/golden/gen_golden.py (an independent Python oracle). Committed as literals.", "// Do not edit by hand: if the ID spec changes, that is a new format version, not a test fix.", ""]
out.append("/// (repo_path, qualname, occurrence, collision counter (0 = none), expected id)")
out.append("pub const ID_CASES: &[(&str, &str, u32, u32, &str)] = &[")
for p, q, o, c in cases:
    out.append(f'    ("{p}", "{q}", {o}, {c}, "{node_id(p, q, o, c)}"),')
out.append("];\n")
out.append("/// (a 128-bit value, its Crockford text)")
out.append("pub const ENCODE_CASES: &[(u128, &str)] = &[")
for n in ints:
    out.append(f'    ({n}, "{crockford(n)}"),')
out.append("];\n")
src = open(os.path.join(here, "..", "fixtures", "nested.py")).read()
out.append(f'/// The repo-relative path the fixture is pretended to live at.\npub const FIXTURE_PATH: &str = "{fixture_path}";')
out.append("/// (qualname as Python reports it, occurrence, expected id) for every def and class of the fixture, in source order")
out.append("pub const FIXTURE_DEFS: &[(&str, u32, &str)] = &[")
for q, o in definitions(src):
    out.append(f'    ("{q}", {o}, "{node_id(fixture_path, q, o)}"),')
out.append("];")
open(os.path.join(here, "..", "golden_data.rs"), "w").write("\n".join(out) + "\n")
print("wrote golden_data.rs:", len(cases), "id cases,", len(ints), "encode cases,", len(definitions(src)), "fixture defs")
