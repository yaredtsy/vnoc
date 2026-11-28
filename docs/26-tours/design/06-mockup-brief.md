# 06 The mockup: what to build, the fixture, the fake clock

```
mockup brief
├── 1. what it is
├── 2. what is real and what is faked
├── 3. the window
├── 4. the fixture: the tour of checkout_order
├── 5. the fake agent
├── 6. the demo strip
├── 7. the frames to reach
└── 8. not to build
```

For whoever builds `walkthrough/mockup.html`. Read first: the [README](README.md) §4 (the one picture), [`03-stage.md`](03-stage.md),
[`04-route-and-map.md`](04-route-and-map.md). Then [`02-flow.md`](02-flow.md) for the ten frames and [`05-states.md`](05-states.md) for the
states. Where two files differ, the one with the higher number is about the smaller thing and wins for that thing.

## 1. What it is

One file, `walkthrough/mockup.html`. Plain HTML, CSS and JS, no build step, codicons from the CDN: the same way as
[`../../16-ui-redesign/mockup.html`](../../16-ui-redesign/mockup.html). **Start from that file's shell**: its VS Code window (activity bar, side bar,
canvas, status bar), its colour variables for Dark+ and Light+, its card, its bar, its bottom strip. Copy what you need into the new file;
do not link to it.

Its purpose is one question: **is a tour comfortable to read and easy to move in?** So the reading has to be real (real code, real
sentences, real keys), and everything else can be a picture.

## 2. Real and faked

| Real: it has to work | Faked |
|---|---|
| the stage: every kind of step of 03 §3 except *contents*; the lit lines; the code's scroll rule; the fixed height; the drag on its top edge; the mini bar | the agent: a clock (§5) |
| Next, Prev, next / previous stop, jumps from the route, the map, the segments, a call in the code, `↳ called on line`, **Go there** | the code: the Minicart files pasted in as strings, coloured by a few regular expressions (keywords, strings, comments, `def` names) |
| `←` "where I was" with its stack; the keys of 05 §6 | MAIN: a static panel and a static canvas with three cards, enough to switch to and back |
| the marks: seen, here, ahead, not written, being written, explained before, fallback, changed | the docs panel `ⓘ`: one static doc |
| the route: only the current stop unfolded; the foot line | menus, search, the palette's **Open Tour…** |
| the TOURS section ([07](07-tour-list.md)): folded, the list, the route, `‹ Tours`; All and On the selected node, following MAIN's selection; opening at the cover, at where you left, at a node's stop; Delete with Undo; `＋` copies | the four other tours of §4.1: they open on their cover and their route, with no texts |
| the map: cards with badges, dashed and heavy edges, the ring, the dotted outline; drag to pan, wheel to zoom; the camera rules of 04 §4 | the agent's terminal of frame 1: a static block shown before the tour starts |
| the bar: the chips, the crumbs, the agent's text and `■`; Main ⇄ tour keeps the step and the viewport | |
| writing → stopping → stopped; no news; the cover; the end card | |
| the narrow layout (the stage stacks under 640 px) | |

## 3. The window

| Part | Size |
|---|---|
| activity bar | 48 px |
| side bar | 300 px: MAIN's panel (a third), the splitter, the tour's panel (the rest) |
| canvas | the rest. No code editor group: the tour does not use one |
| the bar | 35 px |
| the stage | 215 px by default (the header 30 px, nine code lines of 19 px, 14 px of padding), under the map |
| a card | 300 × 80 px; columns 380 px apart, rows 100 px apart |
| status bar | 22 px, with the tour's item on the left |

Fonts: 13 px UI, 12.5 px mono for code. Sizes of the rows, the bar and the icons: as the redesign's mockup.

## 4. The fixture

Minicart's `checkout_order`, two levels of calls: **10 stops, 20 steps**. The code is real: copy the files, with their line numbers, from
`/Users/yared/Documents/Programing/ide/v-noc/examples/minicart/app/` (`checkout.py`, `inventory.py`, `payments.py`, `orders.py`). Each stop
shows its own lines only (`start_line` to `end_line`), numbered as in the file.

