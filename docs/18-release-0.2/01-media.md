# 18.01 The pictures: a small capture tool, a demo project, a shot list

```
media
├── 1. the tool: what it is, where it lives, why not in the app
├── 2. the demo project: httpx, pinned, with docs on it
├── 3. the look: window, theme, settings
├── 4. how it drives VS Code (Playwright, webviews, the fake cursor)
├── 5. the GIFs: story, beats, length
├── 6. the screenshots: what each one shows, crop, where it is used
├── 7. from frames to files (ffmpeg), sizes
└── 8. the review: what a person checks before a picture is committed
```

Step R4 of the README. Everything here is made by a script so that 0.3 can make its pictures again with one command.

> **2026-11-15: §5 (the GIFs) is replaced, and §4.3 (the cursor) and §7.2 (the edit) are changed, by
> [`04-gifs-v2.md`](04-gifs-v2.md)**, written after watching the first `calls.gif` and `docs.gif`.

## 1. The tool

A **separate small Node app** in `tools/media/`. It is **not** in `pnpm-workspace.yaml` and nothing in `extension/`, `webview/` or `shared/`
imports it. It uses **npm** (its own `package-lock.json`), so pnpm never treats it as part of the workspace.

```
tools/media/
├── package.json         private; deps: playwright-core, @vscode/test-electron. Nothing else
├── README.md            how to run it, what it makes, how to remake the demo overlay
├── config.mjs           pinned versions (VS Code, httpx), window size, output paths
├── settings.json        the VS Code user settings for every capture (§3)
├── demo/
│   ├── PIN              httpx: tag and commit
│   ├── overlay.patch    the ID lines V-NOC wrote into httpx for the documented nodes
│   └── vnoc/            the .vnoc/ folder (docs), without local/
├── run.mjs              prepare → install → launch → scenes → encode
└── lib/
    ├── prepare.mjs      clone httpx into a temp folder, apply the overlay, commit
    ├── vscode.mjs       launch, window size, palette, webview frames, waits
    ├── cursor.mjs       the fake cursor and the click ring (§4.3)
    ├── record.mjs       the frame loop and the ffmpeg calls (§7)
    └── scenes/          one file per GIF or screenshot: g1-calls.mjs, s1-overview.mjs, …
```

Run: `cd tools/media && npm ci && node run.mjs` (all), `node run.mjs --only g1,s3`, `--keep-frames`, `--vsix <file>`. Plain `.mjs`, no build
step. macOS only (Retina screenshots, the fonts people expect); not run in CI. `tools/media/out/` is git-ignored; the results are written to
`extension/media/screenshots/`.

**Why Playwright (`playwright-core`)**: it is the library without the browsers (it downloads nothing), and its `_electron` API launches
VS Code itself. The alternatives are worse here: raw DevTools Protocol means writing waits and iframe handling by hand; WebdriverIO's VS Code
service and `vscode-extension-tester` (Selenium) are test frameworks with far more setup; recording the real screen catches your own windows,
notifications and Dock.

**What it tests**: the packaged VSIX, installed into a clean `--extensions-dir`, not `--extensionDevelopmentPath`. A development host writes
*[Extension Development Host]* in the title bar, and the pictures must show what users install.

## 2. The demo project

**httpx** (`encode/httpx`, BSD-3-Clause), at tag **`0.28.1`** **[check: the newest tag on the day; write tag and commit in `demo/PIN`]**.
60 files, 1,134 functions, call trees of a thousand merged nodes (14 §11): big enough to look real, small enough to load in seconds, and every
Python developer knows `httpx.get`. The README credits it: *"Code shown: httpx (BSD-3-Clause)."*

### 2.1 Prepare (every run)

1. `git clone --depth 1 --branch <tag> https://github.com/encode/httpx <tmp>/httpx`, in `os.tmpdir()`, **not inside this repo** (a folder
   inside the vnoc repo would sit inside its git repo).
