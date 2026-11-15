# 18. Release 0.2: merge, pictures, pages, a way to tell us

```
release 0.2
├── 1. what "released" means for 0.2
├── 2. what exists today, and what is missing
├── 3. decisions (answer the ask rows before R1)
├── 4. the steps (R0 – R7), in order
├── 5. the exit checks
└── the files
    ├── 01-media.md              the capture tool: demo project, shots, GIFs, timing, how they are made
    ├── 02-pages.md              root README, the Marketplace README, CONTRIBUTING, SECURITY, issue forms, CHANGELOG
    ├── 03-help-and-report.md    Report Issue, Show Log, Discord, Star, Licence: where each lives and what it sends
    └── 04-gifs-v2.md            the GIFs again after the first two were watched: the script first, record freely, fix it in the edit (replaces 01 §5; changes §4.3, §7.2)
```

Written 2026-11-15 on `0.2-call-trees` at `0b585ff` (8.8 done, 71 commits ahead of `main`).

## 1. What "released" means for 0.2

A tag `v0.2.0` on `main` puts four VSIX files on the Marketplace (pre-release), on Open VSX if its token is set, and on a GitHub release with
notes. Someone who lands on the GitHub repo or the Marketplace page **sees what it does in ten seconds** (a GIF), can install it, can build it,
and has **one obvious way to report a problem** from inside VS Code.

## 2. What exists today, and what is missing

