# 18.02 The pages: what someone reads on GitHub and on the Marketplace

```
pages
├── 1. the root README (GitHub's front page): sections and the draft text
├── 2. extension/README.md (the Marketplace page): what changes
├── 3. CONTRIBUTING.md
├── 4. SECURITY.md
├── 5. the issue forms (.github/ISSUE_TEMPLATE/)
├── 6. the CHANGELOG for 0.2.0
├── 7. GitHub settings (by hand)
├── 8. the checks
└── 9. telling people
```

Step R5 of the README. The pictures are 01's; the help commands are 03's. **The two READMEs have different readers**: GitHub's is read by
developers deciding whether to try it *or contribute*; the Marketplace's by someone deciding whether to install. They share the GIF and the
first paragraph, and nothing else needs to match.

The logo is the one the extension already ships: `extension/media/icon.png` (256 × 256, the green share-node mark of the old project's
`assets/icon.svg`, `#22c55e`). The GitHub README may use the PNG; **the Marketplace README may not use an SVG** (vsce refuses it, badges from
known services aside).

## 1. The root README

Sections, in order. The text below is the draft; write it as it is unless the code says otherwise.

````markdown
<div align="center">
  <img src="extension/media/icon.png" alt="" width="72" height="72" />
  <h1>V-NOC</h1>
  <p><b>Browse Python code as a graph in VS Code. See what a function calls. Attach docs that survive renames.</b></p>

  <!-- badges: Marketplace version · Open VSX version · CI · Licence · Discord -->
  [![Marketplace](https://vsmarketplacebadges.dev/version/vnoc.vnoc.svg)](https://marketplace.visualstudio.com/items?itemName=vnoc.vnoc)
  [![Open VSX](https://img.shields.io/open-vsx/v/vnoc/vnoc?label=Open%20VSX)](https://open-vsx.org/extension/vnoc/vnoc)
  [![CI](https://github.com/yaredtsy/vnoc/actions/workflows/ci.yml/badge.svg)](https://github.com/yaredtsy/vnoc/actions/workflows/ci.yml)
  [![Licence: Apache-2.0](https://img.shields.io/badge/licence-Apache--2.0-blue.svg)](LICENSE)
  [![Discord](https://img.shields.io/badge/Discord-join-5865F2.svg?logo=discord&logoColor=white)](https://discord.gg/J5nfPHqyBr)
</div>

![Open a function, see what it calls, follow a call to its own file](extension/media/screenshots/calls.gif)

*Software development is a computational problem that we have turned into a memory problem.* V-NOC lets the computer keep the map:
what is in the project, what calls what, and what someone wrote about it.

## What it does

- **The structure, as a tree and a canvas.** Folders, files, classes and functions in the side bar; open any part of it as cards on a canvas.
  Focus on one node and both show only it.
- **Calls, one level at a time.** Open a function to see what it calls, as cards joined by dashed lines. Explore a call to jump to where it
  lives, and climb from there.
- **Docs that stay with the code.** Attach Markdown to a function, class, file or folder. Rename it, move it, `git mv` its file: the doc is
  still there. The docs are files in `.vnoc/`: commit them and your team sees them.

![The side bar, the canvas and a doc](extension/media/screenshots/overview.png)

## Install

V-NOC is a **pre-release**: in VS Code's Extensions view, search **V-NOC** and choose **Install Pre-Release Version**.
Also on [Open VSX](https://open-vsx.org/extension/vnoc/vnoc) (Cursor, Windsurf, VSCodium) and as VSIX files on the
[releases page](https://github.com/yaredtsy/vnoc/releases).

Python only, for now. macOS (Apple silicon and Intel), Linux x64, Windows x64.

## Start

1. Open a Python project. 2. Click the V-NOC icon in the Activity Bar. 3. Click a function to see it; **Show Calls** to see what it calls.
4. Right-click anything → **Add Doc…**. 5. `⊞` opens the canvas.

## Docs that survive refactors

![Add a doc, rename the function: the doc stays](extension/media/screenshots/docs.gif)

V-NOC writes a short ID into the code (one line in the docstring, a comment at the top of a file, a hidden `.vnoc-id` file in a folder) and
finds the code again by it. A doc whose code is gone shows under **Needs Attention**, where you relink it or remove it.

## How it is built

```
VS Code extension (TypeScript)  ──JSON-RPC, stdio──▶  core (Rust)  ──JSON-RPC, stdio──▶  language driver (Rust; Python today)
  side bar + canvas (React webviews)                    the index, IDs, .vnoc/ files        outline, IDs to write, call trees
```

A language is one driver program: [docs/06-drivers.md](docs/06-drivers.md). The design notes are in [`docs/`](docs/README.md).

## Roadmap

0.1 the docs loop · **0.2 calls** (this release) · 0.3 edit code and docs in the canvas · later: more languages, changes shown as a graph,
access for coding agents ([docs/futureplan](docs/futureplan/README.md)).

## Contributing

Bug reports: **V-NOC: Report Issue…** in VS Code, or [an issue](https://github.com/yaredtsy/vnoc/issues/new/choose). Questions and ideas:
[Discord](https://discord.gg/J5nfPHqyBr). Code: [CONTRIBUTING.md](CONTRIBUTING.md). If V-NOC is useful to you, a ⭐ helps other people find it.

## Licence

Apache-2.0. The pictures show [httpx](https://github.com/encode/httpx) (BSD-3-Clause). Third-party code in the binaries:
[THIRD_PARTY_NOTICES.md](extension/THIRD_PARTY_NOTICES.md).
````

Relative image paths are fine here: GitHub resolves them on the branch shown.

## 2. `extension/README.md` (the Marketplace page)

Keep its structure (it is right). Changes:

| Where | Change |
|---|---|
| top, after the first paragraph | `![…](https://raw.githubusercontent.com/yaredtsy/vnoc/v0.2.0/extension/media/screenshots/calls.gif)`. **Absolute URLs pinned to the tag**: images packed in the VSIX are not shown, and a link to `main` changes under a published version |
| `SCREENSHOT 1` | `overview.png` |
| `SCREENSHOT 2` | `calls.png` |
| `SCREENSHOT 3` | `call-menu.png` and `editor-menu.png`, side by side is not possible in Marketplace Markdown: one after the other |
| a badge line under the title | Marketplace, Discord, Licence (shields.io is an allowed badge source **[check]**) |
| new section *Feedback*, before *Licence* | **V-NOC: Report Issue…** fills in your versions and opens a GitHub form. Questions: [Discord](https://discord.gg/J5nfPHqyBr). The source: [GitHub](https://github.com/yaredtsy/vnoc). |
| *Licence* | add the third-party line of §1 |

The URL is the tag's, so **the pictures must be committed before the tag** (README §4: R4 before R5, R7 last).

## 3. `CONTRIBUTING.md`

Short; it points into `docs/` for depth. Sections:

1. **The layout**: the five folders (`crates/`, `drivers/python/`, `extension/`, `webview/`, `shared/`) in one line each (the current root README's
   list), and `tools/media/` (01).
2. **What you need**: rustup (the two `rust-toolchain.toml` files: `1.94.1` at the root, `1.99.1` in `drivers/python`; `rustup toolchain install`
   in each), Node 22, pnpm (`corepack enable`).
3. **Build**: `pnpm install` · `cargo build` · `(cd drivers/python && cargo build)` · `pnpm -r build`. Then **F5** with *"V-NOC: run the
   extension, open the Python fixtures"* (`.vscode/launch.json`).
4. **Test**: the list `ci.yml` runs, in its order, one line each (typecheck; shared; webview; extension unit; core; driver; core integration;
   the window suite).
5. **Traps**: `ELECTRON_RUN_AS_NODE` (run the window suite as `env -u ELECTRON_RUN_AS_NODE pnpm -r test`); Windows long paths for the driver's
   vendored ruff (`git config --system core.longpaths true`, `CARGO_NET_GIT_FETCH_WITH_CLI=true`); a release build of the driver needs about
   5 GB of disk.
6. **The files users commit are frozen**: `.vnoc/`, the ID line, the markers. A change there needs a doc and a fixture test first
   (`docs/02-mvp-and-releases.md` §1, §4).
7. **Commits and docs**: one step per commit, `step x.y: what it leaves working`; a step updates its doc's "as built" note. The conventions at
   the end of `docs/README.md`.
8. **Adding a language**: `docs/06-drivers.md`.

## 4. `SECURITY.md`

```markdown
# Security

Report a vulnerability privately: GitHub → Security → Report a vulnerability (private vulnerability reporting is on).
Please do not open a public issue. You get an answer within a week.

What V-NOC does on your machine: it starts two programs from the extension (the core and a language driver), which read the files of the
folder you open and write ID lines into the code you attach docs to. In Restricted Mode, the paths to those programs from workspace settings
are ignored (`vnoc.server.path`, `vnoc.driver.path`).
```

## 5. The issue forms

`.github/ISSUE_TEMPLATE/`:

**`bug_report.yml`**: name *Bug*, labels `bug`. The field **ids are the query keys of 03 §3.2** (03 §8 tests this):

| id | type | label | required |
|---|---|---|---|
| `what` | textarea | What happened, and what did you expect? | yes |
| `steps` | textarea | Steps to make it happen (a small public repo helps most) | no |
| `vnoc` | input | V-NOC version (Report Issue fills this in) | yes |
| `vscode` | input | Editor and version | yes |
| `os` | input | System | yes |
| `log` | textarea, `render: text` | Log: V-NOC: Show Log, or paste what Report Issue copied | no |

**`feature_request.yml`**: *Idea*, labels `idea`: `job` (what are you trying to do?), `today` (what do you do today?), `idea` (what would help?).

**`config.yml`**: `blank_issues_enabled: false`; `contact_links`: *Questions and chat → Discord*.

Also `.github/pull_request_template.md`: three lines (what it does · how you tested it · which doc it updates).

## 6. The CHANGELOG for 0.2.0

Replace `## Unreleased (0.2)` with this; the existing list moves under its own heading.

```markdown
## 0.2.0

Calls, and a new side bar.

- **Show Calls**: open a function on the canvas to see what it calls, one level at a time, as call cards joined by dashed lines. A chevron
  shows only when there is something to open.
- **Explore** a call (double-click its card): it opens in a panel of its own in the side bar, focused where the function lives, and the canvas
  follows. Each context has a chip on the canvas and a panel in the side bar; **Close** and **Close Others**.
- **Go to Call Site…**: every place a function is called, with a preview.
- **The side bar is new**: a search box, the focus bar, context panels you can resize. Keyboard and screen reader support as before.
- **Cards** show their code (Show Code in Card) and their docs count, and follow your theme (checked in Dark, Light and both High Contrast themes).
- Closing what you opened stops its request; the Python driver keeps the project open, so the next Show Calls is fast.
- **Report Issue…** and **Help and Feedback…**.

### Changed and removed commands

**A key binding of your own on a removed command stops working.**

(the current list, unchanged)
```

**The heading must be exactly `## 0.2.0`**: `publish.yml` refuses a tag whose version has no such heading, and the GitHub release notes are
this section, cut out with `awk` up to the next `## ` heading (`### Changed and removed commands` stays inside it).

## 7. GitHub settings (by hand, after the merge)

| Setting | Value |
|---|---|
| About → description | *Browse Python code as a graph in VS Code: see what a function calls, attach docs that survive renames.* |
| About → website | the Marketplace URL |
| About → topics | `vscode-extension`, `python`, `code-visualization`, `call-graph`, `documentation`, `code-navigation`, `rust`, `developer-tools` |
| Social preview | `tools/media/out/social.png` (1280 × 640, 01 §6) |
| Features | Issues on; Discussions optional (if on, point `qna` at them, 03 §6) |
| Security | private vulnerability reporting **on** (SECURITY.md promises it) |
| Branches | `main`: require a PR and the three `ci` jobs |
| Labels | `bug`, `idea`, `good first issue`, `driver`, `canvas`, `sidebar` |

## 8. The checks

- [ ] Every link in both READMEs opens (relative ones on GitHub; absolute ones after the tag is pushed).
- [ ] `vsce ls` (from `scripts/package.mjs`): no `media/screenshots/` in the VSIX; `THIRD_PARTY_NOTICES.md` is in it.
- [ ] vsce gives no warning about SVG images or relative image links in `extension/README.md`.
- [ ] *New issue* on GitHub shows *Bug* and *Idea* and the Discord link, and no blank issue.
- [ ] The root README reads well on a phone (GitHub's mobile view): the GIF scales, the badge row wraps.

## 9. Telling people

After the release is live and installs: a post in the Discord with `calls.mp4` (01 §7: Discord plays MP4 inline; a GIF of 6 MB is slow there);
r/vscode and r/Python with the GIF and two lines. The old project's `plan/show-hn-post.md` is a good start for a Show HN, **but keep that for a
release you would be glad to have judged on its own**: a pre-release with one language may be too early. That choice is yours.