2. `git apply demo/overlay.patch`; copy `demo/vnoc/` to `<tmp>/httpx/.vnoc/`.
3. `git -c user.name=demo -c user.email=demo@example.com commit -qam demo`, so the Source Control badge shows nothing.

### 2.2 The overlay (generated, kept in the repo)

**As built:** the docs are written in `demo/docs.mjs`, and `node lib/make-overlay.mjs` starts the core as a stand-in editor and sends the same
requests as *Add Doc…*, so the IDs are the core's. It writes `demo/overlay.patch` and `demo/vnoc/`. To change a doc: edit `docs.mjs`, run it,
commit `demo/` (every ID changes). `tools/media/README.md` has the details.

| Node | Doc title | Text (short on purpose: it must be readable in a GIF) |
|---|---|---|
| folder `httpx/` | Overview | `# httpx` · Public API in `_api.py`; each helper opens a one-off `Client`. The real work is in `_client.py`; transports live in `_transports/`. **Start reading at `Client.send`.** |
| file `httpx/_api.py` | Module helpers | Every function opens a new `Client` and closes it. Fine for scripts; use one `Client` for anything repeated. |
| `Client.send` | How a request is sent | `send → _send_handling_auth → _send_handling_redirects → _send_single_request`, then a list of three: auth flows are generators (each `yield` is one more request) · redirects are followed here, not in the transport · `history` collects every response. Then a quote: *If a request hangs, check the pool limits first.* |
| `Client.send` | Debugging tips | a code block: `client = httpx.Client(event_hooks={"request": [print]})`. (Two docs on one node: the card shows **2 docs**.) |
| `Client._send_single_request` | One request | Sends exactly one request, no redirects. Sets `response.elapsed` when the stream closes. Cookies are stored here. (S5) |
| `HTTPTransport.handle_request` (`_transports/default.py`) | Where httpx meets httpcore | Maps an `httpx.Request` to an `httpcore.Request` and back; httpcore's exceptions come back as httpx exceptions. |
| **one Missing doc** | Old timeout notes | On `Timeout.as_dict`; the overlay then renames it to `as_mapping` without its `ID:` line, so the doc shows under NEEDS ATTENTION (S6). The renamed method shows in the code of a picture that has `_config.py` or the call site in view: keep it out of frame |
| **no doc** | — | **`Client._send_handling_redirects` must have none**: G2 adds one on camera (§5.3) |

### 2.3 The dry pass (before writing any scene)

**Done (first G1):** the chain draws, and `handle_request` in `_transports/default.py` is a callee of `_send_single_request`. Kept for 0.3: open the prepared demo by hand and **check that the chain draws**: Show Calls on `Client.send` → `_send_handling_auth` →
`_send_handling_redirects` → `_send_single_request` → a callee **in another file** (the transport's `handle_request` is the first choice;
`Cookies.extract_cookies` in `_models.py` the second). Explore needs a cross-file callee to show the canvas switching files. Write the chain you
saw into `config.mjs`. If a step does not resolve, start the GIF from the deepest function that does; do not change the story.

## 3. The look

| Setting | Value | Why |
|---|---|---|
| VS Code | pinned (`1.146.0`, the one in `extension/.vscode-test`), downloaded by `@vscode/test-electron` | same chrome on every run |
| window | **1280 × 800** logical, on a Retina Mac (2560 × 1600 pixels) | GIFs at 1280 wide are exactly 1× (sharp text); screenshots at 2× |
| profile | a fresh `--user-data-dir` and `--extensions-dir` in the temp folder; only V-NOC installed | no other extensions, no account, no history |
| theme | **Default Dark Modern**; S7 is **Default Light Modern** | what most people use; the light one proves "follows your theme" |
| layout | Activity Bar on V-NOC; sidebar ~300 px; no panel (Terminal, Problems); no secondary side bar | the canvas gets the space |

`tools/media/settings.json` (each name **[check]** against the pinned VS Code; drop a line that does not exist rather than guess):

