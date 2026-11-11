# 13. Step 6: Harden and publish

Written 2026-11-11. It is the build plan for **Step 6** of [`02-mvp-and-releases.md`](02-mvp-and-releases.md): the last step before 0.1 is public.
Where this file is more exact, it wins. **Do Step 5.6 first** ([`11-missing-relink-remove.md`](11-missing-relink-remove.md) §11) and merge Step 5.

```
Step 6
├── 1. the goal, and what exists today
├── 2. accounts and keys you need            ← your part, start it first: some of it takes days
├── 3. what you must decide
├── 4. the refactor suite
├── 5. CI on three systems, four VSIX files
├── 6. the Marketplace listing
├── 7. the publish workflow, and the first run before 2026-11-13     ← there is a deadline
├── 8. order of work, each with its proof
├── 9. the first release, by hand
├── 10. the exit checks for 0.1
└── 11. what may go wrong
```

## 1. The goal, and what exists today

A tag `v0.1.0` on `main` gives four VSIX files, tested, on the VS Code Marketplace as a **pre-release**. Nobody builds or uploads by hand.

| | Today | After Step 6 |
|---|---|---|
| CI | `.github/workflows/ci.yml`: every push, **macOS only**, all tests, one VSIX as an artifact | the same tests on macOS, Linux and Windows |
| Packaging | `scripts/package.mjs`: one VSIX for the machine it runs on | four targets: `darwin-arm64`, `darwin-x64`, `linux-x64`, `win32-x64` |
| Publishing | nothing in the repo. On the Marketplace: the Trusted Publishing policy `vnoc-release` (repo `yaredtsy/vnoc`, workflow `publish.yml`), **inactive until its first use, which must be before 2026-11-13 17:56** | `.github/workflows/publish.yml`: a tag builds, publishes with no stored token, and makes a GitHub release |
| The listing | version `0.0.1`, publisher `vnoc`, no README, no icon, no licence, no repository | all there |
| Tests | rename and move are tested in the core only (`lifecycle.rs`) | the same in a real window, with VS Code and with `git mv` |
| The Python driver | `drivers/python`, on `pylspt` at one pinned commit | **unchanged.** The move to `language_driver_rs` comes after 0.1 |

## 2. Accounts and keys you need

Only you can make these. Nothing here goes into the repo. The Marketplace needs no token; the Open VSX token is a **GitHub secret**.

### VS Code Marketplace (needed): Trusted Publishing, no token

You set this up on 2026-11-11. The Marketplace trusts one workflow file of one repo; GitHub proves to it which workflow is running (OIDC).
There is **no token to store and none to renew**. The older way, an Azure DevOps Personal Access Token in a `VSCE_PAT` secret, is not used:
Microsoft retires those tokens on 2026-12-01.

| What | Value |
|---|---|
| the policy | `vnoc-release` |
| the repo | `yaredtsy/vnoc` |
| the workflow file | **`publish.yml`**: the file must have exactly this name, in `.github/workflows/`, and the `vsce publish` step must be **in this file**, not in a workflow it calls |
| state | **inactive.** It becomes active at the first successful publish from that workflow. **Deadline: 2026-11-13, 17:56 local time** (§7.1) |
| what the workflow needs | `permissions: id-token: write` on the publish job · Node 22 or newer · `@vscode/vsce` 4.1.0 or newer (the repo has `^4.1.0`) · `vsce publish --oidc` |
| the **publisher ID** | **`vnoc`** (the page shows `vnoc(vnoc)`: name, then ID). It is already in `extension/package.json` (`"publisher"`), and it is the first half of the extension's ID: `vnoc.vnoc` |

If the deadline passes, the policy does not activate. I do not know if it can then be used late or must be made again **[check]**: the page that
shows the policy says. Making one again is the same short form.

### Open VSX (for Cursor, Windsurf, VSCodium and the other editors built on VS Code)

Those editors cannot use Microsoft's Marketplace. They install from `open-vsx.org`. It is the same VSIX files.

| # | What | Where | What you get |
|---|---|---|---|
| 1 | an Eclipse account, linked to your GitHub login | `https://open-vsx.org` → log in with GitHub, then link the Eclipse account | the login |
| 2 | the Publisher Agreement | open-vsx.org → your profile | you may publish (one click, once) |
| 3 | an **access token** | open-vsx.org → Settings → Access Tokens | the token text, shown once |
| 4 | the **namespace** | once, on your machine: `npx ovsx create-namespace <publisher ID> -p <token>` | the same name as the publisher ID above |
| 5 | the secret | GitHub, as above | name it **`OVSX_PAT`** |

