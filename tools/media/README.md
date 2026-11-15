# tools/media: the pictures of the READMEs

Makes the GIFs and screenshots by driving a real, clean VS Code with the packaged V-NOC installed (plan: `docs/18-release-0.2/01-media.md`). A separate npm app: it is
not in the pnpm workspace and nothing else imports it.

```sh
node scripts/package.mjs --target darwin-arm64 --pre-release   # at the repository root: the VSIX it installs
cd tools/media && npm ci
node run.mjs --only g1 --stills   # no recording: one screenshot per beat and out/g1/storyboard.html (gate 1 of 04-gifs-v2.md §7)
node run.mjs --only g1            # record, edit and encode one scene; no --only = all
node run.mjs --encode-only g1     # automatic pass + edits/g1.json, then the GIF, g1.review.mp4 and review.html, from the saved frames: no VS Code, no recording
node run.mjs --only g1 --debug    # also a screenshot of every beat: out/debug/ (these add static time to the film: look at the edit list without it)
node run.mjs --only g1 --dev      # the extension from this repository, not the VSIX (title bar: Extension Development Host; for writing a scene)
MEDIA_TIMES=1 node run.mjs …      # the film's seconds at each beat
node run.mjs --only g1 --publish  # copy out/ to extension/media/screenshots/ (after you have looked at every frame: plan §8)
```

macOS only (Retina capture, the fonts people expect); not run in CI. Needs `ffmpeg` (`/opt/homebrew/bin/ffmpeg`, or `FFMPEG=`). Everything it makes goes to
`out/` (git-ignored) first. The VS Code it uses is the one in `extension/.vscode-test` (`vscodeVersion` in `config.mjs`).

## The second design (`docs/18-release-0.2/04-gifs-v2.md`)

It replaces the storyboards of 01 §5 and changes the cursor (01 §4.3) and the edit (01 §7.2). Built so far (step M1, G1 only): the window is **960 × 600**, and
`lib/cursor.mjs` is the overlay drawn into the page: the arrow (glide 450 ms, ring 350 ms), `stage.key("Meta+k Meta+m")` (presses the keys and shows a badge for 0.8 s)
and `stage.caption("…")` (a strip over the status bar, six words or fewer). A scene starts every beat with `await rec.beat("name")`: a named mark in a film
(`out/<scene>.beats.json`), a screenshot of the beat just ended with `--stills`. `lib/canvas.mjs` has what a scene needs to know about the canvas page (`settled`, `zoomOf`, `viewSettled`, `emptySpot`,
`place`, `closeContexts`). A scene records freely: no warm-up, no put-back, no `target`. The capture (`startScreencast`), the review film and the edit list by beat are M2; G2 is M3 and
does **not** run at 960 × 600 until then (it still has the 1280 × 800 layout and the old way of timing).

## The demo project

httpx at a pinned tag (`demo/PIN`), with the V-NOC docs on top: `demo/overlay.patch` (the ID lines the core wrote into the code, one renamed method) and
`demo/vnoc/` (the `.vnoc/` folder). `lib/prepare.mjs` clones httpx into the temp folder (not inside this repository), applies both and commits.

To change the demo's docs, edit `demo/docs.mjs` and run `node lib/make-overlay.mjs`. It needs the core and the Python driver built (`cargo build`, and
`cargo build` in `drivers/python`): it starts the core as a stand-in editor and sends the same requests as *Add Doc…*, so the IDs in the overlay are the core's, not
invented. Commit the new `demo/` and expect every ID to change.

## What a scene may and may not do

These cost a day to find out. Read them before writing a scene.

- **Frames come from `Page.startScreencast`** (`lib/record.mjs`, 04 §6.2): a frame arrives when the screen changes, with its own time, at the screen's own resolution, as a JPEG;
  a still stretch has no frames and the last one stays. Measured on G1 against the `Page.captureScreenshot` loop it replaced: 45 different pictures a second while the
  screen moves, against 13.7; page scale, resolution and hovers unaffected. Do not use `page.screenshot({ scale: "css" })` or a DevTools `clip.scale` while a recording runs:
  both change the page's scale for the capture. The window flickers, hovers and clicks under way are cancelled, and a clip scale can leave the layout zoomed. The GIF is 20 fps, 256 colours.
