# 19. CI and releases: test every change, build only for releases

Written 2026-11-15 from the workflow files, the step timings of runs 43706820820 and 43706825744 (GitHub API), and doc 13. **Nothing here was
built or run.** Every `[check]` is a claim to verify before depending on it; every `[measure]` is a number to write here once a run gives it.

```
CI and releases
├── 1. where things stand          43 minutes, and why
├── 2. the shape after this        three workflows, one setup action
├── 3. ci.yml                      every PR: lint and test, once, cancelled when superseded
├── 4. package.yml                 the four VSIX files, each on its own runner
├── 5. publish.yml                 test and package side by side · notes from the CHANGELOG · checksums · provenance
├── 6. dependencies and pinning    Dependabot · actions pinned to a commit
├── 7. GitHub settings (yours)     rulesets for main and for tags · the release environment · merge and Actions settings
├── 8. how to release              the runbook
├── 9. decisions                   what needs your yes or no
├── 10. order of work              one commit each, with exit checks
└── 11. later
```

## 1. Where things stand

| Problem | Cost | Where it comes from |
|---|---|---|
| **Every push builds the release.** `package` runs in every matrix row: `lto = "fat"`, `codegen-units = 1` over ruff, ty, salsa | macOS 20–23 min (arm64, then x64), Windows 9–11, Linux 3–8 | doc 13 §5: "every row runs the full list of steps that the job runs today", carried over from the macOS-only CI that made one VSIX |
| **A PR push runs CI twice**: `push: branches: ["**"]` and `pull_request` both fire | 6 runners instead of 3; both runs start cold and both try to save the cache | step 6.4 (`a2a8c2a`): `["**"]` was meant only to stop a *tag* push running CI twice |
| **The Rust cache misses on macOS and Windows**: the python driver's debug build took 8–14 min with rust-cache on | 8–14 min per row | probably eviction: 2 caches × 3 systems, the root one also holding release builds, against GitHub's 10 GB per repo **[check]** Actions → Caches |
| **The Intel Mac file is built after arm64**, on the same runner | 10–11 min on the slowest row | doc 13 §5 (cross-built: the Intel runners are going away) |
| No `cargo fmt`, no `clippy` in CI | both workspaces fail `cargo fmt --check` today | never added |
| `main` is not protected | anything can be pushed or force-pushed to it, and a red PR can be merged | never set up |
| No `permissions:` in `ci.yml`, no `timeout-minutes` anywhere, actions pinned by tag (`@v4`) | a hung job can run for 6 h; a moved tag changes what runs | never set up |

Timings (macOS is the slowest row, so its time is the total):

| Step | macOS | Windows | Linux |
|---|---|---|---|
| python driver `cargo build && cargo test` | 10–14 min | 8–13 min | < 20 s (hit) |
| `package <target>` | 10–12 min | 9–11 min | 3–8 min |
| `package darwin-x64` | 10–11 min | — | — |
| everything else | ~4 min | ~3 min | ~2 min |
| **job** | **34–41 min** | **20–27 min** | **6–11 min** |

## 2. The shape after this

```
.github/
├── actions/setup/action.yml   one setup for every job: pnpm, Node 22, long paths on Windows, both toolchains, both Rust caches, pnpm install
├── workflows/ci.yml           pull_request · push to main · by hand · called by publish.yml
│     changes ──► lint ─┐
│             └─► test (macOS, Linux, Windows) ─┴─► ci-ok        ← the one required check
│     package (push to main only) ──► package.yml
├── workflows/package.yml      called only: the four VSIX files, four runners side by side
├── workflows/publish.yml      tag v* · by hand (dry run)
│     check ──► test (ci.yml) ────┐
│           └─► package (package.yml, --pre-release) ─┴─► publish (approval) ──► Marketplace · Open VSX · GitHub release
└── dependabot.yml             monthly, grouped
```

| Event | Runs | Expected time |
|---|---|---|
| push to a PR branch | lint + test on 3 systems (a newer push cancels the older run) | **~10–15 min** warm **[measure]** |
| a PR that changes only `docs/**` or `*.md` | `changes` and `ci-ok`, nothing else | **< 1 min** |
| push to a branch with no PR | nothing (Actions → ci → Run workflow, if you want it) | — |
| merge to `main` | lint + test, and the four files as artifacts; the caches are saved here | ~25 min, nobody waits for it |
| tag `v*` | tests and the four files side by side, then your approval, then publish | ~25 min + approval **[measure]** |

