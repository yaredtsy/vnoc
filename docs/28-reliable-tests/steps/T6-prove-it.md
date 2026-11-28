# T6 — Prove it

## Goal
The promise of the README holds, and something watches that it keeps holding.

## What changes
- `VNOC_STRESS=1`: the window suite with a busy loop per CPU.
- The ledger job: started by hand, and nightly when `main` changed that day. The window suite 3 times per OS, retries off; the page suite 10 times. It lists every test that failed at least once. After a month with an empty list it runs weekly.
- `retries` is 0 everywhere; T0a's FLAKY block stays, so a later bridge is one variable.
- The tests of the tests ([04-design.md](../04-design.md) §8) are in the normal run.
- [03-rules.md](../03-rules.md) is linked from `CONTRIBUTING.md`.

## Done when
- 10 CI runs in a row on one commit are green on Ubuntu, macOS and Windows, retries off.
- Stress passes 5 times in a row on the Mac.
- No `sleep(`, no `process.env.CI` skip, no `NEEDS`.

## After
A test on the ledger is fixed or deleted within the week. It is never given a retry.
