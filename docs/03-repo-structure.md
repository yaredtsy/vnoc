# 03. The repository: folders and the monorepo

```
repo
├── 1. today, and what is wrong with it
├── 2. the target layout
│   ├── Rust side      one workspace + one separate driver workspace
│   ├── TypeScript side  extension · webview · shared
│   └── everything else  docs · spikes · scripts
├── 3. crates vs modules: when to split
├── 4. inside crates/vnoc: the module map
├── 5. building and packaging
└── 6. what to do, in order
```

## 1. Today

```
vnoc_playground/
├── Cargo.toml          a workspace AND a package (vnoc_playground)     ← leftover from `cargo new`
├── src/main.rs         "Hello, world!"                                  ← leftover
├── crates/
│   ├── vnoc-domain/            ✅ pure types
│   ├── vnoc-driver-protocol/   ✅ wire types (only the core uses them)
│   └── vnoc/                   ✅ the core program
├── drivers/python/     ✅ its own workspace (own toolchain, own Cargo.lock)
├── extension/          ✅ TypeScript, compiled with tsc
├── webview/            ✅ React + Vite; output in webview/dist  ← outside the extension, cannot ship (R1)
├── plan/  plan_vs/  report/  fable_plan/      four doc folders at the top level
└── spikes/
```

The shape is right. A monorepo is the correct choice here, because the four programs change together and ship together. The problems are small:
two leftovers, an output folder in the wrong place, no shared home for TypeScript types, and docs crowding the top level.

## 2. The target layout

```
vnoc/                                   (rename the repo when you publish)
├── Cargo.toml                 workspace only: no [package]
├── Cargo.lock
├── rust-toolchain.toml        pin the Rust version for the core (CI and you build the same thing)
├── package.json               npm workspaces: extension, webview, shared
├── package-lock.json          ONE lockfile for all TypeScript
│
├── crates/
│   ├── vnoc-domain/           types everyone agrees on. No I/O.
│   ├── vnoc-driver-protocol/  the driver wire. Drivers depend on this.
│   └── vnoc/                  the core program (modules in §4)
│
├── drivers/
│   └── python/                separate Cargo workspace (pylspt pins its own toolchain)
│
├── extension/
│   ├── src/                   TypeScript sources
│   ├── dist/extension.js      esbuild output            (gitignored)
│   ├── media/
│   │   ├── vnoc.svg
│   │   └── webview/           the webview build lands HERE   (gitignored)
│   ├── bin/                   vnoc, vnoc-driver-python       (gitignored; filled by the package step)
│   └── package.json           the extension manifest
│
├── webview/
│   ├── src/
│   └── vite.config.ts         outDir: ../extension/media/webview
│
├── shared/
│   └── messages.ts            message and data types used by extension AND webview
│
├── scripts/
│   └── package.mjs            build everything → one .vsix
│
├── docs/
│   ├── plan/  plan_vs/  report/  fable_plan/     moved here, unchanged
│   └── …
├── spikes/                    throwaway, outside every workspace
└── .github/workflows/ci.yml
```

### The Rust side

> **🦀 Rust note: package, crate, workspace.**
> - A **crate** is one compiled unit: one library or one program.
> - A **package** is a folder with a `Cargo.toml`. It holds one library crate and/or some program crates.
> - A **workspace** is a group of packages that share one `Cargo.lock` and one `target/` folder, so they build with the same dependency versions and do not
>   compile the same dependency twice.
>
> Your root `Cargo.toml` is both a workspace and a package. That is allowed, but the package is empty noise. A root with only `[workspace]` is called a
> **virtual workspace**:
> ```toml
> [workspace]
> members  = ["crates/*"]
> exclude  = ["drivers", "spikes"]
> resolver = "3"
>
> [workspace.package]            # shared fields; each crate writes `edition.workspace = true`
> edition = "2024"
> version = "0.1.0"
>
> [workspace.dependencies]       # one place for versions; each crate writes `serde.workspace = true`
> serde = { version = "1", features = ["derive"] }
> ```
> `[workspace.dependencies]` matters more as crates multiply: you bump `serde` once.

**Why `drivers/python` stays a separate workspace.** It must compile with the exact Rust version and the exact `salsa` version that pylspt uses.
If it joined the main workspace, the core would be forced onto those pins too. Two workspaces means two `target/` folders and two lockfiles. That cost is
worth it, and your `Cargo.toml` comment already says so. Keep it.

### The TypeScript side

> **🧩 VS Code note: npm workspaces.** A root `package.json` with `"workspaces": ["extension", "webview", "shared"]` gives you one `npm install`, one
> lockfile, and lets `extension` and `webview` import from `shared` by name. The one trap: `vsce` (the packager) does not understand workspaces and will
> complain about `node_modules`. The fix is the one you need anyway (R3): bundle the extension with esbuild so it needs **no** `node_modules` at run time,
> and call `vsce package --no-dependencies`.

`shared/messages.ts` is the important new piece. Today the `TreeNode` shape exists three times (Rust, `structureView.ts`, `webview/src/types.ts`) and the
message names are strings in an `if` chain. One file that both TypeScript sides import removes two of the three copies today. The Rust copy can be generated
later (`ts-rs`) when the shapes stop moving. [`07-ui/01-state.md`](07-ui/01-state.md) §3 shows the file.

### Everything else

Moving the four doc folders under `docs/` is cosmetic. Do it in one commit with `git mv` when you have a quiet moment; links inside each folder are relative
and keep working. Links **between** folders (`plan_vs` → `plan`) also keep working because they move together.

## 3. Crates vs modules: when to split