## 3. `ci.yml`

### 3.1 Triggers, concurrency, permissions

```yaml
name: ci
on:
  pull_request:
  push:
    branches: [main]     # a branch is tested through its PR; main after a merge
  workflow_dispatch:     # a branch without a PR, by hand
  workflow_call:         # publish.yml: the same lint and tests before a release
permissions:
  contents: read
concurrency:             # a newer push to the same PR cancels the run still going; main and releases are never cancelled
  group: ${{ github.workflow }}-${{ github.event.pull_request.number || github.ref }}
  cancel-in-progress: ${{ github.event_name == 'pull_request' }}
env:
  CARGO_TERM_COLOR: always
```

- The `pull_request` run tests the branch **merged with main** (`refs/pull/N/merge`), which the push run did not.
- In a called workflow `github` is the caller's context **[check]**: from `publish.yml` the group is `publish-refs/tags/v…`, its own.
  `package.yml` must therefore have **no** `concurrency:` of its own: called from the same release it would compute the same group and the two
  would block each other.
- The `workflow_call` input `pre-release` goes away: `ci.yml` no longer packages.

### 3.2 Jobs

| Job | Runs on | Does | Why |
|---|---|---|---|
| `changes` | ubuntu-latest | on a PR, `dorny/paths-filter@v3`: does anything outside `docs/**` and `**/*.md` change? Any other event: yes | a docs-only PR finishes in seconds. A workflow-level `paths-ignore` cannot do this: the required check would never report and the PR could not be merged |
| `lint` | ubuntu-22.04 | `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings` in both workspaces · `pnpm typecheck` | fast feedback; runs beside `test`, not before it (waiting would add its time to every run) |
| `test` | the 3-system matrix | today's steps from `pnpm install` to the window suite, **without** `typecheck` (moved to `lint`) and **without** the package and upload steps | the point of CI |
| `ci-ok` | ubuntu-latest | `needs: [changes, lint, test]`, `if: always()`. Passes only if `changes` succeeded **and** either `lint` and `test` both succeeded, or this is a PR that `changes` confirmed docs-only and both were skipped. Anything else fails, including a skip nobody asked for | **the one required check** (§7.1): a matrix row renamed or added never breaks the ruleset, and a configuration mistake that skips the tests turns it red, not green |
| `package` | — | `if: github.event_name == 'push' && github.ref == 'refs/heads/main'` · `uses: ./.github/workflows/package.yml` | the four files of every merge, to install by hand before tagging; fills the release caches that a tag run can read (§4). Not in `ci-ok`'s needs: nothing waits for it |

```yaml
  changes:
    runs-on: ubuntu-latest
    permissions: { contents: read, pull-requests: read }   # paths-filter asks the API for the PR's files
    outputs:
      code: ${{ steps.filter.outputs.code || 'true' }}      # not a PR (main, a release, by hand): everything runs
    steps:
      - uses: dorny/paths-filter@v3
        id: filter
        if: github.event_name == 'pull_request'
        with:
          predicate-quantifier: every    # a file counts only if it matches all three: not docs, not Markdown
          filters: |
            code:
              - '**'
              - '!docs/**'
              - '!**/*.md'

  ci-ok:
    if: always()
    needs: [changes, lint, test]
    runs-on: ubuntu-latest
    steps:
      - env:
          EVENT: ${{ github.event_name }}
          CHANGES: ${{ needs.changes.result }}
          CODE: ${{ needs.changes.outputs.code }}
          LINT: ${{ needs.lint.result }}
          TEST: ${{ needs.test.result }}    # a matrix: "success" only if every row succeeded
        run: |
          echo "event=$EVENT changes=$CHANGES code=$CODE lint=$LINT test=$TEST"
          [ "$CHANGES" = success ] || { echo "::error::changes did not succeed"; exit 1; }
          if [ "$CODE" = true ]; then
            [ "$LINT" = success ] && [ "$TEST" = success ] || { echo "::error::code changed: lint and test must both succeed"; exit 1; }
          elif [ "$CODE" = false ] && [ "$EVENT" = pull_request ]; then
            [ "$LINT" = skipped ] && [ "$TEST" = skipped ] || { echo "::error::docs-only PR, but lint or test ran and did not skip cleanly"; exit 1; }
          else
            echo "::error::unexpected: code=$CODE on $EVENT"; exit 1
          fi
```

