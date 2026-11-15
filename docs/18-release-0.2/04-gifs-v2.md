# 18.04 The GIFs, second design: the script first, record freely, fix it in the edit

```
gifs v2
├── 1. the review: what is wrong with calls.gif and docs.gif as built
├── 2. the decisions (2026-11-15)
├── 3. the rules of a script (start, middle, end; mouse and keys)
├── 4. G1 calls.gif: "What does httpx.get() do?"
├── 5. G2 docs.gif: "Write a doc. Rename the function. The doc stays."
├── 6. the tool: record freely → review with a clock → an edit list → encode
├── 7. the gates: who looks at what, and when
├── 8. product fixes the films showed (outside R4)
├── 9. the build order
└── 10. not chosen: a film drawn from stills
```

Written 2026-11-15 after watching `tools/media/out/calls.gif` and `docs.gif` (commit `be402f7`); §2 – §7 rewritten the same day after your
answers. It replaces **01 §5** (the storyboards) and changes **01 §4.3 and §7.2** (the cursor, the edit). The rest of 01 stays.

## 1. The review

What is good and stays: the demo project and its overlay, the clean profile, the VSIX install, finding rows and cards by accessible name,
"wait for state", the frames kept after recording, `--encode-only`.

### 1.1 The story

| # | Seen in the film | Why it hurts |
|---|---|---|
| 1 | **G1 opens on nine look-alike cards** (`Client`'s methods, cut off at the top and bottom), the one that matters is written `_send_handling_redir…`, and 60 % of the canvas is empty | The viewer does not know where they are or which card to look at |
| 2 | The function was chosen because its chain fits (`_send_handling_redirects`), not because anyone asks about it | No question, so no answer at the end. 01 §2 says it itself: every Python developer knows `httpx.get` |
| 3 | No code is ever shown in G1 | A README visitor knows code, not V-NOC's canvas. Nothing familiar to start from |
| 4 | At 3.0 s the sidebar changes (a MAIN header and an EXPLORE panel appear, the list moves) because the chevron also selected the card | The eye goes left for a change the story does not need |
| 5 | The third column runs off the bottom; there are two `handle_request` cards | The target of the next click is one of two equal names |
| 6 | **Explore is a jump cut**: 2.6 s (the product's zoom jump, the scene's zoom clicks and a drag to undo it) cut to 0.3 s | The viewer cannot connect "that card" to "this canvas". The cut hides what the product does (§8 P1) |
| 7 | G1 ends on a doc panel with thirty lines of text | 01 §5.1 says one idea per GIF. The last frame is not the answer to anything |
| 8 | **G2 opens with a third of the window empty** (the VS Code watermark in the empty editor group), the code cut off on the right | The first frame is the poster. It shows nothing |
| 9 | G2: the eye goes sidebar bottom-left → title box top → code → doc right → sidebar → code → sidebar | Each change happens somewhere else than the last one |
| 10 | G2's proof is a `1` on a sidebar row, and the open doc still says `hint: …_send_handling_redirects` | The point of the GIF is the smallest thing on screen (and see §8 P3) |

### 1.2 The frames and the way of working

| # | Measured | Why it hurts |
|---|---|---|
| 11 | Card names are about 10 px high in the 1280 px film. GitHub shows a README image at about 880 px: **about 7 px** | Not readable. A 1280 × 800 window is mostly chrome and empty canvas |
| 12 | One frame per 64 ms (median), six stalls over 0.25 s in G2 (the longest 1.0 s). The GIF is 12 fps: a cursor glide is **4 frames**, a click ring **2** | The cursor jumps. This is most of "feels off" |
| 13 | `calls.gif` is 0.70 MB, `docs.gif` 0.45 MB. The budget is 6 MB | 12 fps and 128 colours save space nobody needs |
| 14 | The scene tries to be the right length while it records: a warm-up run, a put-back (about 40 lines in `g1-calls.mjs`), exact holds; then `edit.mjs` forces a length (floors, a 1.15× speed-up). G2: 24.2 s forced to exactly 15.0 s | The script worries about speed, and the flow breaks. Speed is the edit's job, and a person's |
| 15 | Two GIFs took from 17:47 to 19:49, each try a full launch, judged by seconds and freezes | Nobody said "this script is right" before a film was made |

## 2. The decisions (2026-11-15)

| # | Decision |
|---|---|
| 1 | **Record, then post-process.** A take does not have to be right in time. It is recorded slowly and safely; afterwards a person marks seconds to **cut** or **speed up** (§6). The scene never thinks about length |
| 2 | **G2 keeps the rename, as its own act.** What confused in the first film was the rename happening while the doc was being added. Now: add, a pause, then rename (§5) |
| 3 | **The doc's text is typed fast or pasted.** Nobody needs to watch typing |
| 4 | **Not everything with the mouse.** Keys where a user would use them; each key shows as a badge (§3). **G2 is keys only** |
| 5 | **`F` is Fit View on the canvas** (§8 P4): a product change, made before G1 is recorded |
| 6 | The scripts of §4 and §5 are this file's; the session that builds them may improve a beat at gate 1 and says what it changed |

## 3. The rules of a script

1. **One question, in the viewer's words.** G1: *What does `httpx.get()` do?* G2: *Does my doc survive a rename?*
2. **Start (the first 1.5 s): something the viewer knows.** Code, with a name they know, large. Never V-NOC's own UI first.
3. **Middle: one action, one change, where the eye already is.** The next action starts next to the last change. Nothing else on screen
   changes. At most **seven cards** on screen.
4. **End: the answer, as one frame that also works as a still**, held 2.5 – 3 s.
5. **3 – 6 actions, 10 – 15 s after the edit.** Over that: cut a beat from the script, not seconds from every hold.
6. **Mouse for what a new user would click** (a menu, a chevron, a card). **Keys for what a user does with keys**: confirm (`Enter`), save
   (`⌘S`), frame the view (Fit View), go back (`Alt+←`). Only keys that **work in the shipped product**: the film must not teach a
   shortcut nobody has. What exists today:

   | Where | Key | Does |
   |---|---|---|
   | canvas (`webview/src/canvas/keys.ts`) | `↑` `↓` `←` | move the selection |
   | canvas | `→` | open the selected card; open already: go to its first call |
   | canvas | `Enter` | on a call card: **Explore**; on a structure card: Focus |
   | canvas | `Space` · `Esc` · `⌘Enter` · `Alt+←` | toggle · clear the selection · Go to Code / Call Site · Focus Back |
   | sidebar | `/` or `⌘F` · `⌘Enter` | search · Focus |
   | VS Code | `Enter` in a title box · `⌘S` · `⌘K ⌘M` (maximise the editor group **[check in 1.146]**) | |
   | VS Code | `F1` (the palette) · `⌘W` · `⌘D` (selects the word at the cursor) | |
   | canvas | **`F`: Fit View** | new, §8 P4 |

7. **A key is shown.** A small badge (`⏎`, `⌘S`, `F`) at the bottom centre of the window for 0.8 s, drawn into the page like the arrow.
8. **A caption for every step, six words or fewer**, in a strip that covers the status bar (the only part of the window the story never
   needs). Drawn into the page too, so a cut or a speed-up can never put it out of step with the picture.
9. **What is shown is what the product does.** A cut removes waiting. It never hides something ugly: that is a product fix (§8).

## 4. G1 `calls.gif`: "What does `httpx.get()` do?" (9 – 11 s after the edit)

**Changed 2026-11-15 after gate 1 (the stills of commit `d93799b`).** The first script went on to open `send` and Fit View: at 100 % the
five calls were off the right edge, and after Fit View the picture was at 56 % (names about 7 px), with a caption that named `get`, which
was not on screen. The film now **ends on Explore**, the largest and clearest picture, and `F` is used earlier, where it makes the cards
bigger instead of smaller.

The chain, asked from the core (`vnoc/callTree` on the demo clone) and seen in the stills:

```
get  (_api.py)
└── request  (_api.py)
    ├── Client  (_client.py)
    └── request  (_client.py, Client's method)   ← Explore this one
        ├── build_request
        └── send
```

Never more than four cards on screen.

**Setup (off camera):** the window 960 × 600; *Focus* on `get` (through its sidebar row: without it the canvas of beat 2 is the whole
project), then the sidebar closed; the Activity Bar hidden; `httpx/_api.py` with `def get(` a third of the way down, **nothing selected
or highlighted in the code** (`editor.occurrencesHighlight` and `editor.selectionHighlight` off), the arrow at rest **right of the
signature, on no text**. No warm-up, no put-back.

| # | Beat | Action | With | The viewer sees | Caption |
|---|---|---|---|---|---|
| 1 | `start` | — | | the code of `get` in `_api.py` | *What does httpx.get() do?* |
| 2 | `show-calls` | right-click `get` → **V-NOC ▸ Show Calls** | mouse | the canvas opens beside the code: `get` and its one call, `request` | *Right-click → Show Calls* |
| 3 | `maximise` | maximise the canvas | **`⌘K ⌘M`** | the canvas takes the window | *Maximise the canvas* |
| 4 | `open-request` | the chevron of `request` | mouse | `Client` and `request · _client.py` | *Open a call: one level deeper* |
| 5 | `fit` | Fit View | **`F`** | the canvas glides: the four cards centred and larger (the product's own 300 ms move) | *F: fit the view* |
| 6 | `explore` | click `request · _client.py`, then Explore | mouse, then **`Enter`** | the breadcrumb becomes `httpx › httpx › _client.py › Client › request`; the cards are `request` → `build_request`, `send` | *Enter: follow it to its file* |
| 7 | `code` | the `</>` button of the `request` card (*Show Code in Card*) | mouse | **the function's own code opens inside the card** (about fourteen lines, coloured, with the file's line numbers) | *A card carries its code* |
| 8 | `end` | — | | the arrow moves to empty canvas; **3 s** | *It builds a request and sends it* |

Beat 7 was added 2026-11-15 (your idea): a node is not only a name, it carries its slice of the code. Nothing is scrolled. The card
grows by 220 px, so **[check in the stills]** that `request`, `build_request` and `send` are all still whole on screen; if not, press
`F` once more after it (and then consider dropping beat 5, so that `F` appears once). If the stills are better with the code on `send`
(a shorter signature), use `send`.

The last caption is the answer to the first, and it is what the last picture shows. **[check in the stills]**: after beat 5 the cards are
not smaller than before it (if Fit View does not make them larger, drop beat 5); `F` works straight after the chevron (§8 P5).
Accepted for 0.2: a `request ×` chip appears in the bar at beat 4 (§8 P2).

## 5. G2 `docs.gif`: "Write a doc. Rename the function. The doc stays." (13 – 15 s after the edit)

The first `docs.gif` was confusing because the rename happened in the middle of adding the doc, in a window with four things in it.
This one has **two panes that never move, two acts with a pause between them, and one mouse gesture** (the right-click that starts it; the arrow then
leaves and is hidden): every other step is a key with its badge. The left pane is where the hands are; the right pane only answers.

**Setup (off camera):** the window 960 × 600; the sidebar and the Activity Bar closed; `httpx/_client.py` on the left with
`def _send_handling_redirects(` a third of the way down and **the text cursor in the name**; on the right **the docs tab** (*Show Docs*,
unpinned: it shows the cursor's node), which says this function has no doc. `_send_handling_redirects` has none in the overlay (01 §2.2).
**[check in the stills]** that the `def` line and the docs tab's text both fit at this width; if not, the window is 1100 × 640 for this scene.

| # | Act | Keys | Left: the code | Right: the docs tab | Caption |
|---|---|---|---|---|---|
| 1 | | — | `def _send_handling_redirects(` | no doc yet | *A function. No doc yet.* |
| 2 | **1: add** | **mouse** (changed 2026-11-15, the same gesture as G1's Show Calls): right-click the name → **V-NOC ▸ Add Doc…** | the menu, then the title box with *Overview* | | *Right-click → Add Doc…* |
| 3 | | `⏎` | `overview.md` opens as a tab over the code | | *(the same)* |
| 4 | | the text, typed at 15 ms a key or pasted (`⌘V`) | three short lines (below) | | *Write it in Markdown* |
| 5 | | `⌘S`, then `⌘W` | the code is back, with **the new `"""ID: …"""` line** under the `def` | **the doc appears, rendered** | *Saved: one ID line links them* |
| 6 | *the pause: 1.2 s, nothing moves* | | | | *(the same)* |
| 7 | **2: rename** | `⌘D` (selects the name), type `_follow_redirects` | the name changes | | *Now rename the function* |
| 8 | | `⌘S` | | **the header becomes `_follow_redirects`; the doc is still there**; the end, 3 s | *The doc is still attached* |

The text of beat 4 (true about httpx, short enough to read on the last frame):

```md
Follows redirects, one request per hop.

- stops at `max_redirects`
- every hop is added to `history`
```

Why it reads now: one act is finished and held before the next begins; each act ends with the right pane changing, so cause is always
left and effect always right; the raw file with its `hint:` line is closed before the rename. All keys work in the shipped product (the
palette, `⌘S`, `⌘W`, `⌘D`): no new key binding is needed. **[check in the stills]**: the palette's *Add Doc…* takes the cursor's
function; `⌘W` returns to `_client.py`; the docs tab shows the new doc after the save (its header following the rename is confirmed, §8 P3).

## 6. The tool

### 6.1 The scene records freely

A scene's only job is that **every step happens and is on film**. It does not try to be short.

- It waits for state as today, and holds a generous **1.5 s** after every change (`rec.hold`), 3 s at the end.
- No warm-up, no put-back, no `target`, no budget: all removed from `g1-calls.mjs`, `g2-docs.mjs` and `edit.mjs`.
- Every beat of §4 and §5 is a **named mark**: `rec.beat("explore")`. The edit list refers to beats by name (§6.3).
- `settled`, `zoomOf`, `emptySpot`, `place`, `closeContexts` move from `g1-calls.mjs` to `lib/canvas.mjs`.
- `lib/cursor.mjs` grows into the overlay: the arrow (glide **450 ms**, ring **350 ms**), `stage.key("Enter")` (presses the key and
  shows its badge), `stage.caption("…")`.

### 6.2 The capture

| | As built | v2 |
|---|---|---|
| window | 1280 × 800 | **960 × 600**: the README shows it nearly 1 : 1, and a frame is half the pixels, so the loop is faster |
| frames | a `Page.captureScreenshot` loop, about 15 a second, with stalls | **`Page.startScreencast` [check]**: frames arrive when the screen changes, each with its own time. If it does not work in VS Code's window, keep the loop: the smaller window alone helps |
| GIF | 12 fps, 128 colours | **20 fps, 256 colours** (50 ms a frame is exact in a GIF); still far under 6 MB |

### 6.3 The edit: first the machine, then a person with a clock

1. **The automatic pass** (what `edit.mjs` does today, minus the budget): every `wait` cut to 0.3 s; an unmarked freeze capped at 0.7 s;
   a `hold` trimmed to its kind (after 0.6 s, read 1.0 s, end 3.0 s); long typing 2×. No floors, no uniform speed-up, no target.
2. **The review film**: `out/<scene>.review.mp4` is the film after the automatic pass with a **clock burned in**: the beat's name and the
   seconds since it began (`explore +0.8`). `out/review.html` shows it next to the GIF at 880 px on GitHub's dark and light
   backgrounds, with the edit list beside it.
3. **The edit list**, written by a person while watching the review film: `tools/media/edits/<scene>.json`, **committed** (it is part of
   how the picture is made). Times are **relative to a beat**, so a new recording does not make the list wrong:

   ```json
   [
     { "beat": "show-calls", "from": 0.4, "to": 1.6, "cut": true },
     { "beat": "explore",    "from": 0.0, "to": 0.9, "speed": 1.5 },
     { "beat": "write",      "speed": 2 },
     { "beat": "end",        "hold": 3.0 }
   ]
   ```

   `cut` removes the stretch; `speed` plays it faster (a whole beat when there is no `from`/`to`); `hold` sets how long a beat's last
   frame stays. Guides, not limits: speed **up to 1.5×** where the cursor moves (faster and it jumps), up to 3× for typing; never inside
   the 0.3 s after a click (the ring and the change must be seen).
4. `node run.mjs --encode-only g1` applies both passes and encodes again **without VS Code**: a few seconds. This is the loop a person
   stays in until the film is right. A new recording is needed only when the **script** changes.

## 7. The gates

| Gate | Who | Looks at | Costs |
|---|---|---|---|
| **0. the script** | you | §4 and §5 of this file | reading |
| **1. the stills** | you | `node run.mjs --only g1 --stills`: the scene without recording, one screenshot per beat with its caption, in `out/g1/storyboard.html`. Does it read as a story with no motion? | about a minute |
| **2. the take** | you | `out/review.html`: every step is on film and nothing wrong is on screen. Timing does not matter yet | one recording |
| **3. the edit** | you (or the session, then you) | the edit list and `--encode-only`, until it flows; then 01 §8 | seconds per try |

Rules for the session that builds it: **stop at every gate and show the page**; never record again to fix timing; after two takes that
fail for the same reason, stop and ask; **stage files by path, never `git add docs` or `git add .`**.

## 8. Product fixes the films showed (their own commits, outside R4)

| # | What | Where | Recommendation |
|---|---|---|---|
| **P1** | A new EXPLORE context opens zoomed far in (the scene clicks `−` up to eight times to undo it) | `webview/src/canvas/Canvas.tsx:134`: `fitView={saved === null}` has no `fitViewOptions`, so the first fit may zoom to React Flow's limit **[check]** | `fitViewOptions={{ maxZoom: 1 }}`. **Before G1 is recorded**: beat 5 shows what the product does |
| **P2** | The chevron of a call card also selects it: an EXPLORE chip and a sidebar panel appear for a card the user only opened | 16/03 §4 | decide: opening is not selecting. The chip in the canvas bar shows in G1's beat 4 |
| **P3** | After a rename the **docs tab's header follows** (confirmed by you, 2026-11-15). The raw `.md` file's `hint:` line keeps the old name | the doc's `hint:` line | nothing for G2: the raw file is closed before the rename (§5 beat 5). Decide later whether `hint:` should follow |
| **P4** | Fit View has no key | `webview/src/canvas/keys.ts`, its table test, 16/08-menus, the CHANGELOG | **decided: `F` on the canvas**, not in a text field or a code card (as the other canvas keys). Before G1 |
| **P5** | Straight after a chevron is clicked, `F` does nothing: the chevron keeps the focus and the canvas keys leave a focused button alone | `webview/src/app/useCanvasKeys.ts` | `F` is not a button's key (Space and Enter are): let `F` through when a button has the focus, still not in a text field or a code card. With its test |

## 9. The build order

Each is one commit, `release 0.2 R4: …`.

| Step | What | Proof |
|---|---|---|
| **M1** | the overlay (key badge, caption, slower arrow), `rec.beat`, `--stills` and the storyboard page, `lib/canvas.mjs`; the window 960 × 600 | gate 1 for G1 with §4's script (P1 and P4 first) |
| **M2** | the capture (§6.2) and the edit (§6.3): the budget out, the review film with its clock, `edits/<scene>.json` by beat, `review.html` | gates 2 and 3 for G1 |
| **M3** | G2 as §5 | gates 1 – 3 for G2 |
| **M4** | the screenshots of 01 §6; `tools/media/README.md`; 01 §4.3, §5, §7.2 say "see 04" | `node run.mjs` makes everything; 01 §8 |

## 10. Not chosen: a film drawn from stills

The first version of this file took one screenshot per state and drew the film (cursor, fades, a camera) from them, with the timing
written in the scene. It gives the same film on every run and a smooth cursor at any frame rate, but it is a new renderer, it loses the
product's own motion (the 300 ms Fit View of G1's beat 7), and you prefer to fix a real recording by hand. Kept here as the way out if
the capture of §6.2 cannot be made smooth.