```jsonc
{
  "workbench.colorTheme": "Default Dark Modern",
  "window.titleBarStyle": "custom",
  "window.menuStyle": "custom",            // macOS draws context menus natively by default: they would be MISSING from every screenshot
  "window.zoomLevel": 0,
  "workbench.startupEditor": "none",
  "workbench.tips.enabled": false,
  "workbench.secondarySideBar.defaultVisibility": "hidden",
  "chat.disableAIFeatures": true,
  "update.mode": "none",
  "extensions.autoCheckUpdates": false,
  "extensions.ignoreRecommendations": true,
  "telemetry.telemetryLevel": "off",
  "security.workspace.trust.enabled": false,
  "editor.minimap.enabled": false,
  "editor.fontSize": 14,
  "editor.lineHeight": 22,
  "files.exclude": { "**/__pycache__": true }
}
```

Before every shot: run `notifications.clearAll`, close the panel, and **close the Chat view** (the secondary side bar: `workbench.action.closeAuxiliaryBar`, and again after any reload, because `defaultVisibility` only sets the first-open default and `chat.disableAIFeatures` is **[check]**). The Chat tab and its empty-state panel must not appear in any picture.

## 4. How it drives VS Code

### 4.1 Launch

1. `downloadAndUnzipVSCode(version)` → the executable; `resolveCliArgsFromVSCodeExecutablePath` → the CLI; install the VSIX with
   `--extensions-dir <tmp>/ext --install-extension <vsix>` (default: the VSIX `node scripts/package.mjs` makes for this Mac).
2. `_electron.launch({ executablePath, args: [<tmp>/httpx, --user-data-dir=…, --extensions-dir=…, --skip-welcome, --skip-release-notes, --disable-workspace-trust], env })`
   with **`ELECTRON_RUN_AS_NODE` removed from `env`** (16/10 §1: a shell started from VS Code or an agent has it, and VS Code then starts as Node).
3. `app.browserWindow(page)` → `setContentSize(1280, 800)`, centre it.
4. Wait for the sidebar's tree (`role=tree`, name *Structure*) to have rows. Opening httpx takes a few seconds the first time.

### 4.2 Reaching into webviews

The sidebar, the canvas and the docs tab are webviews: an `iframe.webview` with an inner frame **[check the inner frame's id in the pinned
VS Code]**. Do not hard-code the nesting: `webviewWith(selector)` walks `page.frames()` and returns the frame that contains the selector:

| Surface | Find it by |
|---|---|
| sidebar | `[role=tree][aria-label="Structure"]` (`webview/src/sidebar/Tree.tsx`) |
| canvas | a node card (`webview/src/node-card/NodeCard.tsx`, its `aria-label`) |
| docs tab | its doc title text |

Find rows and cards **by their accessible names**, never by CSS classes: rows have `role=treeitem` and `rowLabel(…)`
(`webview/src/sidebar/Row.tsx`), cards have `cardLabel(…)`. Both start with the kind and the name (*"function send, …"*). **[check]** the exact
strings in the label functions, and the label or title of a card's chevron and its `ⓘ`.

Commands go through the palette (`F1`, type the title, `Enter`) **before a recording starts**; during a GIF only the mouse is used, so the
viewer can follow every step.

### 4.3 The fake cursor

Screenshots of an Electron page **do not contain the mouse pointer**. `cursor.mjs` adds a small arrow (an inline SVG in a fixed `div`,
`pointer-events: none`, above everything) to the workbench page. `moveTo(locator)`:

1. reads `locator.boundingBox()` (main-page coordinates, also for elements inside webviews);
2. moves the arrow there with a CSS transition (**300 ms, ease-in-out**; was 450 ms in the first G1);
3. moves the real pointer with Playwright's `locator.hover()` / `click()`, **not along the way** (a straight line over a list hovers every row and
   leaves tooltips behind). The arrow is built with DOM calls: VS Code requires Trusted Types, `innerHTML` throws;