The texts are written by hand, inside the budgets of 03 §5 for normal verbosity. `file` is here for the mockup only (the real file names a
node, not a path). `first_seen`, `via_line` and `step_estimate` are the fields 05 §7 asks for.

```json
{
  "id": "w_5d20",
  "schema": 1,
  "status": "generating",
  "start_node": "app/checkout.py::checkout_order",
  "depth": 2,
  "query": null,
  "verbosity": "normal",
  "commit": "f4d6e15",
  "step_estimate": 20,
  "stops": [
    {
      "id": "s0", "name": "checkout_order", "kind": "function", "level": 0, "parent": null, "mode": "full",
      "file": "app/checkout.py", "start_line": 11, "end_line": 24, "via_line": null,
      "intro": "checkout_order turns an order request into a completed order: it holds the stock, takes the money, and records the sale, in that order.",
      "blocks": [
        { "start_line": 11, "end_line": 16, "focus": "Check the stock, then hold it",
          "text": "Nothing is charged until the items are set aside. `check_stock` only looks; `reserve_stock` takes the units out of the count, so a second buyer cannot get them while this payment runs." },
        { "start_line": 18, "end_line": 24, "focus": "Charge, and give the stock back on a timeout",
          "text": "If the payment gateway times out, `cancel_checkout` returns the reserved units and the error goes on to the caller. Only a charge that worked reaches `create_order`, which needs its transaction id." }
      ]
    },
    {
      "id": "s1", "name": "check_stock", "kind": "function", "level": 1, "parent": "s0", "mode": "full",
      "file": "app/inventory.py", "start_line": 12, "end_line": 15, "via_line": 13,
      "intro": "check_stock answers one question: are there at least `quantity` units of this SKU? An unknown SKU counts as zero. It reads and never changes the stock.",
      "blocks": []
    },
    {
      "id": "s2", "name": "reserve_stock", "kind": "function", "level": 1, "parent": "s0", "mode": "full",
      "file": "app/inventory.py", "start_line": 19, "end_line": 28, "via_line": 16,
      "intro": "reserve_stock takes units out of the available count, so that nobody else can buy them.",
      "blocks": [
        { "start_line": 19, "end_line": 22, "focus": "Refuse what is not there",
          "text": "It checks the stock again by itself. `checkout_order` already did, but a caller that skips the check must not push the count below zero." },
        { "start_line": 24, "end_line": 28, "focus": "Take the units, warn when few are left",
          "text": "The count drops at once; there is no separate \"reserved\" number. With `LOW_STOCK_THRESHOLD` (2) or fewer left it logs a warning and carries on: low stock is news, not an error." }
      ]
    },
    {
      "id": "s3", "name": "check_stock", "kind": "function", "level": 2, "parent": "s2", "mode": "contextual",
      "file": "app/inventory.py", "start_line": 12, "end_line": 15, "via_line": 21, "first_seen": "s1",
      "intro": "Called again here: reserve_stock does not trust its caller to have checked.",
      "blocks": []
    },
    {
      "id": "s4", "name": "charge_payment", "kind": "function", "level": 1, "parent": "s0", "mode": "full",
      "file": "app/payments.py", "start_line": 20, "end_line": 23, "via_line": 19,
      "intro": "charge_payment is the name the rest of the app uses for \"take the money\". It only hands the order to `retry_charge`, so the retry rule can change without touching checkout.",
      "blocks": []
    },
    {
      "id": "s5", "name": "retry_charge", "kind": "function", "level": 2, "parent": "s4", "mode": "full",
      "file": "app/payments.py", "start_line": 7, "end_line": 17, "via_line": 23,
      "intro": "retry_charge sends the charge to the gateway, and tries once more if the gateway times out.",
      "blocks": [
        { "start_line": 7, "end_line": 14, "focus": "Send the charge, up to two times",
          "text": "Each attempt calls `send_charge` with the order's total and id. The first one that works returns the transaction id, and that ends the loop." },
        { "start_line": 15, "end_line": 17, "focus": "Give up only on the last attempt",
          "text": "A timeout is swallowed unless it was the last attempt. Then it is raised again, and `checkout_order`'s `except` gives the stock back." }
      ]
    },
    {
      "id": "s6", "name": "cancel_checkout", "kind": "function", "level": 1, "parent": "s0", "mode": "full",
      "file": "app/checkout.py", "start_line": 28, "end_line": 30, "via_line": 21,
      "intro": "cancel_checkout undoes the one thing `checkout_order` did before charging: it gives the reserved units back.",
      "blocks": []
    },
    {
      "id": "s7", "name": "release_stock", "kind": "function", "level": 2, "parent": "s6", "mode": "full",
      "file": "app/inventory.py", "start_line": 32, "end_line": 34, "via_line": 30,
      "intro": "release_stock adds the units back to the count. A SKU that is not in the table starts from zero, so giving stock back never fails.",
      "blocks": []
    },
    {
      "id": "s8", "name": "create_order", "kind": "function", "level": 1, "parent": "s0", "mode": "full",
      "file": "app/orders.py", "start_line": 23, "end_line": 34, "via_line": 24,
      "intro": "create_order writes down the finished sale and returns it.",
      "blocks": [
        { "start_line": 23, "end_line": 32, "focus": "Copy the request, add the transaction id",
          "text": "The completed order is the request's fields plus the id the payment returned. Nothing is computed again here: the total is the one the customer sent." },
        { "start_line": 33, "end_line": 34, "focus": "Keep it in memory, by order id",
          "text": "`_ORDERS` is a dict in the module. Orders are lost when the app restarts, and a second order with the same id replaces the first." }
      ]
    },
    {
      "id": "s9", "name": "CompletedOrder", "kind": "class", "level": 2, "parent": "s8", "mode": "full",
      "file": "app/orders.py", "start_line": 12, "end_line": 19, "via_line": 25, "degraded": true,
      "intro": "CompletedOrder is a class in app/orders.py.",
      "blocks": [
        { "start_line": 12, "end_line": 15, "focus": "Lines 12 to 15", "text": "" },
        { "start_line": 16, "end_line": 19, "focus": "Lines 16 to 19", "text": "" }
      ]
    }
  ]
}
```