The three results go in through `env:`, not spliced into the script, and the first line prints them. A red `ci-ok` then says why in its log.

`lint` and `test` both have `needs: changes` and `if: needs.changes.outputs.code == 'true'`. `timeout-minutes`: `lint` 25, `test` 45, the
small jobs 5.

**The caller must allow the permissions.** A called workflow can only lower the token's permissions. `publish.yml` gives `contents: read`, so
its `test` job must say `permissions: { contents: read, pull-requests: read }` or `changes` asks for more than it has and the release does not
start (`startup_failure`).

### 3.3 The setup action and the cache

`.github/actions/setup/action.yml`, a composite action with two inputs. `cache-key` is `test`, `lint` or `release-<target>`. `save-cache` is
a string `'true'`/`'false'` that the calling job computes from the rules below, and the action passes it to both rust-cache steps' `save-if`. It holds today's steps
in this order: `pnpm/action-setup@v4` · `actions/setup-node@v4` (22, `cache: pnpm`) · the Windows long-paths step · `rustup toolchain install`
in the root and in `drivers/python` · the two `Swatinem/rust-cache@v2` · the `.vscode-test` cache (`test` only, via an input or a
`hashFiles` guard) · `pnpm install --frozen-lockfile`. Every `run:` in a composite action needs `shell: bash`.

| Cache rule | Why |
|---|---|
| both rust-cache steps: `save-if: ${{ github.ref == 'refs/heads/main' \|\| github.event_name == 'pull_request' }}` | **PRs restore and save; tags never save.** A PR first restores its own cache, then main's (GitHub's lookup order). rust-cache saves only when it did **not** restore its exact key. So a PR with main's dependencies writes nothing. A PR that bumps the `language_driver_rs` rev, `Cargo.lock` or a toolchain writes once, on its first push, and its later pushes hit. If PRs never saved, every push to such a PR would rebuild ruff again (8–14 min). A PR's cache is visible only to that PR. When the total is over the limit GitHub evicts the least recently used first **[check]**: main's caches are read by every PR and stay, a merged PR's cache is read by nothing and goes (or after 7 days unused). A cache saved on a tag can't be read by any other ref, so releases don't save. None of this **guarantees hits**: the first run after a new toolchain, lockfile or key is cold |
| `package.yml`'s rust-cache: `save-if: ${{ github.ref == 'refs/heads/main' }}` | PRs don't package. Only main's runs make release caches, and a tag run reads them |
| `key:` from `cache-key` (`test`, `driver-test`, `release-darwin-arm64`, …) | lint (check mode), test (debug) and release builds have different files; one cache each |
| keep `cache-on-failure: true` | a red main still leaves a cache |
| `drivers/python/rust-toolchain.toml`: add `components = ["rustfmt", "clippy"]` | `lint` runs both there; the root file has them, the driver's (1.99.1) does not, and the runner's rustup profile may be minimal **[check]** |
| optional: `CARGO_PROFILE_DEV_DEBUG: line-tables-only` in `env:` | smaller debug builds and caches, and backtraces still have line numbers. Try it once, compare the cache sizes **[measure]**, keep it only if they shrink a lot |

This plan adds caches as well as fixing them: per system there are now `lint` (Linux only), `test` and `release-<target>`, each in two
workspaces. Whether misses stop is **[measure]**, not a given. After the first merge to `main`, record in §1:

| Measure | Where | Good |
|---|---|---|
| the size of each cache and the total | Actions → Caches | total under 10 GB, with room for one `Cargo.lock` change (old and new side by side until the old one ages out) |
| hit or miss on a PR run's two rust-cache steps, per system | the step's log ("Cache restored" / "No cache found") | a hit on all three systems when only `src/` changed |
| the python driver step's time on a PR, per system | the step timing | ~1–3 min **[measure]**, against 8–14 today |

If the total goes over, drop `rust-cache` from `package.yml` (§4) first. Releases are rare, and a cold release build costs ~25 min. If PRs
still miss with the total well under the limit, the key is the problem, not the size: compare the key printed in the PR's log with the one
saved on `main`.