> **🦀 Rust note: module vs crate.** A **module** (`mod index;` → `src/index.rs`) is a namespace inside one crate. It compiles together with the rest of the
> crate, and `pub(crate)` lets other modules of the same crate see things that outsiders cannot. A **crate** is a hard wall: only `pub` items cross it, it
> has its own `Cargo.toml`, and it compiles separately.
>
> Modules are cheap to create and cheap to reshape. Crates are neither: every move across a crate wall means changing `pub`, `Cargo.toml`, and imports.

The plan lists six more crates. My advice is to build all six as **modules of `crates/vnoc`** and promote one to a crate only when one of these is true:

| Reason to make a crate | Example in this project |
|---|---|
| A **second program** needs it | `vnoc-driver-protocol`: the core *and* every driver. ✅ already a crate |
| It must have **no heavy dependencies** | `vnoc-domain`: no tokio, no I/O, so it can be used anywhere. ✅ already a crate |
| A dependency is **slow to compile or optional** | a future `vnoc-store-sqlite` (bundled SQLite takes a while to build) |
| Compile times hurt | not yet |

Everything else (index, userdata writer, driver host, sync) has exactly one user: the core. Keep the walls soft until the design settles. You lose nothing:
turning a well-kept module into a crate later is mechanical.

## 4. Inside `crates/vnoc`: the module map

```
crates/vnoc/src/
├── main.rs            start-up, logging, the LSP main loop          (exists)
├── server.rs          the router: one line per message               (exists; keep it thin)
├── protocol.rs        editor ↔ core message types                    (exists)
├── watchdog.rs        handler timing                                 (exists)
│
├── project.rs         NEW  find the git root, load vnoc.toml, the scope
├── scan.rs            which files are in scope                       (exists; extensions come from drivers)
├── driver_host.rs     start, handshake, request, timeout, restart    (exists)
│
├── index/             NEW  the in-memory index            → 04-storage/02-index.md
│   ├── mod.rs              struct Index + its methods (the only way in)
│   ├── build.rs            full build, and update for a list of changed files
│   └── tree.rs             today's structure.rs: index → the tree the UI draws
│
├── userdata/          NEW  everything under .vnoc/        → 04-storage/03-writing-safely.md
│   ├── mod.rs              use-case functions: attach_doc, relink, remove…
│   ├── layout.rs           paths: nodes/<shard>/<id>/…  (one place that knows the layout)
│   ├── read.rs             load all node folders at start
│   └── atomic.rs           write-temp-then-rename
│
└── identity/          NEW  IDs and finding nodes          → 05-identity/
    ├── attach.rs           the attach flow (ID → edit → files)
    ├── resolve.rs          marker → anchor → Missing
    └── folder_marker.rs    .vnoc-id read / write
```

**The dependency rule** (arrows mean "may use"):

```
server ──► identity ──► userdata ──► vnoc-domain
   │           │
   │           └──────► index ─────► vnoc-domain
   └──► index, project, driver_host

Never the other way. `index` and `userdata` do not know each other, and neither knows `server`.
```

If `userdata` never imports `index`, you can test the whole `.vnoc/` writer with nothing but a temp folder. If `index` never imports `userdata`, the future
SQLite version only has to replace one module.

## 5. Building and packaging

One script, `scripts/package.mjs` (Node, so it runs the same on macOS, Linux and Windows):

```
1. cargo build --release -p vnoc                        → target/release/vnoc
2. (cd drivers/python && cargo build --release)         → drivers/python/target/release/vnoc-driver-python
3. npm run build -w webview                             → extension/media/webview/assets/index.{js,css}
4. npm run build -w extension   (esbuild)               → extension/dist/extension.js
5. copy the two binaries                                → extension/bin/
6. (cd extension && vsce package --no-dependencies --target <platform>)   → vnoc-<platform>-0.1.0.vsix
```

What ends up inside the VSIX (add everything else to `extension/.vscodeignore`):

```
extension/
├── package.json
├── dist/extension.js
├── media/vnoc.svg
├── media/webview/assets/index.js, index.css
└── bin/vnoc, bin/vnoc-driver-python
```

How the extension finds a binary, in order:

```
1. the setting (vnoc.server.path)        ignored in Restricted Mode, as today
2. <extension folder>/bin/vnoc           the installed VSIX
3. <repo>/target/debug/vnoc              development (F5), as today
```

Keep `.vscode/tasks.json` as it is for the F5 loop. The script is for releases and CI.

**macOS note:** a binary downloaded inside a VSIX can be blocked by Gatekeeper (spike item C11, still open). Test Step 0 by installing the VSIX on a second
Mac, or after downloading it through a browser, not only on the machine that built it **[check]**.

## 6. What to do, in order

| # | Change | Size |
|---|---|---|
| 1 | Root `Cargo.toml` → virtual workspace; delete `src/main.rs`; add `[workspace.dependencies]` | small |
| 2 | Vite `outDir` → `../extension/media/webview`; `distUri` follows (R1) | small |
| 3 | esbuild bundle for the extension (R3) | small |
| 4 | Binary lookup: setting → `bin/` → dev path (R2) | small |
| 5 | pylspt as a git dependency pinned by `rev` (R4) | medium **[check the vendored ruff]** |
| 6 | `scripts/package.mjs`; install the VSIX in a clean VS Code | medium |
| 7 | root `package.json` with npm workspaces; `shared/messages.ts` | small |
| 8 | CI workflow | medium |
| 9 | Move doc folders under `docs/` | small, any time |

Next: [`04-storage/`](04-storage/README.md)
