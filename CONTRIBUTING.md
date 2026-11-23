# Contributing

Thank you for looking. This page is short; the design notes in [`docs/`](docs/README.md) go deeper.

## The layout

- `crates/`: the Rust workspace (the core, the domain types, the driver protocol)
- `drivers/<language>/`: one folder per language driver (`python`, `bend`), each a separate Cargo workspace with its own toolchain
- `extension/`: the VS Code extension (esbuild)
- `webview/`: React + Vite, builds into `extension/media/webview`
- `shared/`: TypeScript types and logic used by the extension and the webview
- `tools/media/`: records the README's pictures from a real VS Code window

## What you need

- [rustup](https://rustup.rs). Two toolchains, pinned by `rust-toolchain.toml`: `1.94.1` at the root and `1.99.1` in each `drivers/<language>`. Run
  `rustup toolchain install` in each folder.
- Node 22 and pnpm (`corepack enable`).
- For `scripts/notices.mjs` (the third-party notices): `cargo install cargo-about --locked --features cli`.

## Build

```
pnpm install
cargo build
for d in drivers/*/; do (cd "$d" && cargo build); done
pnpm -r build
```

Then press **F5** with *"V-NOC: run the extension, open the Python fixtures"* (`.vscode/launch.json`).

## Test

`.github/workflows/ci.yml` runs these, in this order:

1. `pnpm typecheck` (after `cargo fmt --check` and `cargo clippy -D warnings` in every workspace)
2. `pnpm --filter @vnoc/shared test`: session, tree, focus stack
3. `pnpm --filter vnoc-webview test`: layout, keys, markers, markdown
4. `pnpm --filter vnoc unit`: the extension's unit tests
5. `cargo test -p vnoc-domain -p vnoc-driver-protocol`: the core
6. `cargo build && cargo test` in each folder of `drivers/`: the drivers
7. `cargo test -p vnoc`: core integration tests (they need the drivers built above)
8. `pnpm -r build`, then `pnpm --filter vnoc test`: the suite in a real VS Code window

## Traps

- `ELECTRON_RUN_AS_NODE`: run the window suite as `env -u ELECTRON_RUN_AS_NODE pnpm -r test`.
- Windows: the driver's vendored ruff has long paths. `git config --system core.longpaths true` and `CARGO_NET_GIT_FETCH_WITH_CLI=true`.
- A release build of the driver needs about 5 GB of disk.

## The files users commit are frozen

`.vnoc/`, the ID line and the markers are in people's repositories. A change there needs a doc and a fixture test first
(`docs/02-mvp-and-releases.md` §1, §4).

## Commits and docs

One step per commit, `step x.y: what it leaves working`. A step updates its doc's "as built" note. The conventions are at the end of
[`docs/README.md`](docs/README.md).

## Adding a language

A language is one driver program: [docs/06-drivers.md](docs/06-drivers.md).