## 4. `package.yml`

Called only (by `ci.yml` on `main`, by `publish.yml` on a tag). No `concurrency:` (§3.1).

```yaml
name: package
on:
  workflow_call:
    inputs:
      pre-release: { type: boolean, default: false }
permissions:
  contents: read
jobs:
  package:
    name: package ${{ matrix.target }}
    runs-on: ${{ matrix.os }}
    timeout-minutes: 45
    strategy:
      fail-fast: false
      matrix:
        include:
          - { target: darwin-arm64, os: macos-latest }
          - { target: darwin-x64,   os: macos-latest, cross: x86_64-apple-darwin }   # cross-built: packaged, not tested (doc 13 §5)
          - { target: linux-x64,    os: ubuntu-22.04 }                              # the older glibc (doc 13 §5)
          - { target: win32-x64,    os: windows-latest }
    defaults: { run: { shell: bash } }
    steps:
      - uses: actions/checkout@v4
      - uses: ./.github/actions/setup
        with: { cache-key: "release-${{ matrix.target }}" }
      - if: matrix.cross
        run: |
          rustup target add ${{ matrix.cross }}
          (cd drivers/python && rustup target add ${{ matrix.cross }})
      - run: node scripts/package.mjs --target ${{ matrix.target }} ${{ inputs.pre-release && '--pre-release' || '' }}
      - uses: actions/upload-artifact@v4
        with: { name: "vsix-${{ matrix.target }}", path: "vnoc-${{ matrix.target }}.vsix", if-no-files-found: error, retention-days: 30 }
```

The artifact names stay `vsix-<target>`: `publish.yml` already downloads `vsix-*`. *(Changed in A13 of doc 21, 2026-11-20.)* `scripts/package.mjs`
now also writes the **command-line archive** right after it copies the two programs into `extension/bin/` (nothing is compiled twice): `vnoc-cli-<target>.tar.gz`
(`vnoc-cli-win32-x64.zip`), flat, four files (`vnoc`, `vnoc-driver-python`, `LICENSE`, `THIRD_PARTY_NOTICES.md`). `package.yml` uploads it as a second
artifact, `cli-<target>`, after the `vsix-<target>` one:

```yaml
      - uses: actions/upload-artifact@v4
        with: { name: "cli-${{ matrix.target }}", path: "vnoc-cli-${{ matrix.target }}.*", if-no-files-found: error, retention-days: 30 }
```

On Windows the zip is written by Windows' own `bsdtar` (`%SystemRoot%\System32\tar.exe -a`, by its full path: the `tar` first on the PATH of a `bash` on a
runner is GNU tar from Git, which cannot write a zip). **[check]** until the first run on `windows-latest`.

How much the release cache saves under fat LTO is **[measure]**: it skips compiling the dependencies, but the LTO step over the whole driver
runs every time.

## 5. `publish.yml`

The file name and the place of the `vsce publish` step stay as they are (the Trusted Publishing policy, doc 13 §2).

| Change | Detail |
|---|---|
| `concurrency: { group: release, cancel-in-progress: false }` | never two releases at once; a second one waits |
| `check`: the CHANGELOG has the version | `grep -qx "## $version" extension/CHANGELOG.md` or `::error::extension/CHANGELOG.md has no "## $version" section`. Fails before anything is built |
| `build` becomes two jobs, both `needs: check` | `test: uses: ./.github/workflows/ci.yml` with `permissions: { contents: read, pull-requests: read }` (§3.2) · `package: uses: ./.github/workflows/package.yml` with `pre-release: true`. Side by side: the release takes as long as the slower of the two, not the sum |
| `publish: needs: [test, package]` | unchanged otherwise: `environment: release`, the four-files check, `vsce publish --oidc` **first** (the 15-minute login), then Open VSX |
| `publish` also downloads `cli-*` *(A13)* | beside `vsix-*`, into `cli/`; the "files are there" step counts four VSIX **and four archives** (`vnoc-cli-*`) |
| `install.sh`, stamped *(A13)* | after the two stores: `scripts/install.sh` with its line `VERSION="@VERSION@"` replaced by the extension's version (`sed`), checked for the replacement and for `sh -n`, into `cli/`. It is stamped so that `curl …/v0.5.0/install.sh \| sh` installs 0.5.0 (07 §6 of doc 21) |
| after the two stores: checksums | all files of the release, in one folder `release/` (four VSIX, four archives, `install.sh`), **one** `SHA256SUMS` over them: `(cd release && sha256sum -- * > SHA256SUMS)`, and a check that it has nine lines |
| after the two stores: provenance | `actions/attest-build-provenance@v2` with `subject-path` the VSIX, the archives and `install.sh` of `release/`; the job gets `attestations: write` (it already has `id-token: write`). Anyone can then check a file with `gh attestation verify <file> -R yaredtsy/vnoc` |
| the release notes come from the CHANGELOG | `awk -v h="## $version" '$0==h{f=1;next} /^## /{if(f)exit} f' extension/CHANGELOG.md > notes.md`. A tag message is then just the version number |
| the GitHub release is safe to run again | see below |
| `timeout-minutes` | `check` 5, `publish` 15 |

