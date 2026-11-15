<div align="center">
  <img src="extension/media/icon.png" alt="" width="72" height="72" />
  <h1>V-NOC</h1>
  <p><b>Browse Python code as a graph in VS Code. See what a function calls. Attach docs that survive renames.</b></p>

  <!-- badges: Marketplace version · Open VSX version · CI · Licence · Discord -->
  [![Marketplace](https://img.shields.io/visual-studio-marketplace/v/vnoc.vnoc?label=Marketplace&include_prereleases)](https://marketplace.visualstudio.com/items?itemName=vnoc.vnoc)
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
