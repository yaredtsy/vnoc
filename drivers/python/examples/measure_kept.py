#!/usr/bin/env python3
"""Times `vnoc/callTree` through the real driver binary, on a project (docs/14 §11, step 8.2a).

  python3 examples/measure_kept.py target/release/vnoc-driver-python <root>

Prints: the first request on a driver (it opens the project and walks), then 20 more functions on the same driver (the project is kept open and
refreshed before each), then the same two big entries again, and the numbers of the worker (`vnoc/test/stats`: one open, no reopen).
The functions are the first 20 defs of kind function with at least one call in the package folder, in file order, so the list is the same every run.
"""
import json, statistics, subprocess, sys, time
from pathlib import Path

binary, root = sys.argv[1], str(Path(sys.argv[2]).resolve())

def frame(o):
    b = json.dumps(o).encode()
    return b"Content-Length: %d\r\n\r\n" % len(b) + b

def read(f):
    n = int(f.readline().split(b":")[1]); f.readline()
    return json.loads(f.read(n)), n

class Driver:
    def __init__(self):
        import os
        self.p = subprocess.Popen([binary], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, env={**os.environ, "VNOC_TEST_HOOKS": "1"})
        self.id = 0
        self.call("initialize", {"protocol": 1})
        self.p.stdin.write(frame({"jsonrpc": "2.0", "method": "initialized", "params": {}})); self.p.stdin.flush()

    def call(self, method, params):
        self.id += 1
        self.p.stdin.write(frame({"jsonrpc": "2.0", "id": self.id, "method": method, "params": params})); self.p.stdin.flush()
        msg, size = read(self.p.stdout)
        return msg, size

    def tree(self, file, qualname, occurrence=1):
        t = time.perf_counter()
        msg, size = self.call("vnoc/callTree", {"root": root, "file": file, "entry": {"qualname": qualname, "occurrence": occurrence}})
        ms = (time.perf_counter() - t) * 1000
        if "error" in msg:
            return ms, None, size
        return ms, nodes(msg["result"]["root"]), size

    def stop(self):
        self.call("shutdown", None)
        self.p.stdin.write(frame({"jsonrpc": "2.0", "method": "exit"})); self.p.stdin.flush(); self.p.wait()

def nodes(n):
    return 1 + sum(nodes(c) for c in n["children"])

def entries(driver):
    out = []
    for path in sorted(Path(root, "httpx").glob("*.py")):
        file = path.relative_to(root).as_posix()
        msg, _ = driver.call("vnoc/outline", {"root": root, "file": file})
        seen = {}
        for d in msg["result"]["defs"]:
            seen[d["qualname"]] = seen.get(d["qualname"], 0) + 1
            if d["kind"] == "function" and "<locals>" not in d["qualname"]:
                out.append((file, d["qualname"], seen[d["qualname"]]))
    return out

def show(label, file, q, result):
    ms, n, size = result
    print(f"  {label:<10} {file}::{q:<34} {ms:7.1f} ms  nodes {n if n is not None else 'error':>5}  response {size/1024:6.0f} KB")

d = Driver()
everything = entries(d)
BIG = [("httpx/_api.py", "request", 1), ("httpx/_client.py", "Client.send", 1)]
print(f"project {root}: {len(everything)} functions in httpx/*.py")

print("\nthe first request on a driver (opens the project, then walks):")
show("1st", *BIG[0][:2], d.tree(*BIG[0]))
print("\nthe same entry again, and the second big one (the project is kept):")
show("2nd", *BIG[0][:2], d.tree(*BIG[0]))
show("1st", *BIG[1][:2], d.tree(*BIG[1]))
show("2nd", *BIG[1][:2], d.tree(*BIG[1]))

picked = []
for e in everything:
    if e[:2] in [b[:2] for b in BIG]:
        continue
    ms, n, size = d.tree(*e)
    if n and n > 1:
        picked.append((e, ms, n, size))
    if len(picked) == 20:
        break
print("\n20 more functions on the same driver, warm (each had at least one call):")
for (file, q, _), ms, n, size in picked:
    show("warm", file, q, (ms, n, size))
times = [p[1] for p in picked]
print(f"\nwarm, 20 functions: median {statistics.median(times):.1f} ms, p90 {sorted(times)[17]:.1f} ms, slowest {max(times):.1f} ms")
stats, _ = d.call("vnoc/test/stats", {})
print("worker:", stats["result"])
d.stop()

print("\nfor comparison, a driver with nothing kept: a new process per request (the project is opened and walked cold each time):")
for e in BIG + [p[0] for p in picked[:3]]:
    cold = Driver()
    show("cold", e[0], e[1], cold.tree(*e))
    cold.stop()