**Every step of `publish` must be safe to run a second time**, because the fix for a partial failure is "Re-run failed jobs":

| Step | On a second run | How |
|---|---|---|
| Marketplace | the version is already there: skipped | `vsce publish --skip-duplicate` (already in the file) |
| Open VSX | the same | `ovsx publish --skip-duplicate` **[check]** that ovsx has the flag (doc 13 §7.2 left it open). If not, treat its "already exists" error as success and any other error as failure |
| checksums, provenance | made again from the same files | a second attestation for the same file is harmless |
| GitHub release | **`gh release create` fails if the release exists** | create it if missing, then upload every file with `--clobber`, so a release left half-made gets its missing files |

```bash
if gh release view "$GITHUB_REF_NAME" >/dev/null 2>&1; then
  echo "release $GITHUB_REF_NAME exists: uploading the files again"
else
  gh release create "$GITHUB_REF_NAME" --prerelease --title "v$version" --notes-file notes.md
fi
gh release upload "$GITHUB_REF_NAME" release/* --clobber   # four VSIX, four archives, install.sh, SHA256SUMS
```

Release and files are two commands on purpose: a `create` that fails halfway through its uploads leaves a release with some of the files, and
the second run's `upload --clobber` completes it. The notes are written only when the release is made. A second run doesn't overwrite notes you
edited by hand on the release page.

**What the tests prove, and what they don't.** `test` runs the window suite against `target/debug` and the freshly built extension. It
never installs a `.vsix`. The files from `package` are built from the same commit but are not run by any job. The `darwin-x64` file is
cross-built and runs nowhere. Before you approve a release, install the release's own file on your machine (§8 step 4). That is the only check
that a packaged file starts, and it covers one system, not four.

The dry run stays: Actions → publish → Run workflow, on any branch, `publish: false`. It runs the tests and builds the four files and publishes
nothing. It does **not** reach the `publish` job, so it proves nothing about the `release` environment (§7.3).

## 6. Dependencies and pinning

`.github/dependabot.yml`, monthly and grouped so it opens a few PRs a month, not dozens:

| Ecosystem | Directory | Group |
|---|---|---|
| `github-actions` | `/` | all in one PR |
| `npm` (reads `pnpm-lock.yaml`) | `/` | minor + patch in one PR; majors one each |
| `cargo` | `/` | minor + patch in one PR |
| `cargo` | `/drivers/python` | minor + patch in one PR. The `language_driver_rs` git rev is yours to move, not Dependabot's **[check]** whether it opens PRs for git deps; if so, `ignore` those three crates |

**Pin every third-party action to a commit SHA** with the version in a comment (`uses: actions/checkout@<40-char sha> # v4.2.2`). A tag can be
moved to other code; a commit can't. Dependabot updates the SHA and the comment together. Look each SHA up on the action's releases page,
**don't type one from memory**. `./.github/actions/setup` is local and needs no pin.

## 7. GitHub settings (yours: the repo can't set these)

Do 7.1 **after** commit C3 has run on a PR: a check can be required only once it has reported at least once.

### 7.1 Ruleset `main` (Settings → Rules → Rulesets → New branch ruleset)