4. `click()` / `dblclick()` play a ring around the arrow (**250 ms**) at the moment of the real click.

Screenshots (§6) hide the arrow, except S3 and S4, where it sits on the menu entry.

### 4.4 Waiting

Wait for **state**, then hold for **time**: `await card("function _send_handling_auth").waitFor()`, then wait until the canvas has no
*updating…* and no `role=status`, then `hold(ms)`. Never a fixed sleep instead of a wait: a slow first request would then be recorded as a frozen
spinner. **As built:** `settled()` also waits for the progress badge and the late "Activating Extensions…"; `rec.pause()` / `rec.resume()` cut a
load the viewer would only watch (a cut, not dead time). The warm-up of §5.1 should leave few loads to cut.

## 5. The GIFs

Rewritten 2026-11-15 after the first G1 (22.9 s, about 10 s of it frozen frames, the rest mostly canvas drags). What is below replaces the first
storyboards.

### 5.1 Rules for every GIF

| Rule | Value |
|---|---|
| one idea per GIF | G1 calls, G2 docs. Nothing from one goes into the other |
| length | **G1 12 – 13 s, G2 ≤ 15 s.** If G2 runs over 16 s, split it (G2a Add Doc, G2b Rename); never speed it up to fit |
| cursor | glides in **300 ms**; click ring 250 ms |
| holds | **0.5 s** on the first frame; **0.6 s** after an action; **1.0 s** where there is something new to read; **2.5 s** on the last frame |
| no frozen frame | **no freeze longer than 0.7 s** except the last frame. The edit enforces it (§7.2), so the timing of a run need not be exact |
| warm up | **run the scene once off camera**, put the state back (§5.4), then record. The driver keeps the project open, so a second fetch is ~20 ms instead of 200 – 500 ms |
| camera | **set before recording**, never dragged during it, except the one move G1 allows. Never Fit View on a deep chain |
| mouse only | while recording, only the mouse (and in G2 the typing). Commands through the palette only in setup |
| speed-up | per segment, after recording (§7.2): waits cut, drags 2×, the reveals never. Overall at most **1.15×**, last. Faster makes the cursor jump and the names unreadable |
| a fresh demo per scene | each scene starts from a clean clone state (`git reset --hard` to the demo commit, `git clean -fdx`) and a fresh `--user-data-dir`: G2 changes the code |

### 5.2 G1 `calls.gif`: see what a function calls, follow a call to its own file (**12 – 13 s**, the hero)

