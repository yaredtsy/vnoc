# 10.01: CI and packaging

> **Status:** ✅ short: **how to build and run today** is real; **the CI workflows are not built yet** (M0, after the walking skeleton) · **Program:** release · **Milestones:** M0, ⭐ MVP, ⭐ 1.0

**Purpose:** How a commit becomes a VSIX per platform on both stores, pre-release first, and how to run it locally meanwhile.

**Builds on:** [02-06 distribution](../02-technology/06-distribution-and-integrations.md) · [02-07 release workflows](../02-technology/07-repo-and-drivers.md)

## Run it locally (works today)

Four things are built, in this order of dependence. Open the repo root in VS Code and press **F5**: the launch configurations (`.vscode/launch.json`) run the build task first.

```sh
cargo build -p vnoc                         # the core            -> target/debug/vnoc
(cd drivers/python && cargo build)          # the Python driver   -> drivers/python/target/debug/vnoc-driver-python
(cd webview && npm install && npm run build)     # the canvas      -> webview/dist
(cd extension && npm install && npm run build)   # the extension   -> extension/out
```

F5 starts a second window (the Extension Development Host). Open a Python folder in it (or use the launch configuration that opens the fixtures), run **"V-NOC: Open canvas"**, and the folders, files, classes and functions are drawn. Click a folder, file or class to expand it.

**Tests:** `cargo test` (the domain crate and the core, with the real driver behind it), `(cd drivers/python && cargo test)`, `(cd webview && npm test)`, and `(cd extension && npm test)`, which runs the whole chain inside a real VS Code window with a throwaway profile (it needs the other three built first).

## Two build facts that shape CI

- **`drivers/python` is its own Cargo workspace** with its own `rust-toolchain.toml` (1.98.0, because of pylspt) and its own `Cargo.lock` (ruff only compiles against the `salsa` pylspt is locked to). A cold build takes minutes (the dev build with ruff at opt-level 2 took about 2m40s; a release build with LTO about 7 minutes), so **CI caches `drivers/python/target`**, keyed by the lockfile and the pylspt revision.
- **Size.** The release driver is about 16 MB (8 MB zipped); the VSIX limit is 40 MB per platform (NFR-32).

## Will define (the CI part)

1. **Targets:** `darwin-arm64`, `darwin-x64`, `linux-x64`, `win32-x64` (MVP); `linux-arm64` (stable).
2. **Workflows:** build + test on every PR; `release-core` builds the core and the bundled driver, packages with `vsce package --target`, publishes to the Marketplace and Open VSX (`--pre-release` for 0.x). **M0's first workflow: one VSIX (macOS arm64) without the driver**; bundling the driver and copy-on-first-run are M1.
3. **Versioning:** odd minors = pre-release, even = release.
4. **Secrets:** Marketplace and Open VSX tokens.
5. **C5:** which store Cursor and Windsurf install from, checked here.
6. **macOS binaries must be at least ad-hoc signed** or the OS kills them on launch (rcodesign when cross-compiling from Linux, or macOS runners). This includes the binaries inside the VSIX.

## MVP vs stable

| MVP (0.x) | Stable (1.0) |
|---|---|
| four platforms, pre-release channel | fifth platform, release channel, a frozen driver protocol |

## Questions still open

- Cross-compile, or native runners per OS? (macOS signing and the 7-minute driver build both lean toward native runners.)