| Area | Today | Missing |
|---|---|---|
| publish pipeline | `publish.yml` (reworked in PR #2, [`19-ci-cd.md`](../19-ci-cd.md)): tag check, **including a `## <version>` heading in `extension/CHANGELOG.md`** → tests and packaging → Marketplace (OIDC) → Open VSX (if `OVSX_PAT`) → GitHub release with **the CHANGELOG section as its notes** | nothing; the CHANGELOG heading must be exactly `## 0.2.0` or the release stops at the check |
| version | `extension/package.json` and every `Cargo.toml` say `0.1.0`; the core reports its Cargo version in `serverInfo` (`crates/vnoc/src/server.rs:253`) | `0.2.0` everywhere: a bug report shows the core's version, and `0.1.0` there would mislead |
| CHANGELOG | "Unreleased (0.2)" lists the menu changes only | call cards, Explore, the webview sidebar, Go to Call Site, cancel (02 §6) |
| pictures | three `<!-- SCREENSHOT -->` placeholders in `extension/README.md`; no `media/screenshots/` | the tool and the files (01) |
| root README | nine lines | the front door (02 §1) |
| CONTRIBUTING / SECURITY / issue forms | none | 02 §3 – §5 |
| `package.json` links | `repository` only | `bugs`, `homepage`, `qna` (03 §6) |
| report a problem | *Show Log* in the palette; the sidebar's error state has Try Again and Show Log (16/06 §8) | Report Issue, Help and Feedback, a crash notification with Report (03) |
| compatibility | `docs/02` §4 item 4: "every release keeps a fixture repo from the previous release … it starts with 0.2" | `tests/fixtures/vnoc-0.1/` and its test (R4) |
| third-party notices | the VSIX ships `LICENSE` only; the driver binary contains ruff / ty (MIT), salsa and more (`language_driver_rs/NOTICE`) | a `THIRD_PARTY_NOTICES` file in the VSIX (R5) |

## 3. Decisions

| # | Question | Recommendation | Status |
|---|---|---|---|
| **D1** | Where does the repo live: `yaredtsy/vnoc` (today, in `package.json`) or the `v-noc` org (the old project is `v-noc/IDE`)? | **Decide before R1**: image URLs, issue links and the Marketplace's Trusted Publishing policy (tied to the repo and `publish.yml`) all name it. Moving later = new policy + new links. If you want the org, move now. | **ask** |
| **D2** | The demo project | **httpx at a pinned tag** (01 §2): known to every Python developer, real call depth, already measured (14 §11). Not minicart (too small), not `language_driver_rs` (analyzer fixtures, not an app), not Jarvis (mostly benchmark data) | decided 2026-11-15 |
| **D3** | Asking for a GitHub star | **A menu entry and a README line, never a pop-up** (03 §4) | **ask** |
| **D4** | A *Get Started* walkthrough (`contributes.walkthroughs`) in 0.2 | **Later (0.2.1)**: it needs its own images inside the VSIX and its own review; 0.2 is already large | **ask** |
| **D5** | Sentry / telemetry | **Not in 0.2.** Report Issue (03) first; opt-in telemetry behind `telemetry.telemetryLevel` as its own step later | decided 2026-11-15 |
| **D6** | Merge style | a **merge commit** (keeps the `step x.y:` history `docs/` refers to) | **ask** |

## 4. The steps

Each step is one commit (or a few with the same prefix) and leaves the branch green (`env -u ELECTRON_RUN_AS_NODE pnpm -r test`, `cargo test`, the
driver's tests). **Stage paths by name; never `git add docs` or `git add .`** (`docs/futureplan/` has uncommitted work of its own).

| Step | What | Proof | Commit |
|---|---|---|---|
| **R0** | the version: `0.2.0` in `extension/package.json`, the root `Cargo.toml` `[workspace.package]`, `drivers/python/Cargo.toml`; `Cargo.lock`s follow | `pnpm typecheck`, `cargo build` in both workspaces; *V-NOC: Show Log* says `vnoc 0.2.0` | `release 0.2 R0: version 0.2.0` |
| **R1** | **help and report**, 03 (code: commands, menus, the error state, the crash notification, `package.json` links) | 03 §8's tests; by hand, Report Issue opens a filled form | `release 0.2 R1: Report Issue, Help and Feedback` |
| ~~R2~~ | **Skipped for 0.2 (your call, 2026-11-15); do it before any release that changes `.vnoc/` or the ID lines**. **The 0.1 fixture**: a small repo whose `.vnoc/` and ID lines were written by the 0.1.0 VSIX, checked in under `crates/vnoc/tests/fixtures/vnoc-0.1/`; a lifecycle test opens a copy with today's core and checks every doc is found, no Missing, **no file changed on disk** (hash before/after) | the test | `release 0.2 R2: the 0.1 fixture opens unchanged` |
| **R3** | **third-party notices**: a script (`cargo about generate` **[check]** or `cargo-bundle-licenses`) writes `extension/THIRD_PARTY_NOTICES.md` for both Cargo workspaces and the bundled npm code; `.vscodeignore` keeps it; `package.mjs` fails if it is missing | `vsce ls` lists it | `release 0.2 R3: third-party notices in the VSIX` |
| **R4** | **the capture tool and the pictures**, 01 | the files in `extension/media/screenshots/`; you reviewed every frame (01 §8) | `release 0.2 R4: the capture tool; screenshots and GIFs` |
| **R5** | **the pages**, 02 (both READMEs, CONTRIBUTING, SECURITY, issue forms, CHANGELOG) | 02 §8's checks | `release 0.2 R5: the pages` |
| **R6** | **merge**: PR `release-0.2` → `main` (`0.2-call-trees` is already in `main`, PR #1); CI green on all three systems; then `publish` by hand with `publish: false` (the dry run) on `main`; install the four VSIX files: macOS arm64 (yours), **Linux x64 and Windows x64** (a VM or a friend), Intel Mac if you can (it is cross-built and never tested) | §5 | (the merge) |
| **R7** | **tag and publish**: `git tag v0.2.0` on `main` (the notes come from the CHANGELOG, not the tag); push the tag; approve the `release` environment; then the GitHub settings of 02 §7 and the announcement (02 §9) | §5 | — |

Order matters in one place: **R1 before R4**, so the pictures show the final sidebar error state and menus; **R4 before R5**, so the pages link
pictures that exist at the tag.

## 5. The exit checks

- [ ] In a clean VS Code: Extensions → "V-NOC" → *Install Pre-Release Version* → `0.2.0`; open httpx; Show Calls on `Client.send` draws call cards.
- [ ] The Marketplace page and the GitHub README show the GIF and every screenshot (no broken image, no SVG in the Marketplace README).
- [ ] *V-NOC: Report Issue…* opens a GitHub form with the versions filled in; *Help and Feedback…* lists every link and each one opens.
- [ ] The GitHub release `v0.2.0` has four VSIX files and the 0.2 notes.
- [ ] A repo made by 0.1.0 opens in 0.2.0 with every doc found and no file changed: **once by hand** (R2's test was skipped).
- [ ] The Discord invite in the README still works (it is the one from the old project: `https://discord.gg/J5nfPHqyBr`) **[check]**.