| Stop | Steps | Shows |
|---|---|---|
| 1 `checkout_order` | 3 | an intro and two blocks |
| 2 `check_stock` | 1 | a short stop (W9) |
| 3 `reserve_stock` | 3 | the README's picture is its second step |
| 4 `check_stock` | 1 | explained before: `⇢ 2`, **Go there** |
| 5 `charge_payment` | 1 | a short stop |
| 6 `retry_charge` | 3 | the stop the agent is on in most pictures |
| 7 `cancel_checkout` | 1 | |
| 8 `release_stock` | 1 | |
| 9 `create_order` | 3 | a block of ten lines: taller than the pane's lit part in the narrow layout |
| 10 `CompletedOrder` | 3 | **fallback**: the core's even split, no text |

The sizes the agent offers in frame 1 for `checkout_order`: only it, 1 stop, about 1 min · and what it calls, 6 stops, about 4 min · and what
those call, 10 stops, about 7 min · all the way down, 11 stops, about 8 min.

### 4.1 The other tours in the list

Five tours are in the cache. Only the first is the fixture above and can be read; the others need their stops' names (for the route and
for "on this node") and nothing else.

| # | Tour of | Stops, in order | Started | Question | State at load |
|---|---|---|---|---|---|
| 1 | `checkout_order`, two levels | the fixture | today 18:40 | — | `writing`, by the clock |
| 2 | `checkout_order`, one level | checkout_order · check_stock · reserve_stock · charge_payment · cancel_checkout · create_order | today 11:05 | "what happens when the payment times out" | `finished` |
| 3 | `app/`, and its files | app/ · api.py · checkout.py · gateway.py · inventory.py · models.py · orders.py · payments.py · \_\_init\_\_.py | yesterday | — | `stop 4/9` |
| 4 | `reserve_stock`, only it | reserve_stock | 12 Nov | — | `changed` |
| 5 | `checkout` (api.py), one level | checkout · checkout_order | 10 Nov | — | `new` |