### GitHub

| What | Needed? |
|---|---|
| `GITHUB_TOKEN` | nothing to do: GitHub gives it to every workflow |
| a token for `yaredtsy/rust_python_parser` (pylspt) | **only if that repo is private.** Then the runners cannot fetch it: make a fine-grained token with read access to that one repo, and store it as the secret **`PYLSPT_TOKEN`**. If it is public, nothing |

### Not needed for 0.1

| What | Why not |
|---|---|
| an Apple Developer ID, a Windows code-signing certificate | VS Code unpacks the VSIX itself, so the two programs in `bin/` are not "downloaded files" to the system. Rust signs macOS arm64 programs ad hoc at link time. **[check]** on a clean Mac and a clean Windows machine in §10 |
| Visual Studio (the IDE), JetBrains | a different kind of extension. V-NOC is a VS Code extension only |

## 3. What you must decide

| # | Question | My suggestion |
|---|---|---|
| D1 | The publisher ID | **settled: `vnoc`.** The Marketplace page shows the publisher as `vnoc(vnoc)`: the name, then the ID in brackets. It is what `extension/package.json` already says, so the extension's ID stays `vnoc.vnoc` and no test changes |
| D2 | The licence | you must pick one: the Marketplace shows it, and the repo has no `LICENSE` file. `language_driver_rs` already has one: use the same unless you have a reason |
| D3 | Open VSX in 0.1? | yes: it is one more step in the workflow and it reaches Cursor users |
| D4 | Is the `vnoc` repo public at 0.1? | the listing links to it (`repository`). A private repo still publishes; the link is dead for others |
| D5 | More platforms (`linux-arm64`, `win32-arm64`, `alpine`) | after 0.1, when somebody asks |

## 4. The refactor suite

**Why:** the promise of 0.1 is "the doc is still there after a rename". The core tests prove it with files written by the test. This suite proves it
with what a person does: an edit in VS Code, a rename in the Explorer, a `git mv` in a terminal.

One new file, `extension/src/test/suite/refactor.test.ts`, with its own Python files in the test workspace, removed in `suiteTeardown`.
IDs are **on** (`format = 1` in `.vnoc/vnoc.toml`, put back after: see the "write id" suite in `attention.test.ts`).

| # | The change | How it is made | Proof |
|---|---|---|---|
| 1 | rename a function | a `WorkspaceEdit` on the open document, then save | the node has the new name, the same `node_id`, `docs == 1`, `link == "marker"` |
| 2 | move a function to another file | cut its text (with the `ID:` line) from one document, insert it in another, save both | the same, with the new `path` |
| 3 | rename a file | `WorkspaceEdit.renameFile` (what the Explorer does) | the **file** node keeps its `node_id` and doc (the `# FileID:` line moved with it); a documented function in it keeps its own |
| 4 | rename a file | `git mv`, run with `child_process.execFileSync` in the workspace | the same |
| 5 | rename a folder | `WorkspaceEdit.renameFile` on the folder | the **folder** node keeps its `node_id` and doc (the `.vnoc-id` moved with it); a documented file and function in it keep theirs |
| 6 | rename a folder | `git mv` | the same |

After each one: `attention().missing` is empty and `webviewErrors()` is empty. Nothing may show under Missing, even for a moment at the end.

| File | Change |
|---|---|
| `extension/src/test/runTest.ts` | after the fixture is copied: `git init`, `git add -A`, `git commit` in the workspace (with `-c user.name=… -c user.email=…`, the runners have no git identity). `git mv` needs a repo. **[check]** the other four suites still pass: the git root is now the workspace itself |
| `extension/src/test/suite/refactor.test.ts` | new, the six tests. Reuse `waitFor` from `attention.test.ts`: move it to `util.ts` first |
| `extension/src/test/suite/index.ts` | `mocha.addFile(… "refactor.test.js")` |
| `extension/package.json` | add `src/test/suite/refactor.test.ts` to the esbuild list of the `test` script |

The **no-data-loss suite** (Remove → Restore gives the same bytes) is already there from Step 5.4, in `attention.test.ts` and in `lifecycle.rs`.
Nothing to build: §5 makes it run on three systems.

