# 02.07: The monorepo and language drivers as downloadable executables

Decisions VS-D27 (monorepo), VS-D28 (every driver is its own executable), VS-D29 + its six requirements
(the core downloads drivers), VS-D30 (v1 bundles Python but uses the same lookup path).

---

## 1. Monorepo layout (VS-D27) 🟡 names

```
vnoc/                                  (today: vnoc_playground)
├── Cargo.toml                         [workspace]; default-members = the core only (see the 🦀 note)
├── crates/
│   ├── vnoc-domain/                   ids, nodes, node.toml, hashes          (plan/01)
│   ├── vnoc-store/                    storage traits + WritePlan             (plan/03)
│   ├── vnoc-store-sqlite/             the SQLite cache adapter
│   ├── vnoc-store-memory/             test adapter + contract suite
│   ├── vnoc-userdata/                 .vnoc/ shared + local text files, the placement rules (VS-D24)
│   ├── vnoc-sync/                     scan · diff · reconcile · call layers  (plan/05)
│   ├── vnoc-app/                      use cases
│   ├── vnoc-protocol/                 editor ↔ core messages (+ generated TS types)
│   ├── vnoc-driver-protocol/          core ↔ driver messages, PROTOCOL_VERSION
│   ├── vnoc-driver-host/              find · install · verify · spawn · talk to drivers
│   └── vnoc/                          the core binary:  vnoc lsp | mcp | query | driver …
├── drivers/
│   └── python/                        vnoc-driver-python: a thin binary around pylspt
├── extension/                         TS extension host code (esbuild)
├── webview/                           React + xyflow canvas (Vite)
├── plan/  plan_vs/
└── .github/workflows/
    ├── release-core.yml               tag core-v0.4.0      → VSIX per platform + `vnoc` binaries
    └── release-driver.yml             tag driver-python-v0.3.0 → signed archives + new manifest
```

**pylspt stays its own repo (VS-D65).** `drivers/python` depends on it as a git dependency, so pylspt keeps its own history and tests, and the monorepo only holds
the thin wrapper. Three conditions:

1. **pylspt must not depend on `vnoc-driver-protocol`** (or any other V-NOC crate). It keeps its own Rust API and types.
2. **The adapter in `drivers/python` translates** between the driver protocol and pylspt's API. A protocol change therefore touches only the monorepo (the protocol
   crate, the core, the adapter) in one commit, and never forces a pylspt release.
3. **Pinned by `rev`** (an exact commit), never a branch. For development, a **`[patch]`** points it at a local checkout.

```
vnoc-driver-protocol  ◄── drivers/python (adapter: protocol ⇄ pylspt API) ──► pylspt (own repo, knows nothing about V-NOC)
```

> **🦀 Rust note: pinning a git dependency, and `[patch]` for local work.**
> ```toml
> # drivers/python/Cargo.toml — what CI and releases build
> [dependencies]
> pylspt = { git = "https://github.com/yaredtsy/pylspt", rev = "3f2a9c1" }    # exact commit; `branch = "main"` would drift
> ```
> ```toml
> # .cargo/config.toml at the monorepo root — gitignored, so it only affects your machine
> [patch."https://github.com/yaredtsy/pylspt"]
> pylspt = { path = "../pylspt" }          # build against your working copy while changing both
> ```
> `[patch]` swaps a dependency's *source* everywhere in the build without touching any `Cargo.toml`. Putting it in a **gitignored** `.cargo/config.toml` (Cargo supports
> `[patch]` there) rather than in the committed root `Cargo.toml` means CI can never accidentally build against a path that only exists on your laptop.
> When both changes are done: push pylspt, bump `rev`, commit. `Cargo.lock` records the commit too.

One repo pays off most for the **editor ↔ core** protocol: a field added in `vnoc-protocol` changes the Rust side, the generated TS types,
the extension and the webview in **one commit**, and CI checks them together. The **core ↔ driver** protocol is versioned on purpose
(below), because drivers are released separately even though they live in the same repo.

