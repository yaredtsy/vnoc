# 02. The MVP and the road to stable

```
releases
├── 1. the rule that makes fast releases safe
│   ├── frozen   what users commit
│   └── free     everything else
├── 2. release 0.1: the docs loop
│   ├── what is in
│   ├── what is out (and when it comes back)
│   └── the work, in seven steps
├── 3. 0.2 → 1.0: one increment per release
└── 4. how each release stays compatible
```

## 1. The rule that makes fast releases safe

Split everything V-NOC produces into two groups:

| | **Frozen from 0.1** | **Free until 1.0** |
|---|---|---|
| What | Things written into the user's repository | Things that live in memory or inside the VSIX |
| Examples | `.vnoc/` layout · `node.toml` · doc frontmatter · the 26-character ID text · the `ID:` line · `# FileID:` · `.vnoc-id` | core ↔ extension messages · core ↔ driver messages · the index · webview state · crate layout |
| Why | A teammate with an older V-NOC will read them. They are in git history forever. | The core, the driver, the extension and the webview ship **together in one VSIX**. They are always the same version. |
| How to change | Only by **adding**. Readers keep what they do not understand. | Any time. |

This is why a small 0.1 is safe: it only has to get the left column right, and the left column is small (five formats, all in
[`04-storage/01-vnoc-folder.md`](04-storage/01-vnoc-folder.md) and [`05-identity/`](05-identity/README.md)).

## 2. Release 0.1: the docs loop

**The one sentence it must prove:** *I opened my project, wrote a note on a function, renamed and moved things for a week, and my note is still on that function.*

### What is in

| Area | 0.1 has | Status today |
|---|---|---|
| Browse | sidebar tree · canvas · node card · read-only code in the card · focus + breadcrumbs · go to code | ✅ mostly built |
| Index | built once at start, updated when files change, **in memory** | ❌ (rebuilt on every request today) |
| Attach | "Attach doc" on a **function, class, file or folder** → a Markdown file in `.vnoc/` | ❌ |
| Marker | function/class: `ID:` in the docstring · file: `# FileID:` · folder: `.vnoc-id`. One key in `vnoc.toml` turns markers off. | 🟡 domain types done |
| Read / edit docs | a **doc panel** on the right of the canvas shows the doc rendered, read-only; **Edit** opens it in a normal VS Code editor tab | ❌ |
| Find again | by marker, else by the stored path + name | ❌ |
| Missing | a quiet "Missing" list · **Relink** by hand · **Remove** (to the V-NOC trash) | ❌ |
| Platforms | macOS arm64 first. Then macOS x64, Linux x64, Windows x64 as 0.1.x. | ❌ |

### What is out, and when it comes back

Each cut below removes work **without changing any frozen format**, so it can come back later as a pure addition.

| Cut from 0.1 | Why it is safe to cut | Returns in |
|---|---|---|
| SQLite cache | Holds nothing a user wrote. Without it, start-up re-reads the code (seconds on ~1,000 files **[measure]**). | 0.2, if start-up or call trees need it |
| Call trees | Independent of docs. | **0.2** |
| Editing code in the canvas | Blocked on spike S5. "Go to code" opens the real editor. | **0.3** |
| A Markdown **editor** inside the webview | The 0.1 panel only reads. A VS Code tab already edits Markdown, with undo and save. | 0.3, with the canvas editor |
| Guided setup / Quick look as a separate mode | With an in-memory index, *everything* is Quick look until the first attach. `.vnoc/` is created then, with defaults. | later, when scope needs a UI |
| Missing vs "not on this branch", `gix`, the `.git` watcher | That rule exists to avoid a **pop-up** on branch switch. 0.1 has no pop-up, only a list. A node that is absent on this branch shows in the list and leaves it when you switch back. | 0.4, with notifications |
| Archive (UI) | The fields stay in `node.toml`. Missing nodes simply stay Missing until relinked or removed. | 0.4 |
| Matching ladder steps 2–6, step 0 | Markers follow renames. Unmarked nodes that move go Missing → Relink. | 0.4 |
| File-rename listeners | Markers already follow the file. | 0.4, for unmarked nodes |
| Private docs, solo mode, `local.toml` | Additive: a second folder with the same layout. | 0.5 |
| Copy-on-first-run, driver downloads | The driver runs straight from the VSIX's `bin/`. | after 1.0 |

### The work, in seven steps

Each step ends with something you can see working. Do them in order: every step uses the one before it.

**Step 0. Package it (do this first)**
The goal is a `.vsix` file that you install with *Extensions ▸ … ▸ Install from VSIX* in a clean VS Code and it works. Nothing new is built.
- [ ] fix R1–R4 from [`01-review.md`](01-review.md) §3
- [ ] root becomes a pure Cargo workspace; delete `src/main.rs` ([`03-repo-structure.md`](03-repo-structure.md))
- [ ] one command builds everything and writes the VSIX
- [ ] CI: build + test on every push (macOS only is enough for now)

> **🧩 VS Code note: platform-specific VSIX.** A VSIX is a zip of the extension folder. Because yours contains native programs (the core and the driver),
> you build one VSIX **per platform**: `vsce package --target darwin-arm64`, `linux-x64`, `win32-x64`… The Marketplace gives each user the right one.
> `vsce publish --pre-release` puts a version on the opt-in pre-release channel, which is where 0.x belongs.

