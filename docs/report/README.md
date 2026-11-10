# V-NOC status report (2026-11-08)

A catch-up guide to the project: what V-NOC is, what is built, what is only planned, and which `plan_vs/` file to open for each topic.
It starts broad and gets more detailed file by file. Read it in order, or jump to the part you need.

> **New to VS Code extensions?** Every VS Code word is explained the first time it appears, and [`02-vscode-extension-basics.md`](02-vscode-extension-basics.md)
> covers them all in one place. Look for the **🧩 VS Code** boxes.

---

## The short version

- **What V-NOC is:** a VS Code extension that shows your Python code as a graph (folders → files → classes → functions, plus call trees) and lets you
  attach Markdown docs to any node. The docs live in a `.vnoc/` folder that you commit with your code.
- **Where we are:** Phase 1 (requirements and decisions) is **done**. Phase 2 (the spikes, which are small experiments) is **mostly done**: S1–S4 ran, S5 did not.
  Phase 3 (the real product) has **started**: a **walking skeleton** works from end to end.
- **What works today:** press F5 in VS Code → a second window opens → run **"V-NOC: Open canvas"** → the Rust core asks the Python driver to read every
  `.py` file → the canvas draws folders, files, classes and functions, and you can click to expand them.
- **What's next:** finish milestone **M0** (CI that builds a VSIX), then **M1** (SQLite cache, sidebar tree, Quick look, Go to code).
- **⚠ The plan docs are out of date in one big way.** The MVP was re-cut (draft row **VS-D78**): the MVP now **does** write an `ID:` line into your code when
  you attach a doc. `plan_vs/04-roadmap/` still says the opposite ("MVP never edits code, ID mode `none` only"). The new rows **VS-D78, VS-D79, VS-D80** exist only
  as drafts in `spikes/s4-git/DRAFT-doc-rows.md` and are not in the decision log yet. The full list: [`12-contradictions.md`](12-contradictions.md).
- **pylspt** (the Python analyzer under the driver) hasn't changed since 2026-11-03. The MVP needs small changes from it (a facade, pinning by `rev`, `plan_ids` taking
  V-NOC's IDs). Big repos will need lazy call-tree solving and real cancel later. Details: [`13-pylspt-whats-left.md`](13-pylspt-whats-left.md).
- **⚠ One test fails right now**, and the code is not the cause: an empty file `crates/vnoc-domain/tests/fixtures/te.py` (untracked, created 2026-11-08 10:04) makes the fixture
  folder have 2 files instead of 1. Delete it and the test passes again. Details: [`05-code-rust-core.md`](05-code-rust-core.md#tests).

## Status at a glance

```
PHASE 1  requirements + decisions      ██████████  done (8 review rounds, VS-D1…D77 in the log)
PHASE 2a spikes S1–S5                  ████████░░  S1 ✅  S2 ✅  S3 ✅  S4 ✅ (step 0 checks left)  S5 ❌ not run
PHASE 2b design docs (06-…10-)         ███░░░░░░░  4 written, 17 still outlines
PHASE 3  build the MVP (M0–M6)         █░░░░░░░░░  walking skeleton = most of M0, a little of M1/M2
```

## Reading order

| # | File | What you get | Time |
|---|---|---|---|
| 1 | [`01-big-picture.md`](01-big-picture.md) | what V-NOC is, the four programs, how they talk | 5 min |
| 2 | [`02-vscode-extension-basics.md`](02-vscode-extension-basics.md) | every VS Code idea the project uses, explained from zero | 10 min |
| 3 | [`03-progress-overview.md`](03-progress-overview.md) | phases and milestones, with ✅ / 🟡 / ❌, and the commit timeline | 5 min |
| 4 | [`04-spikes-results.md`](04-spikes-results.md) | what each experiment taught us and what is left of it | 10 min |
| 5 | [`05-code-rust-core.md`](05-code-rust-core.md) | `crates/`: the core binary, the domain types, the driver protocol | 10 min |
| 6 | [`06-code-python-driver.md`](06-code-python-driver.md) | `drivers/python`: the adapter around pylspt | 5 min |
| 7 | [`07-code-extension.md`](07-code-extension.md) | `extension/`: the TypeScript part that lives inside VS Code | 5 min |
| 8 | [`08-code-webview.md`](08-code-webview.md) | `webview/`: the React canvas | 5 min |
| 9 | [`09-plan-docs-guide.md`](09-plan-docs-guide.md) | **which `plan_vs/` file to read for what**, which ones are stale | 5 min |
| 10 | [`10-whats-left.md`](10-whats-left.md) | the to-do list, in order | 5 min |
| 11 | [`11-glossary.md`](11-glossary.md) | every term (VS Code + V-NOC) on one page | look-up |
| 12 | [`12-contradictions.md`](12-contradictions.md) | **every place the plan, the drafts and the code disagree**, with the file to fix and where the fix text is | 10 min |
| 13 | [`13-pylspt-whats-left.md`](13-pylspt-whats-left.md) | what V-NOC still needs from pylspt, and pylspt's own roadmap status | 10 min |

## How this report was made

From reading `plan_vs/` (all files), `spikes/` (READMEs, findings, the draft rows), the code in `crates/`, `drivers/python/`, `extension/`, `webview/`, the git history,
and running the tests (`cargo test --workspace`, `drivers/python` tests, `webview` tests). The extension's own test (`extension/npm test`, which opens a real VS Code window) was **not** run.
Nothing in `plan_vs/` was changed.