| Setting | Value | Why |
|---|---|---|
| Enforcement | Active | |
| Target | Default branch | |
| Restrict deletions · Block force pushes | on | `main`'s history is what tags and releases point at |
| Require a pull request before merging | on, **0** required approvals | you can't approve your own PR; the PR is there for CI, not for review |
| Require status checks to pass | `ci-ok`, source **GitHub Actions** | the source pin stops another app from posting a fake `ci-ok` |
| Require branches to be up to date | off | the PR run already tests the merge with `main`; only your own PRs move `main` |
| Allowed merge methods | Merge (and Rebase if you like), **not Squash** (D3) | one commit per step is how you review; squash would turn a branch into one commit |
| Bypass list | Repository admin, **"For pull requests only"** | in an emergency you can merge a PR with red CI; you still can't push to `main` directly |

### 7.2 Ruleset `release tags` (New tag ruleset)

| Setting | Value |
|---|---|
| Target | tags matching `v*` |
| Restrict creations · Restrict updates · Restrict deletions | on |
| Bypass list | Repository admin, Always (you make the tags) |

A published tag can then not be moved or deleted by accident; a fix is the next patch number (doc 13 §7.2).

### 7.3 Environment `release` (Settings → Environments → release)

| Setting | Value |
|---|---|
| Required reviewers | you (already set, per `publish.yml`'s comment **[check]**) |
| Deployment branches and tags | **Selected**: tag pattern `v*` only. A publish can then only run from a tag, whatever the workflow says |
| Secrets | `OVSX_PAT` here, not as a repository secret, once Open VSX approves the namespace: only the release job sees it |

### 7.4 Repository settings

| Where | Setting |
|---|---|
| General → Pull Requests | Allow merge commits on · Squash off (D3) · Always suggest updating PR branches on |
| Actions → General → Workflow permissions | **Read repository contents** (the default token; jobs ask for more themselves) · "Allow GitHub Actions to create and approve pull requests" off |
| Actions → General → Fork pull request workflows | Require approval for first-time contributors (the default for a public repo) |
| Security | Dependabot alerts on · Secret scanning and push protection on · Private vulnerability reporting on (all free for a public repo) |

## 8. How to release

```
1  on a branch: the version "X.Y.Z" in four places: extension/package.json · the Rust workspace (Cargo.toml's [workspace.package], and the `version` line of
   each crates/*/Cargo.toml, which do not inherit it: crates/vnoc's is the one the guide test reads) · drivers/python/Cargo.toml · skills/vnoc/SKILL.md
   (metadata.vnoc-version; the guide test fails when it differs from crates/vnoc's). Then `cargo build` in both workspaces refreshes the two Cargo.lock.
   extension/CHANGELOG.md "## Unreleased (…)" becomes "## X.Y.Z"
2  PR → ci-ok green → merge. Optional: main's run leaves vsix-* and cli-* artifacts. Installing one catches a packaging problem before you tag,
   but these are NOT the files that get published: the tag builds its own
3  git checkout main && git pull && git tag -a vX.Y.Z -m "X.Y.Z" && git push origin vX.Y.Z
4  Actions → publish: check, then tests and the files side by side. The run then waits for your approval. Before approving:
   download your system's vsix-* artifact FROM THIS RUN · code --install-extension vnoc-<target>.vsix · open a Python folder:
   the tree loads, a doc opens, a call tree expands. Then the same run's cli-* artifact for your system: unpack it, run `./vnoc --version`
   and `./vnoc status` in a Python folder (the driver is beside it). Only then "Review deployments" → approve
5  check: the Marketplace page shows X.Y.Z for the four systems · the GitHub release has 4 VSIX + 4 archives (vnoc-cli-*) + install.sh + SHA256SUMS (10 files)
   and the CHANGELOG section · `curl -fsSL …/vX.Y.Z/install.sh | VNOC_INSTALL_DIR=$(mktemp -d) sh` installs it and prints vX.Y.Z
```

Step 4 runs one packaged file and one archive on one system. The other three are built and packaged but have not run anywhere. Say so in the release notes
until there is a machine (or a job) for each.

If anything in `publish` fails after a store has accepted the files: Re-run failed jobs. Every step skips or repeats safely what is already
done (§5). Don't make a new tag for the same version.

## 9. Decisions

| # | Question | My lean |
|---|---|---|
| **D1** | Test branches only through PRs (push to a branch without a PR runs nothing) | **yes**. The PR run tests the merge with `main`; Run workflow covers a branch without a PR |
| **D2** | All three systems on every PR, not Linux only | **yes**. Windows and Linux found real bugs in step 6.2; with a warm cache they are not the slow part |
| **D3** | Merge commits, no squash | **yes**: keeps one commit per step |
| **D4** | Provenance attestations on the release files | **yes**: one step, free for a public repo |
| **D5** | Save release caches from `main` | **try it**, keep it only if §3.3's 10 GB check passes |
| **D6** | `clippy -D warnings` | **yes**, after C2 fixes what's there. If there are more than ~30 warnings, run clippy as a warning first and turn on `-D warnings` in a later commit |

## 10. Order of work

Each row is one commit. Workflow changes can only be proven by a run: push the branch, open a PR, read the run.

| # | Commit | Files | Exit check |
|---|---|---|---|
| **C1** | `style: cargo fmt in both workspaces` | only what `cargo fmt` changes | `cargo fmt --check` passes in the root and in `drivers/python`; no other change in the diff |
| **C2** | `chore: clippy clean in both workspaces` | the warnings' files · `drivers/python/rust-toolchain.toml` (components) | `cargo clippy --all-targets -- -D warnings` passes in both (or D6's fallback) · all tests still pass |
| **C3** | `ci: test every PR once, cancel superseded runs, skip docs-only, one required check, cache saved from main` | `.github/actions/setup/action.yml` (new) · `.github/workflows/ci.yml` | on the PR: **one** run per push, not two · a second push cancels the first · `lint`, 3 × `test`, `ci-ok` green · no `package` job. **Docs-only is a separate PR**: the filter compares the whole PR with `main`, so a docs commit on this code PR still runs everything. Open a throwaway PR from `main` that changes only a `.md` file: `lint`/`test` skipped, `ci-ok` green, then close it. **`ci-ok` must also fail when it should**: on a throwaway commit, set the `changes` output to `'false'` on every event, push, and see `ci-ok` red with "unexpected"; then drop the commit |
| **C4** | `ci: package.yml, the four files on four runners, on main and in releases` | `.github/workflows/package.yml` (new) · `ci.yml` (the `package` job) | the dry run (Actions → publish → Run workflow on the branch, `publish: false`, after C5) shows four `package` jobs side by side and four `vsix-*` artifacts |
| **C5** | `publish: tests and packaging side by side, CHANGELOG check and notes, checksums, provenance, safe to re-run` | `.github/workflows/publish.yml` | the dry run is green · `check` fails on a branch whose CHANGELOG has no `## <version>` (try it once, then drop the commit). The create-or-upload logic only runs on a real tag. Read it in review, and the first release's log must show which branch it took |
| **C6** | `ci: Dependabot, actions pinned to commit SHAs` | `.github/dependabot.yml` (new) · the three workflows · the setup action | every `uses:` of a third-party action is a 40-character SHA with a version comment · a run is green |
| **S** | your settings, §7.1–7.4 | — | a PR with a red `ci-ok` can't be merged · `git push origin main` is refused · the environment page lists `v*` as its only deployment rule. A dry run can't test that rule (§5). To see it refuse, push a throwaway branch with a throwaway workflow: one job, `environment: release`, `run: "true"`. The job must fail with "not allowed to deploy to release". Then delete the branch |
| **D** | `docs: 19 as built; doc 13 §5 points here` | `docs/19-ci-cd.md` (the `[measure]` numbers, what changed) · `docs/13-harden-and-publish.md` (§5: "CI tests; packaging moved to package.yml, see 19") · `docs/08-decisions-to-log.md` (D1–D6 rows) | — |

For the coding session: **stage the files by path; never `git add .`, `git add docs` or `git add .github`.** Don't touch `docs/futureplan/`.
C3 to C6 can go on one branch and one PR; the PR's own runs are the test.

## 11. Later

| What | When |
|---|---|
| A weekly scheduled job on `main`: `cargo audit` / `cargo deny` (advisories and licences) in both workspaces, `pnpm audit` | when the first outside user reports something, or before 1.0 |
| CodeQL for the TypeScript (extension, webview) | the same |
| Signing the binaries (macOS notarisation, Windows Authenticode) | when Gatekeeper or Defender block the VSIX's programs for users (doc 08 Q6) |
| Stable releases next to pre-releases | the Marketplace wants different numbers for the two; its advice is an even minor for stable and an odd one for pre-release **[check]**. Decide before the first stable version |