## 5. CI on three systems, four VSIX files

### `scripts/package.mjs`

| Change | Detail |
|---|---|
| a Rust target per VSIX target | `darwin-arm64 → aarch64-apple-darwin`, `darwin-x64 → x86_64-apple-darwin`, `linux-x64 → x86_64-unknown-linux-gnu`, `win32-x64 → x86_64-pc-windows-msvc`. Pass `--target <triple>` to both `cargo build` calls, and copy from `target/<triple>/release/`. An unknown `--target` is an error with the list |
| no `--allow-missing-repository`, no `--skip-license` | once §6 is done they hide real mistakes |

`darwin-x64` is **cross-built** on the arm64 Mac runner (`rustup target add x86_64-apple-darwin` for both toolchains): the Intel Mac runners are
going away. It is built and packaged, not tested.

### `.github/workflows/ci.yml`

One job becomes a matrix. Every row runs the full list of steps that the job runs today.

| Runner | Tests | Packages |
|---|---|---|
| `macos-latest` (arm64) | all | `darwin-arm64`, `darwin-x64` |
| `ubuntu-22.04` | all | `linux-x64` |
| `windows-latest` | all | `win32-x64` |

| Detail | Why |
|---|---|
| `ubuntu-22.04`, not `latest` | the programs then run on older Linux too (an older glibc) |
| the window suite on Linux runs under `xvfb-run -a` | VS Code needs a display |
| `fail-fast: false` | one broken system must not hide the other two |
| a step `rustup toolchain install` in the root and in `drivers/python` | there are two toolchains (`1.94.1` and `1.98.0`); do not rely on rustup installing them by itself **[check]** |
| `Swatinem/rust-cache@v2` with `workspaces: ". -> target` and `drivers/python -> target"` | the driver build is slow (ruff) |
| if `PYLSPT_TOKEN` is set (§2) | `git config --global url."https://x-access-token:${{ secrets.PYLSPT_TOKEN }}@github.com/".insteadOf "https://github.com/"` before the first `cargo`, and `CARGO_NET_GIT_FETCH_WITH_CLI=true` |
| the upload step names the artifact `vsix-<target>` | four files, four names |

**Expect real failures on Linux and Windows the first time.** The tests have only ever run on macOS. Likely places: paths with `\` in test
asserts, `LOCAL_VSCODE` in `runTest.ts` (macOS path, it falls back to a download elsewhere), line endings (`.gitattributes` with `* text=auto eol=lf`
for the fixtures), the case-only rename logic of Step 5.1b. Fix the product where the product is wrong; fix the test where only the test is.

## 6. The Marketplace listing

| File | Change |
|---|---|
| `LICENSE` (repo root) | the licence of D2 |
| `extension/package.json` | `"version": "0.1.0"` · `"publisher"` (D1) · `"license"` (the SPDX name) · `"repository": { "type": "git", "url": "https://github.com/yaredtsy/vnoc" }` · `"icon": "media/icon.png"` · `"categories": ["Visualization", "Other"]` · `"keywords"` (docs, graph, python, documentation, canvas) · `"preview": true` · a real `"description"` (it says "walking skeleton" today) |
| `extension/media/icon.png` | 256 × 256 PNG. The Marketplace does not take SVG for the icon. Your part, or made from `media/vnoc.svg` |
| `extension/README.md` | the listing page: what it is in three lines · one screenshot or GIF of the canvas with a doc · how to start (open a Python folder, the V-NOC icon, Add Doc) · what `.vnoc/` is and that you commit it · what 0.1 does not do yet · Python only. Images must be `https://` links, not files in the VSIX |
| `extension/CHANGELOG.md` | `## 0.1.0`: the docs loop |
| `extension/.vscodeignore` | add `!README.md`, `!CHANGELOG.md`, `!LICENSE`, `!media/icon.png` (the file is a white-list) |
| `scripts/package.mjs` | copy the root `LICENSE` into `extension/` before `vsce package` |

Versions: the Marketplace takes only `major.minor.patch`. Every upload needs a higher number than the last one. All of 0.x goes out with
`--pre-release`; the first version without it is the first "stable" one for users who did not opt in.

## 7. The publish workflow