- **The pointer is drawn into the page** (`lib/cursor.mjs`): a screenshot does not contain the real one. The arrow glides; the real pointer is moved by
  Playwright's `locator.hover()` / `click()` (which reach inside webviews and check what they hit), not along the way: a straight line over a list hovers every row
  and leaves tooltips behind. The arrow is built with DOM calls: VS Code requires Trusted Types, `innerHTML` throws.
- **Commands go through the palette only when no recording runs**, and `runCommand` refuses a first entry that is not the title (a blind `Enter` ran *Toggle
  Panel Visibility* once). A GIF uses only the mouse, so the viewer sees what a person does.
- **Rows and cards are found by their accessible names** (`webview/src/node-card/labels.ts`), never by CSS classes. A row's buttons exist only while it is hovered.
  A file row opens the file when clicked and takes the keyboard: use `stage.openRow`, which checks `aria-expanded`.
- **The camera is set before recording, never dragged during it** (plan §5.1), with one exception: G1 may make **one** smooth drag of at most 600 ms, and only
  if its last card is not fully in view. G2 has none. The camera does not follow an expansion and React Flow draws only the cards in view, so the setup (off
  camera) puts the zoom (the canvas's `−` button) and the position where every column of the scene fits. Never Fit View on a deep chain: the text becomes a few
  pixels. A drag is a gesture with a cost (about a second, and a freeze before it); a scene that needs more than one has the wrong setup.
- **A recording is not timed exactly, and does not try to be** (04 §6). It keeps every frame and writes what happened as marks (`wait`, `drag`, `glide`, `typing`,
  `reveal`, `hold`) to `out/<scene>.marks.json`, the beats (`rec.beat("name")`) to `.beats.json` and the frames' times to `.frames.json`. Timing is fixed afterwards, twice, with
  no VS Code (`node run.mjs --encode-only g1`; the rules are `lib/timing.mjs`, pure, with a table test: `npm test`):
  1. **the automatic pass**: every `wait` cut to 0.3 s, a hold trimmed to its kind (`after` 0.6 s, `read` 1.0 s, `end` 3.0 s; `first` is not), typing over 2 s at 2×, and a
     stretch where nothing changes that no mark explains capped at 0.7 s (printed as `not marked: look at it`). No budget, no floor, no uniform speed-up.
  2. **the edit list**, written by a person: `tools/media/edits/<scene>.json` (committed; no file, no manual edits). Entries name a beat and give times *since the beat began on the
     review film's clock*: `{ "beat": "show-calls", "from": 0.4, "to": 1.6, "cut": true }`, `{ "beat": "explore", "from": 0, "to": 0.9, "speed": 1.5 }` (no `from`/`to`: the whole beat),
     `{ "beat": "end", "hold": 3.0 }` (how long the beat's last hold stays, longer or shorter). A beat that does not exist, an entry that is not exactly one of cut / speed / hold,
     a stretch outside its beat and two that overlap are errors, not ignored.
  `out/<scene>.review.mp4` is the film after the automatic pass with the beat's name and the seconds since it began burned in (`explore +0.8`); `out/review.html` shows the GIF at
  880 px on GitHub's dark and light backgrounds, that film, the beats on its clock, what the automatic pass did and the edit list as text.
- **The capture can stall while VS Code is busy** (the first action, an attach, a save), and a frame then stays the latest picture for a second or more; the edit keeps a
  time window of a cut, and for a wait only frames *taken* in the window, never an older one that spans into it.
- A scene can change settings for itself: `export const settings = { "editor.fontSize": 12 }` (G2 does: its editor groups are narrow).
- **Wait for state, hold for time**, and `rec.pause()` / `rec.resume()` around a load the viewer would only watch (a cut, not dead time). `settled()` also waits
  for the progress badge and for "Activating Extensions…", which comes late.
- A failed scene saves `out/debug/<scene>-failure.png`: look at it before anything else.