MAIN's panel needs `inventory.py` open with its three functions, so that selecting `reserve_stock`, `release_stock` and `inventory.py`
changes the **On the selected node** view (07 §4 has the expected rows for two of them).

## 5. The fake agent

A clock that stands in for the agent writing the file.

| Time | Event |
|---|---|
| 0 s | the tour appears: `status: "generating"`, every stop not written, `writing: "s0"`. The cover is shown |
| every 4 s | the stop in `writing` becomes written (its `intro` and `blocks` appear); `writing` moves to the next |
| after stop 10 | `status: "complete"`, `writing: null` |
| `■` pressed | *stopping* at once; at the next 4 s tick the stop being written is finished, then *stopped*. Nothing more is written |

At 4 s a stop the agent stays ahead of a careful reader. The strip (§6) has a 12 s clock, so that you can catch up with the agent and see
frame 6 of the flow.

## 6. The demo strip

Under the window, outside the design, as in the redesign's mockup. It only flips states:

| Control | Does |
|---|---|
| **Theme** | Dark+ / Light+ |
| **Agent** | Start over · Write all now · Speed: 2 s / 4 s / 12 s per stop · Go silent (no news: the clock stops, the foot changes after 5 s instead of 5 min) |
| **Before the tour** | shows frame 1 (the terminal, static) and frame 2's hidden-canvas notification |
| **Code changed** | marks stop 3 as changed (05 §4): edit one of its lines in place so the difference is visible |
| **Width** | Wide · Narrow (the canvas at 560 px, the stage stacks) |
| **Reduce motion** | on / off |

## 7. The frames to reach

Each must be reachable by clicking, and look as its picture:

| Frame | In | How to reach it |
|---|---|---|
| the cover, the route all `◌` | 02 · 2 | load the page |
| an intro | 02 · 3 | Start |
| a block | 02 · 4 | Next |
| into a call: the ring moves, the heavy edge grows | 02 · 5 | Next twice |
| the README's picture | README §4 | Next to stop 3, step 2, with the clock at stop 6 |
| ahead of the agent | 02 · 6 | click stop 9 in the route in the first seconds |
| explained before, **Go there**, then `←` | 02 · 7 | stop 4 |
| stopping, stopped, an unwritten stop after it | 02 · 8 | `■` |
| fallback | 03 §3 | stop 10 |
| changed | 03 §3 | the strip |
| the end card, with "3 not seen" | 02 · 9 | skip with `↓`, then `End` |
| Main and back | 02 · 10 | the Main chip, then the tour's chip |
| the mini bar | 03 §6 | `⌄` |
| the list, All | 07 §2 | `‹ Tours` |
| the list, On `reserve_stock` and On `inventory.py` | 07 §4 | select them in MAIN, switch the view |
| a tour opened on a node's stop | 07 §5 | On `reserve_stock`, click tour 1: it opens on stop 3 |
| back where I left | 07 §5 | open tour 3, then tour 1 again |
| `▶ 3` on a card in MAIN | 07 §6 | the Main chip |
| the narrow stage | 03 §6 | the strip |

## 8. Not to build

- A tour of a folder (the *contents* step, 03 §3.1): a picture in the document is enough for now.
- Real syntax highlighting, real layout code, React. A fixed position per card is fine: the route never changes.
- Anything from the interactive tour. Leave the space under the step's text empty and the `+N` dim.
- Sound, a play button, a timer that moves to the next step (W8).

When the mockup and a document disagree after you have tried it by hand, say which felt better: the documents are the guess, the mockup is
the test.