**Setup (off camera):** warm up; the canvas open on MAIN, **focused on `_send_handling_redirects`**, its card closed; sidebar about 260 px;
canvas zoom **0.8 – 0.85** (the canvas's `−` button), the card near the top left, so that three columns (the function, its calls, their calls)
fit across the canvas. Check on the first frame that they will.

| t (s) | Beat | Hold |
|---|---|---|
| 0.0 | the `_send_handling_redirects` card alone, closed; the sidebar shows MAIN with it | 0.5 |
| 0.5 | glide to its chevron, click: its call cards fan out with dashed edges (`TooManyRedirects`, `_send_single_request`, …) | 1.0 |
| 2.0 | open `_send_single_request`: its calls appear in a third column; **`handle_request · default.py` is near the top** | 1.0 |
| 3.5 | only if `handle_request` is not fully in view: **one** smooth drag, ≤ 600 ms, left/up. Otherwise skip this beat | 0.3 |
| 4.5 | glide to `handle_request`, **double-click**: the EXPLORE panel opens in the sidebar, the chip in the canvas bar, the breadcrumb becomes `_transports › default.py › HTTPTransport › handle_request` | 1.2 |
| 6.5 | glide to its `ⓘ`, click: the docs panel opens with *Where httpx meets httpcore* | |
| 7.5 | the cursor moves off the card, to the empty canvas | 2.5 (end) |

That is about 10 s of beats plus glides: **12 – 13 s**. Two levels, one jump to another file, one doc. Do not open `send` or
`_send_handling_auth`: they were the four-level version, and each level cost a drag and a second of waiting.

**Explore and the zoom.** The first G1 jumped to a very large zoom when Explore opened (a context with one card). If that is the product
(fit view on one card zooms past 100 %), it is a **product fix, its own commit, outside R4**: fit view never zooms past 1.0. If it is the scene,
fix the scene. Either way the GIF must not show it.

**Selecting a call opens an EXPLORE panel** (16/03 §4): each click on a call card changes the sidebar's EXPLORE panel. Open with the
**chevron**, not by clicking the card, and check that the sidebar shows **one** EXPLORE panel, the one of the double-click. If the chevron also
selects, accept the change; with two levels it is one change, not four.

### 5.3 G2 `docs.gif`: write a doc, rename the function, the doc stays (**≤ 15 s**)

Add Doc as built (`extension/src/addDoc.ts`): a title box pre-filled with *Overview* for a node's first doc; the "V-NOC will add an ID line…"
question only on the first attach in a repository (the demo has `.vnoc/`, so it does not appear); the core writes the `ID:` line and V-NOC saves
the file; the doc's Markdown opens. **The target is `Client._send_handling_redirects`**: it has no doc in the overlay (§2.2), and
`_follow_redirects` is a rename a reader believes.

**Setup (off camera):** the editor split in two groups: **`_client.py` on the left** at `def _send_handling_redirects` (its docstring in view),
**an empty group on the right, active**, so the doc opens there and does not cover the code **[check where `showTextDocument` puts it; move the
focus to the right group before the menu, if needed]**. The canvas is closed. The sidebar shows the row of `_send_handling_redirects` (Reveal in
Sidebar).

| t (s) | Beat | Hold |
|---|---|---|
| 0.0 | the code and the sidebar row, **no doc** on the row | 0.5 |
| 0.5 | right-click the function's name in the editor → **V-NOC ▸ Add Doc…** | 0.4 |
| 2.0 | the title box says *Overview*: click into it, `Enter` | |
| 2.5 | **the `ID:` line appears in the docstring**; the doc opens on the right. The cursor glides next to the `ID:` line | 1.2 |
| 4.5 | in the doc, type one line at ~20 characters a second: *Follows redirects: each hop is one `_send_single_request`.* · `Cmd+S` | 0.6 |
| 8.0 | the row shows **1 doc** | 0.6 |
| 9.0 | in the code, double-click `_send_handling_redirects` (selects the word), type `_follow_redirects`, `Cmd+S` | |
| 11.5 | the row becomes `_follow_redirects`, still **1 doc**; the doc is still open on the right | 2.5 (end) |

About 14 s. The rename breaks its caller in `_send_handling_auth`; that does not matter, and the next scene starts from a clean clone (§5.1).

### 5.4 Putting the state back after the warm-up

After the warm-up run of G1: **Close** every EXPLORE context (the panel's `×`), **Collapse All**, focus `_send_handling_redirects` again, set
the zoom and the position again. After G2's warm-up: `git checkout -- .` and `git clean -fd` in the clone (the ID line and the new doc), and
reload the window, or skip G2's warm-up: its only fetch is the structure, which is already warm.

### 5.5 G3 `focus.gif` (optional): narrow down and back out (**6 – 8 s**)

`⌖` on `_models.py` in the sidebar: both surfaces show only it → open `Response` → `Alt+←`: back to the whole package. Only after G1 and G2 are
reviewed.

### 5.6 Where they go

| GIF | Root README | Marketplace README |
|---|---|---|
| G1 `calls.gif` | top, under the badges | top, after the first paragraph |
| G2 `docs.gif` | *Docs that survive refactors* | under *What it does*, after the doc bullet |

## 6. The screenshots

PNG at 2× (2560 wide for full-window shots). Crops keep a **24 px margin** around what they are about.

| File | State | Crop | Used in |
|---|---|---|---|
| **S1** `overview.png` | the docs panel showing *How a request is sent*: sidebar with the search box, MAIN focused on **`send`** (not `Client`: its 20 methods shrink to nothing), one EXPLORE panel; the canvas with `send` open **one level** and one call open, at a zoom where the names read; `send` selected, its card shows **2 docs** | the whole window | both READMEs (first picture), Marketplace gallery, the GitHub social preview (a 1280 × 640 crop centred on the canvas: `out/social.png`, uploaded by hand, 02 §7) |
| **S2** `calls.png` | G1's frame after `_send_single_request` opened (two levels), with **Show Code in Card** on `_send_single_request` | the canvas only | extension README, placeholder 2 |
| **S3** `call-menu.png` | right-click on the `_send_handling_redirects` call card; menu open; arrow on *Explore* | the card and the menu | extension README, placeholder 3 |
| **S4** `editor-menu.png` | the editor on `_client.py`, cursor in `def send`; right-click → **V-NOC ▸** open | the editor | extension README, placeholder 3 |
| **S5** `docs.png` | the editor and the docs tab with *One request* side by side; the sidebar row with **1 doc** | the whole window | root README, "Docs" |
| **S6** `attention.png` | NEEDS ATTENTION with *Old timeout notes*; hover on it so Relink / Remove show | the sidebar | root README, "Docs" |
| **S7** `light.png` | S1 in Default Light Modern | the whole window | root README, Accessibility line |
| S8 `search.png` (optional) | `send` typed in the search box; the matches | the sidebar | — |

## 7. From frames to files

### 7.1 Frames

**As built:** `record.mjs` takes `Page.captureScreenshot` at the screen's own resolution, as JPEG, in a loop, and writes each with its time;
ffmpeg scales to 1280 wide. **Not** `page.screenshot({ scale: "css" })` or a DevTools `clip.scale`: both change the page's scale during each
capture (the first G1 flickered, lost hovers and clicks, and once stayed zoomed). A list file gives ffmpeg the real duration of each frame.

### 7.2 The edit: cut the waits, speed up the slow parts, after recording

A recording is never timed exactly: a fetch is slow once, a glide starts late, a drag takes a correction. So the script does not have to be
perfect. **The frames are kept, and the edit is a separate step that can be run again without recording again.** It only changes the
duration of frames in the list file (or drops frames); ffmpeg's `fps=12` then does the rest. No complex filter graphs.

**1. The script marks what happens.** `record.mjs` keeps a list of marks with times, written to `out/<scene>.marks.json`. Most come by
themselves; a scene adds the few it knows:

| Mark | Set by | Edit |
|---|---|---|
| `wait` | from an action to `settled()` (a fetch, Explore loading, "Activating Extensions…") | **cut to 0.3 s** (a cut reads as "it loaded") |
| `drag` | `cursor.drag()` | **2× faster** |
| `glide` | `cursor.moveTo()` | 1× (300 ms is already short; faster makes the arrow jump). 1.5× only for a glide longer than 500 ms |
| `typing` | the typing in G2 | 1.5× if the line takes more than 2 s |
| `reveal` | from a click to its cards drawn, Explore's switch, the doc panel opening | **never changed**: this is what the GIF is for |
| `hold` | `rec.hold(ms)` | kept, then shortened only by step 3 |

**2. What the marks miss is found in the frames.** Any stretch of unchanged frames longer than 0.7 s that is not the last hold (ffmpeg
`freezedetect`, about `-60dB`, `d=0.7`, or a compare of consecutive frames) is **cut to 0.7 s**.

**3. A length budget.** If the result is still over the scene's target (G1 13 s, G2 15 s), the holds are shortened, the longest first,
down to these floors: 0.4 s after an action, 0.8 s where there is something to read, **2.0 s at the end**. If that is not enough, a uniform speed-up
of **at most 1.15×**. If it is still long, the scene is too long: cut a beat.

**4. A person can correct it.** The edit writes `out/<scene>.edit.json` (the segments, what was done to each, in recorded and in final time) and
reads it back if it exists and was changed by hand: a person can add `{ "from": 6.2, "to": 7.9, "speed": 2 }` or `{ "cut": [3.1, 3.6] }` and run
`node run.mjs --encode-only g1`, which re-edits and re-encodes **without starting VS Code**. Delete the file to go back to the automatic edit.

The run prints what it did:

```
G1: recorded 15.8 s → 12.7 s
  wait     5.4 – 7.6   2.2 s → 0.3 s   (Explore)
  drag     3.6 – 4.4   0.8 s → 0.4 s
  freeze  10.1 – 11.0  0.9 s → 0.7 s   (not marked: look at it)
  budget   ok (target 13 s)
```

A freeze the marks did not explain means the scene waits for something it does not know about: look at it, and fix the scene if it happens on
every run. **Never** sped up or cut: a `reveal`, the frame where the `ID:` line appears (G2), the last 2 s.

### 7.3 Encoding

**GIF** (two-pass palette; ffmpeg is installed at `/opt/homebrew/bin/ffmpeg`):

```sh
ffmpeg -y -f concat -safe 0 -i frames.txt \
  -vf "fps=12,split[a][b];[a]palettegen=max_colors=128:stats_mode=diff[p];[b][p]paletteuse=dither=bayer:bayer_scale=4:diff_mode=rectangle" \
  -loop 0 calls.gif
```

Then, if `gifsicle` is installed (`brew install gifsicle`): `gifsicle -O3 --lossy=30 calls.gif -o calls.gif`.

**Budget: each GIF ≤ 6 MB** (G1 is the most important one; if it is over, try in this order: fewer colours (96), 10 fps, cut a beat, never a
smaller width). **Each PNG ≤ 1 MB**; if it is over, `pngquant --quality 80-95` (optional install).

**Also an MP4 of each GIF** in `out/` (not committed: for Discord and posts, 02 §9):
`ffmpeg -f concat -safe 0 -i frames.txt -vf fps=30 -c:v libx264 -pix_fmt yuv420p -crf 20 calls.mp4`.

**Where the files go**: `extension/media/screenshots/` (`calls.gif`, `docs.gif`, `overview.png`, …). `.vscodeignore` already keeps them out
of the VSIX; the Marketplace README links them by `https://` URL (02 §2).

## 8. The review (a person, before the commit)

The tool cannot judge whether a picture sells the product. Open every file at 100 % and check:

- [ ] no *[Extension Development Host]*, no notification, no "Restricted Mode", no update badge, no red squiggles
- [ ] **no Chat tab or Chat panel** (secondary side bar closed), no Copilot / Chat icon or "Ask" box in the title bar or status bar
- [ ] no personal path (the window title says `httpx`, not `/var/folders/…`)
- [ ] no spinner, no *updating…*, no half-drawn edge on any held frame
- [ ] the cursor is visible in every GIF frame and every click shows its ring
- [ ] the text is readable on the GitHub README at its normal width (about 880 px): look at it in the browser, not only in Preview
- [ ] the selection ring and the dashed call edges are visible in both themes
- [ ] G1 and G2 loop without a jarring jump; each is under 6 MB
- [ ] **watch each GIF play**, at its real speed, in a browser (a contact sheet does not show stalls or a jumpy cursor)
- [ ] G1 is 12 – 13 s, G2 ≤ 15 s; every line of the run's edit list was looked at, above all a `freeze` the marks did not explain
- [ ] G1: one EXPLORE panel at the end; no zoom jump when Explore opens; the three columns read without a drag (or with the one allowed)
- [ ] G2: the `ID:` line is on screen when it appears; the doc did not cover the code
- [ ] the docs in the pictures say something true about httpx