> **🦀 Rust note: `default-members` keeps everyday builds fast.**
> pylspt vendors ruff/ty, so compiling it takes minutes. In the root `Cargo.toml`:
> ```toml
> [workspace]
> members         = ["crates/*", "drivers/*"]
> default-members = ["crates/*"]          # plain `cargo build` / `cargo test` skip the drivers
> ```
> `cargo build -p vnoc-driver-python` (or `--workspace`) builds the driver when you need it. All members still share one `Cargo.lock`
> and one `target/`, so versions of shared crates (serde, tokio) can't drift between core and drivers.

## 2. A driver is a separate executable (VS-D28)

```
VS Code ext host ──stdio──► vnoc (core) ──stdio──► vnoc-driver-python   (pylspt + its salsa actor, plan/04-02)
       CLI / MCP ─────────►       │       ──stdio──► vnoc-driver-ts        (later)
                                  └── spawns drivers from the global drivers folder, by protocol version
```

- **The core spawns drivers, not the extension.** The process tree is the same in VS Code, Cursor, the CLI and MCP.
- The **same framing** as editor ↔ core (JSON-RPC 2.0 with `Content-Length` headers, `02-02`). It replaces plan/P3's JSON-RPC over HTTP.
- The first message is a **handshake**: the core sends `initialize{protocol: 1, root, settings}`; the driver answers with the protocol it speaks,
  its languages (`["python"]`) and its capabilities (`outline`, `callTree`, `idEdits`, …). Mismatch → the core refuses and reports it.
- The calls are **coarse**: "outline these 40 files", "call tree for this entry, depth 3", "ID edits for this buffer text". So the process boundary
  adds little cost compared to the analysis itself. pylspt's warm salsa cache lives in the long-running driver process, as before.
- `LanguageDriver` (plan/04-01) stays the trait inside the core. There is now **one implementation**: `ProcessDriver`, which talks to any executable.
- A driver crash kills only that driver. The core restarts it and marks the files in flight as "retry".

## 3. The core downloads drivers (VS-D29), and how each requirement is met

### Where they live (VS-D29.5)

```
<data dir>/vnoc/drivers/
├── .lock                                 install lock (VS-D29.4)
├── state.json                            last-used time per version (for cleanup), machine-local
├── python/
│   ├── 0.3.0/  vnoc-driver-python  driver.toml  (driver.toml = name, version, protocol, platform)
│   └── 0.4.0/  …
└── typescript/
    └── 0.1.0/  …
```

`<data dir>` comes from the `dirs` crate: macOS `~/Library/Application Support`, Linux `~/.local/share`. On Windows use **`data_local_dir()`**
(`%LOCALAPPDATA%`), not the roaming one, so binaries don't sync between machines **[check]**.

**Cleanup:** a version is deleted only when it's not the newest compatible one for any installed core, nothing has used it for **30 days** 🟡,
and it isn't running.

### Choosing a version (VS-D29.1)

```rust
// in vnoc-driver-host
pub const SUPPORTED_DRIVER_PROTOCOLS: RangeInclusive<u32> = 1..=1;   // compiled into this core

fn pick(manifest: &Manifest, lang: &str, platform: Platform) -> Option<&Release> {
    manifest.releases(lang)
        .filter(|r| SUPPORTED_DRIVER_PROTOCOLS.contains(&r.protocol))   // only what THIS core can speak
        .filter(|r| r.platforms.contains_key(&platform))
        .max_by(|a, b| a.version.cmp(&b.version))                       // newest of those, never "latest" blindly
}
```

A driver that bumps to protocol 2 is invisible to old cores, so they keep using the last protocol-1 release. An existing install never breaks.

### The manifest (VS-D29.2, VS-D29.3)

A static file attached to a fixed GitHub release (e.g. a `drivers-manifest` release), downloaded from its **asset URL**. Asset downloads
don't count against the REST API limit. The URL is compiled into the core and can be overridden in settings (mirrors, companies).

