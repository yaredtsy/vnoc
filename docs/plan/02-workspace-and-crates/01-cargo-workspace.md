# 02.01 — The Cargo workspace

`vnoc_playground` today is a single binary crate (`src/main.rs`). Milestone M0
turns it into a **workspace**: one repo, several crates, one shared `Cargo.lock`
and one `target/` folder.

---

## Layout

```
vnoc_playground/
├── Cargo.toml                 # [workspace] only: members, shared deps, lints, profiles
├── plan/                      # this plan
├── proto/                     # .proto files (shared with the frontend codegen)
│   └── vnoc/v1/*.proto
├── crates/
│   ├── vnoc-domain/           # pure types (01-data-model)
│   ├── vnoc-store/            # storage TRAITS + WritePlan (03-storage)
│   ├── vnoc-store-terminus/   # TerminusDB adapter
│   ├── vnoc-store-memory/     # in-memory adapter (tests, and a reference implementation)
│   ├── vnoc-drivers/          # LanguageDriver, IdStrategy, python (pylspt) + jsonrpc impls
│   ├── vnoc-sync/             # ★ orchestrator, scheduler, watcher
│   ├── vnoc-app/              # application services (use cases)
│   ├── vnoc-proto/            # generated tonic/prost code (build.rs)
│   ├── vnoc-api/              # tonic service impls
│   └── vnoc-server/           # the binary: config + wiring
└── tests/                     # end-to-end tests (a real Terminus in docker, a fixture project)
```

## Who depends on whom

```
vnoc-server ──► vnoc-api ──► vnoc-app ──► vnoc-sync ──► vnoc-drivers ──► vnoc-domain
     │              │            │            │                ▲
     │              ▼            └────────────┴──► vnoc-store ─┘
     │          vnoc-proto                            ▲
     └──► vnoc-store-terminus ────────────────────────┤
     └──► vnoc-store-memory (dev/test only) ──────────┘
```

Rules:

- **Only `vnoc-server` depends on a concrete store.** Everything else sees `vnoc-store` traits.
- **`vnoc-proto` types don't leak below `vnoc-api`.** `vnoc-api` converts proto ↔ domain types.
  That way a change in the wire format doesn't ripple into business logic, and business types
  can have invariants proto types can't (e.g. `NodeId` is always valid, a proto `string` might not be).
- **`pylspt` is a dependency of `vnoc-drivers` only, behind a cargo feature `python`** (on by default).
  Building without it (`--no-default-features`) gives a fast compile for work on the store/API.

> **🦀 Rust note: cargo features.**
> A feature is a compile-time switch declared in `Cargo.toml`:
>
> ```toml
> [features]
> default = ["python"]
> python  = ["dep:pylspt"]          # turning on "python" pulls in the optional dependency
>
> [dependencies]
> pylspt = { path = "../../../pylspt", optional = true }
> ```
>
> and in code: `#[cfg(feature = "python")] mod python;`. Code under a disabled feature
> isn't compiled at all. Use features for "optional heavy dependency", not for runtime
> configuration.

## The root `Cargo.toml`

```toml
[workspace]
resolver = "3"                    # edition 2024's resolver
members  = ["crates/*"]

[workspace.package]
edition      = "2024"
rust-version = "1.9x"             # match pylspt's rust-toolchain.toml
license      = "Apache-2.0"

[workspace.dependencies]          # one version of each dep for the whole workspace
tokio        = { version = "1", features = ["full"] }
serde        = { version = "1", features = ["derive"] }
serde_json   = "1"
thiserror    = "2"
anyhow       = "1"
tracing      = "0.1"
# … see 02-third-party-crates.md

vnoc-domain  = { path = "crates/vnoc-domain" }
vnoc-store   = { path = "crates/vnoc-store" }
# … one line per internal crate

[workspace.lints.rust]
unsafe_code = "forbid"            # we have no reason for unsafe; make it a compile error

[workspace.lints.clippy]
unwrap_used = "warn"              # unwrap in library code = a panic waiting to happen
```

Each member then writes `tokio = { workspace = true }` and `[lints] workspace = true`.

> **🦀 Rust note: why `[workspace.dependencies]`?**
> Without it, crate A could use `serde 1.0.190` and crate B `serde 1.0.210`. Cargo
> usually unifies them, but for some crates (salsa, prost, tonic) two versions in one
> binary means two incompatible copies of the same types, and you get confusing errors like
> "expected `prost::Message`, found `prost::Message`". One table, one version.

## ⚠ pylspt and the vendored ruff

pylspt pins ruff/ty crates by path into `pylspt/vendor/ruff` and copies the shared
dependency versions (salsa, rustc-hash, …) from ruff's root `Cargo.toml`
(`pylspt/Cargo.toml` has comments about this). When `vnoc-drivers` depends on
`pylspt` by path, **those versions win**. If we add `salsa` or `rustc-hash` to our
workspace, use the **exact same versions** pylspt uses, or Cargo builds two copies.
Check with `cargo tree -d` (duplicates) in M0.

Also copy pylspt's `rust-toolchain.toml` and its `[profile.dev.package."*"] opt-level = 3`.
Debug-mode ty is very slow without it.

---

## Exit criteria (M0)

- `cargo build --workspace` and `cargo build --workspace --no-default-features` both pass.
- `cargo tree -d` shows no duplicate salsa/prost/tonic.
- `cargo tree -p vnoc-sync | grep terminus` prints nothing.

Next: [`02-third-party-crates.md`](02-third-party-crates.md)