One file, **`.github/workflows/publish.yml`** (the name the policy trusts, §2). It is built in two steps: a small one now, to use the policy
before its deadline, and the full one at the end of Step 6.

### 7.1 The first run, before 2026-11-13 17:56 (step 6.0)

**What it is:** the first public pre-release, for macOS arm64 only, of what is on `main` after Step 5.6. It is honest about what it is:
`"preview": true`, a pre-release, a README of ten lines. It exists so that the policy becomes active and so that every later release goes the
same road. **Aim for 2026-11-12**, not the 13th: parts of this are new and the first run may fail (see "if it fails").

```
publish.yml   on: workflow_dispatch          (you press "Run workflow" on the Actions page; no tag yet)
  one job, macos-latest, permissions: { contents: read, id-token: write }
    the steps of ci.yml up to and with the tests          a red test stops the publish
    node scripts/package.mjs --pre-release                 → vnoc-darwin-arm64.vsix
    npx --no @vscode/vsce publish --oidc --packagePath vnoc-darwin-arm64.vsix        (run in extension/)
```

| File | Change |
|---|---|
| `.github/workflows/publish.yml` | new, as above. Copy the steps from `ci.yml` (do not call it: the publish step must be in this file). `actions/setup-node` with `node-version: 22` |
| `scripts/package.mjs` | a `--pre-release` flag, passed on to `vsce package`. **A VSIX is a pre-release because it was packaged as one**: the flag on `vsce publish` does nothing for a file given with `--packagePath` **[check]** |
| `extension/package.json` | `"publisher"`: yours (D1) · `"version": "0.0.1"` stays (0.1.0 is for the end of Step 6, and must be higher than this) · `"preview": true` · `"description"`: "Browse Python code as a tree and a canvas, and attach docs that stay with the code when it is renamed or moved." · `"repository"` |
| `extension/README.md` | ten lines: what it is · **pre-release, macOS (Apple silicon) only for now, Python only** · how to start · `.vnoc/` is committed. The full page comes in 6.3 |
| `extension/.vscodeignore` | `!README.md` |
| the five `getExtension("vnoc.vnoc")` in `extension/src/test/suite/` | only if the publisher ID is not `vnoc` |

The licence (D2) can wait for 6.3: `--skip-license` stays in `package.mjs` until then. Without a `LICENSE` file the listing shows no licence.

**If it fails**

| Sign | Then |
|---|---|
| the Marketplace refuses the login (`TrustedPublishingNotSupported…`, 401, "no matching policy") | check, in this order: the file is named `publish.yml` · the run is in `yaredtsy/vnoc`, not a fork · `id-token: write` is on the job · `"publisher"` is the policy's publisher. vsce does not fall back to a token |
| vsce refuses `--oidc` with `--packagePath` | publish from the folder instead, in `extension/` after `package.mjs` has filled `bin/`: `npx --no @vscode/vsce publish --oidc --pre-release --no-dependencies --target darwin-arm64 --allow-missing-repository --skip-license` |
| "version already exists" | a first try got further than it looked: raise the patch number (`0.0.2`) and run again |

**Done when** the policy page says **Active**, and "V-NOC" installs from the Extensions view of a clean VS Code on your Mac
(*Install Pre-Release Version*).

### 7.2 The full workflow (step 6.4)

The same file grows. It runs on a tag `v*` and still on `workflow_dispatch`.

```
tag v0.1.0
  1  check    the tag is on main · "v" + extension/package.json's version == the tag      else stop
  2  build    the matrix of §5, tests included → four VSIX artifacts (packaged with --pre-release)
  3  publish  needs: build · once, on ubuntu · permissions: { contents: write, id-token: write } · environment: release
       download the four artifacts
       npx --no @vscode/vsce publish --oidc --skip-duplicate --packagePath *.vsix
       npx ovsx publish --packagePath *.vsix                                env OVSX_PAT    (D3)
       gh release create v0.1.0 *.vsix --prerelease --notes-from-tag
```

- The build job may be shared with `ci.yml` (`workflow_call`). The **publish job stays in `publish.yml`**.
- `environment: release`: in GitHub → Settings → Environments you can ask for your approval before the job runs. One more look before
  something public happens.
- Publishing cannot be undone cleanly: a version number, once used, is used. A bad release is fixed by the next patch number.
- `--skip-duplicate` makes a second run safe when one store worked and the other did not. **[check]** that `ovsx` has the same.
- The login from GitHub lasts 15 minutes: the publish job must do nothing slow before `vsce publish`.