```json
{
  "schema": 1,
  "generated": "2026-11-05",
  "drivers": {
    "python": [
      { "version": "0.3.0", "protocol": 1,
        "platforms": {
          "darwin-arm64": { "url": "https://github.com/…/vnoc-driver-python-0.3.0-darwin-arm64.tar.zst",
                            "sha256": "…", "size": 14211033 },
          "linux-x64":    { "url": "…", "sha256": "…", "size": 15022100 } } }
    ]
  }
}
```

Verification chain:

1. Download `manifest.json` **and** `manifest.json.minisig`. Check the signature against the **public key embedded in the core**. A bad signature means stop.
2. The signed manifest carries each archive's `sha256`. Because the manifest is signed, the hash is trustworthy, and the archive is checked against it.
3. Each archive also has its own `.minisig`, so **`vnoc driver install <file>`** (offline, VS-D29.6) can verify a file that came without the manifest.

Key rotation 🟡: the core embeds a *list* of accepted public keys, so a new key can be added in one release before the old one is retired.

> **🦀 Rust note: embedding a key.** `const KEYS: &[&str] = &[include_str!("../keys/vnoc-2026.pub")];`. `include_str!` reads the file
> **at compile time** and bakes its text into the binary, so there's nothing on disk for an attacker to replace.

### Installing (VS-D29.4)

```
take lock  drivers/.lock          (blocking with a timeout; fd-lock or fs4 crate [check])
  ├─ already installed and verified?  → release lock, done   (another process may have just installed it)
  ├─ download → drivers/.tmp-<random>/archive    (streamed, size-capped by the manifest's `size`)
  ├─ verify sha256 (+ signature for offline files)
  ├─ extract into drivers/.tmp-<random>/0.3.0/ ; chmod +x
  └─ rename drivers/.tmp-<random>/0.3.0 → drivers/python/0.3.0       ← atomic on the same filesystem
release lock
```

A crash at any point leaves at most a `.tmp-*` folder, which the next install deletes. Never a half-installed version.

### Proxies and offline (VS-D29.6)

| Where | Proxy source |
|---|---|
| VS Code / Cursor | the extension reads `http.proxy`, `http.proxyStrictSSL` (and `http.noProxy` **[check]**) and passes them in `initialize` |
| CLI / MCP | `HTTPS_PROXY`, `HTTP_PROXY`, `NO_PROXY` environment variables |
| Air-gapped | `vnoc driver install ./vnoc-driver-python-0.3.0-linux-x64.tar.zst` (the `.minisig` next to it is required) |

HTTP client: `ureq` or `reqwest` with rustls **[check]** in phase 2 (small, proxy support, no OpenSSL dependency).

### Trust

Downloading is always a **user choice** ("This project uses TypeScript. Install the TypeScript driver (12 MB)?"). It never happens just because
a cloned `vnoc.toml` lists a language. A signed driver only *reads* code, so it may run in an untrusted workspace (VS-D18's read-only indexing).

## 4. v1: bundled, but found the same way (VS-D30)

```
VSIX: bin/vnoc  +  drivers/python/0.3.0/vnoc-driver-python  +  its .minisig
          │
first run │  core: "python 0.3.0 not in <data dir>/vnoc/drivers/?"
          ▼  → install from the bundled file (same lock, same verify, same rename) ─► drivers/python/0.3.0/
             → run it from there
```

So in v1 the "source" of the file is the VSIX, and from the second language on the source can be the manifest. **Lookup, verification and
spawning are the same code from day one.** The VSIX size stays as in `02-06` (the core shrinks, the Python driver is the bulk).

## 5. To check in phase 2 **[check]**

- macOS: do downloaded, unsigned binaries need notarisation or a quarantine-attribute clear before they can run when spawned by a process?
- Windows: Defender/SmartScreen behaviour for binaries written to `%LOCALAPPDATA%` and spawned.
- Which lock crate behaves the same on all three OSes (and on network home directories).
- Archive format (`.tar.zst` vs `.zip` on Windows).

Next: [`../03-next/01-review-checklist.md`](../03-next/01-review-checklist.md)
