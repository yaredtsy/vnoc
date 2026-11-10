# V-NOC

Browse code as a graph and attach docs to it. Layout and rationale: `docs/fable_plan/03-repo-structure.md`.

- `crates/` Rust workspace (core, domain types, driver protocol)
- `drivers/python/` separate Cargo workspace (own toolchain)
- `extension/` VS Code extension (esbuild)
- `webview/` React + Vite, builds into `extension/media/webview`
- `shared/` TypeScript types used by extension and webview