## 8. Order of work

Each row is one commit and leaves `main` green.

| # | Build | Proof |
|---|---|---|
| **6.0** | **before 2026-11-13 17:56**: `publish.yml` for macOS arm64, and the first pre-release (§7.1). Needs Step 5.6 merged and D1 | the policy page says Active · the extension installs from the Marketplace on your Mac |
| **6.1** | the refactor suite (§4) | the six tests pass in the window suite on your Mac, with the four old suites |
| **6.2** | `package.mjs` targets · the CI matrix (§5) · the fixes it asks for | CI is green on the three runners; four `vsix-*` artifacts on the run page |
| **6.3** | the listing (§6) | `node scripts/package.mjs` works with no `--allow-…` flag · install the VSIX in a clean VS Code: the Extensions view shows the icon, the README and the licence |
| **6.4** | the full `publish.yml` (§7.2) | run it by hand (`workflow_dispatch`) with the publish steps skipped by an input `publish: false`: the four files are built · a tag that does not match the version stops at step 1 |
| **6.5** | the first release (§9) | the extension is on the Marketplace as a pre-release, and it installs from there |
| **6.6** | two weeks of use on this repo and on pylspt · the exit checks (§10) | the list in §10 is ticked |

6.0 comes first because of the deadline; it needs D1. 6.3 needs D2. 6.5 needs `OVSX_PAT` if D3 is yes. 6.1 needs nothing from §2.

## 9. The 0.1.0 release (step 6.5)

The first public run was 6.0. This is the one that counts.

```
1  on your Mac:  node scripts/package.mjs --pre-release
2  install vnoc-darwin-arm64.vsix in a clean profile:  code --profile clean --install-extension vnoc-darwin-arm64.vsix
3  open a Python project that is not this repo · Add Doc · rename the function · the doc is still there
4  extension/package.json: "version": "0.1.0" · CHANGELOG.md · commit on main
5  push the tag v0.1.0 · approve the release job · watch it
6  in a clean VS Code:  Extensions → search "V-NOC" → Install Pre-Release Version → step 3 again
7  the same in Cursor (Open VSX), and on a Linux and a Windows machine if you have one
```

## 10. The exit checks for 0.1

From `02-mvp-and-releases.md`. The first is automatic; the others are done by hand once and written down here with the date.

| # | Check | How |
|---|---|---|
| 1 | the Step 6 suites pass | CI, three systems |
| 2 | a fresh clone on a second machine shows the same docs on the same nodes | commit `.vnoc/` in a small project, clone it on another machine (or another folder and a clean profile), compare the tree |
| 3 | no file is ever left half-written | start an attach, kill the `vnoc` process (Activity Monitor) while the input box is open and again right after Enter; the code file and `.vnoc/` are whole, and the next start shows either the doc or nothing |
| 4 | a ~1,000-file project shows the tree in under 10 seconds | open one (Django is about that size), read the time from the core's log line `index: walked … ms in all`; write the real number into `02-mvp-and-releases.md` |
| 5 | the programs start on a clean Mac and a clean Windows machine | §2 "Not needed": if the system blocks them, signing moves into 0.1 |

## 11. What may go wrong

| What | Sign | Then |
|---|---|---|
| a folder renamed with `git mv` is not seen | refactor test 6 times out, the docs show under Missing | the watcher's pattern only matches files (`**/{*.py,…}`), and a folder move can arrive as one event for the folder. Fix in `watchFiles`: also listen to every create and delete with no file extension, or send `vnoc/rebuild` for such an event |
| `pylspt` cannot be fetched on a runner | `cargo` stops with an authentication error in the driver step | the repo is private: §2, `PYLSPT_TOKEN` |
| the Linux VSIX does not start on an older system | "GLIBC_2.xx not found" in the output channel | build on an older image, or build the two programs with `musl` |
| the workflow file is renamed or the publish step moves to another file | `vsce publish --oidc` is refused although nothing else changed | the policy trusts `publish.yml` only: move it back, or change the policy on the Marketplace |
| the Open VSX token ran out | `ovsx publish` answers 401 | a new token (§2), the same secret name |
| Windows: the driver or the core is held by antivirus | a slow first start, or "access denied" | report it as a false positive; signing is the lasting answer |