**Step 1. A real index**
- [ ] find the git root; all paths relative to it (C1)
- [ ] build the index once at start; `vnoc/structure` answers from it (C3)
- [ ] the extension watches files and sends `vnoc/filesChanged`; the core re-outlines only those files and sends `vnoc/indexChanged`
- [ ] sidebar and canvas refresh on `indexChanged`
- [ ] driver timeout (C4); driver uses the protocol crate (D1)

*Done when:* you edit a Python file, save, and the tree updates by itself in under a second.

**Step 2. Attach a doc to a function (the whole vertical slice)**
This is the heart of the product. Build it for functions only, end to end, before anything else.
- [ ] driver: port `vnoc/planIds` from spike S3
- [ ] core: the attach flow (ID → edit → `node.toml` → doc file), [`04-storage/03-writing-safely.md`](04-storage/03-writing-safely.md) §3
- [ ] core: the `.vnoc/` writer module
- [ ] extension: "Add doc" command (tree context menu + the card's `＋`): title box → the once-per-project question → attach → open the new `.md`

*Done when:* right-click a function ▸ Attach doc → an `ID:` line appears in its docstring, `.vnoc/nodes/…/` exists, and a Markdown tab is open.

**Step 3. Show docs and find nodes again**
- [ ] at start, read `.vnoc/nodes/*/*/node.toml` into the index
- [ ] resolve each documented node: marker, else anchor ([`05-identity/05-finding-nodes-again.md`](05-identity/05-finding-nodes-again.md))
- [ ] doc count badge in the tree and on the card; click opens the **doc panel** (rendered, read-only, with the doc picker and Edit) —
      [`07-ui/02-design.md`](07-ui/02-design.md) §3

*Done when:* rename the function, move it to another file, restart VS Code. The badge is still on it.

**Step 4. Files and folders**
- [ ] folder marker `.vnoc-id` (core only, no driver work) — [`05-identity/04-folders.md`](05-identity/04-folders.md)
- [ ] file marker `# FileID:`: driver `planFileId` (prototyped in S2/S3) — [`05-identity/03-files.md`](05-identity/03-files.md)
- [ ] `[ids] default` in `vnoc.toml`: `"on-attach"` (default) or `"none"` — [`05-identity/README.md`](05-identity/README.md) §3

If `planFileId` turns out slow to port, ship files with **no marker** in 0.1 (found by path) and add the comment in 0.1.x. No format changes either way.

**Step 5. Missing, Relink, Remove**
- [ ] a "Missing" group in the sidebar
- [ ] Relink: pick a node in the tree → the anchor in `node.toml` is rewritten (and a marker is written at the new place if markers are on)
- [ ] Remove: move the node folder to `.vnoc/local/trash/` (port from S3); commands "Restore removed doc" and "Empty trash"
- [ ] "ID was removed from code" → a "Write ID again" action on the node

**Step 6. Harden and publish**
- [ ] the refactor test suite: rename function · move function · rename file · rename folder (in VS Code **and** with `git mv`) → the doc stays attached every time
- [ ] the no-data-loss suite: Remove → Restore gives back the same bytes
- [ ] CI builds four VSIX files; publish 0.1 as **pre-release**
- [ ] use it yourself on this repo and on pylspt for two weeks

### Exit criteria for 0.1

1. The Step 6 suites pass.
2. A fresh clone on a second machine shows the same docs on the same nodes.
3. No file in the user's repo is ever left half-written (kill the core during an attach and check).
4. Opening a ~1,000-file project shows the tree in under 10 seconds **[measure; set the real number after Step 1]**.

## 3. 0.2 → 1.0: one increment per release

| Release | One job | Main work | Adds to frozen formats |
|---|---|---|---|
| **0.2** | **Call trees** | driver `callTree` / `expand` (port S2) · concurrent driver host · lazy expand on the canvas · SQLite **only if** measurements ask for it | nothing |
| **0.3** | **Edit in the canvas** | run spike S5 · Monaco as a view of VS Code's document · ⌘S · doc panel with a Markdown editor | nothing |
| **0.4** | **Docs survive without markers** | matching ladder steps 2–6 · rename listeners · Missing vs "not on this branch" · notifications · archive UI · the **comment** ID style | `[ids] style` in `vnoc.toml` · a second marker syntax |
| **0.5** | **Teams and privacy** | private docs (`local/nodes/`) · `local.toml` · solo mode · settings UI | `local/` side |
| **0.6+** | Polish | CodeLens · hover · search · doc freshness · playground | `body_hash` in frontmatter · playground files |
| **1.0** | **Hardening** | 10,000-file repos · Remote SSH / WSL / containers · driver protocol frozen as `1` · user guide | — |

The order of 0.4–0.6 should follow what your first users ask for. The order of 0.1–0.3 should not move.

## 4. How each release stays compatible

Four habits. You already follow the first two in `node_toml.rs`.

1. **Readers ignore what they do not know; writers keep it.** Applies to every TOML file and to doc frontmatter.
2. **One version gate, in one place.** `vnoc.toml` has `format = 1`. A V-NOC that sees `format = 2` opens the project **read-only** and says "update V-NOC".
   Nothing else carries a version.
3. **New behaviour arrives as a new optional key or a new capability flag,** never as a new meaning for an old key.
4. **Every release keeps a fixture repo from the previous release** in its tests (`tests/fixtures/vnoc-0.1/`). The test opens it with the new code and
   checks that every doc is found and no file changes on disk. This is the cheapest insurance you can buy, and it starts with 0.2.

Next: [`03-repo-structure.md`](03-repo-structure.md)
