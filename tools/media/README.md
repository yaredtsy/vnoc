# tools/media: the pictures of the READMEs

Makes the GIFs and screenshots by driving a real, clean VS Code with the packaged V-NOC installed (plan: `docs/18-release-0.2/01-media.md`). A separate npm app: it is
not in the pnpm workspace and nothing else imports it.

```sh
node scripts/package.mjs --target darwin-arm64 --pre-release   # at the repository root: the VSIX it installs
cd tools/media && npm ci
node run.mjs --only g1            # record, edit and encode one scene; no --only = all
node run.mjs --encode-only g1     # edit and encode again from the saved frames and marks: no VS Code, no recording
node run.mjs --only g1 --debug    # also a screenshot of every beat: out/debug/ (these add static time to the film: look at the edit list without it)
node run.mjs --only g1 --dev      # the extension from this repository, not the VSIX (title bar: Extension Development Host; for writing a scene)
MEDIA_TIMES=1 node run.mjs …      # the film's seconds at each beat
node run.mjs --only g1 --publish  # copy out/ to extension/media/screenshots/ (after you have looked at every frame: plan §8)
```

macOS only
macOS only (Retina capture, the fonts people expect); not run in CI. Needs `ffmpeg` (`/opt/homebrew/bin/ffmpeg`, or `FFMPEG=`). Everything it makes goes to
`out/` (git-ignored) first. The VS Code it uses is the one in `extension/.vscode-test` (`vscodeVersion` in `config.mjs`).

## The demo project

httpx at a pinned tag (`demo/PIN`), with the V-NOC docs on top: `demo/overlay.patch` (the ID lines the core wrote into the code, one renamed method) and
`demo/vnoc/` (the `.vnoc/` folder). `lib/prepare.mjs` clones httpx into the temp folder (not inside this repository), applies both and commits.

To change the demo's docs, edit `demo/docs.mjs` and run `node lib/make-overlay.mjs`. It needs the core and the Python driver built (`cargo build`, and
`cargo build` in `drivers/python`): it starts the core as a stand-in editor and sends the same requests as *Add Doc…*, so the IDs in the overlay are the core's, not
invented. Commit the new `demo/` and expect every ID to change.

## What a scene may and may not do

These cost a day to find out. Read them before writing a scene.

- **Frames are taken with `Page.captureScreenshot` at the screen's own resolution, as JPEG** (`lib/record.mjs`), and ffmpeg scales them to 1280 wide. Do not use
  `page.screenshot({ scale: "css" })` or a DevTools `clip.scale` while a recording runs: both change the page's scale for the capture. The window flickers, hovers
  and clicks under way are cancelled, and a clip scale can leave the layout zoomed.
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
- **A recording is not timed exactly, and does not try to be** (plan §7.2). It keeps every frame and writes what happened as marks (`wait`, `drag`, `glide`,
  `typing`, `reveal`, `hold`) to `out/<scene>.marks.json`; the frames' times go to `out/<scene>.frames.json`. `lib/edit.mjs` then cuts every `wait` to 0.3 s,
  plays a `drag` at 2×, speeds up a long `typing` (1.5×) and a long glide, never touches a `reveal` or the last 2 s, caps unmarked stretches of unchanged frames at 0.7 s,
  and, if the film is still over the scene's budget, shortens the holds (longest first, down to floors) and then speeds everything else up by at most 1.15×. It prints
  each change (`G1: recorded 14.0 s → 11.8 s` and one line per segment; a `freeze` line is time the marks did not explain: look at it) and writes
  `out/<scene>.edit.json`. Add `{ "from": 6.2, "to": 7.9, "speed": 2 }` or `{ "cut": [3.1, 3.6] }` to its `manual` list and run `--encode-only` to correct an edit by hand; delete the
  file for the automatic one. In a scene, `rec.during(type, label, fn)` marks `fn`, `rec.hold(ms, kind)` marks a hold, and the cursor marks its own glides and drags.
- **The capture stalls while VS Code is busy** (the first action, an attach, a save), and a stalled frame stays the latest picture for a second or more. The recorder warms up and
  throws its first frames away; the edit keeps a time window of a cut, and for a wait only frames *taken* in the window, never an older one that spans into it.
- A scene can change settings for itself: `export const settings = { "editor.fontSize": 12 }` (G2 does: its editor groups are narrow).
- **Wait for state, hold for time**, and `rec.pause()` / `rec.resume()` around a load the viewer would only watch (a cut, not dead time). `settled()` also waits
  for the progress badge and for "Activating Extensions…", which comes late.
- A failed scene saves `out/debug/<scene>-failure.png`: look at it before anything else.
