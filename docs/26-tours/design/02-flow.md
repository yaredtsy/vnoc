# 02 The flow: ten frames

```
flow
├── 1. you ask                    the agent's one question, its one-line answer
├── 2. the tour appears           the cover; the canvas hidden: a notification
├── 3. Start                      step 1: what the first stop is for
├── 4. Next                       a block: lines lit, a title, a paragraph
├── 5. Next, into a call          the ring moves on the map, the stage changes file
├── 6. you catch up with the agent   a stop that is not written yet
├── 7. a stop explained before    "explained at stop 2"
├── 8. ■ Stop                     stopping, stopped, what is left
├── 9. the end                    the end card
└── 10. leave and come back       the Main chip, the status bar, Open Tour…
```

The example is the fixture of [`06-mockup-brief.md`](06-mockup-brief.md): Minicart's `checkout_order`, two levels of calls, 10 stops, 20 steps.
Each frame shows only the part that changes. The whole window is in the README §4.

## 1. You ask

In the agent's terminal. The tour is not on screen yet.

When you name how far, or the short route is small (12 stops or fewer, the phase document's depth rule), the agent asks nothing:

```
> walk me through checkout_order

● Tour started: checkout_order and what it calls · 6 stops · about 4 min.
  It is on the V-NOC canvas. Say "deeper" for 10 stops.
```

When the short route is long, it asks **one** question, as routes with a size (W13):

```
> walk me through app/

● How far should the tour of app/ go?

  1. Only app/                        1 stop   · about 1 min
  2. And its files                    9 stops  · about 3 min
  3. And their functions             22 stops  · about 15 min
  4. And what those call             41 stops  · too long for one tour (40 at most)

> 2
```

| Rule | Why |
|---|---|
| a route is named by **what it adds**, never "depth 2" | you choose by what you will see |
| every route shows **stops and minutes** | minutes = steps × 20 s, rounded up. A guess; it only has to be the same guess every time |
| a route over the limit is listed, greyed, with the reason | so you know it exists and why it cannot be picked |
| after `start` the agent says **one line** (W12): what, how big, where | the tour is the answer |
| while writing, the agent says nothing in the chat | progress is on the route and the bar. A terminal that scrolls "wrote stop 4 …" is noise |
| at the end, one line: `Tour finished: 10 stops.` or `Tour stopped at stop 6 of 10.` | |

## 2. The tour appears

The agent ran `vnoc walk start`. The file exists, every stop is not written. **Canvas visible**: a tour context is added and activated (W11).

```
┌ side bar ──────────────────────┐┌ V-NOC canvas ────────────────────────────────────────────────────────────────────────────────┐
│ ┃ ‹ Tours ▶ checkout_order   ✕ ││ [←] [Main] [▶ checkout_order ✕] │ checkout_order                       writing 1/10 ■  ⌖ ⓘ ⋯ │
│ ┃ ◌  1  ƒ checkout_order       ││                                                                                              │
│ ┃ ◌  2    ƒ check_stock        ││   [◌1 checkout_order]┄┄[ 2 check_stock]                                                      │
│ ┃ ◌  3    ƒ reserve_stock      ││                      ┄┄[ 3 reserve_stock]┄┄┄[ 4 check_stock ⇢2]      the whole route,        │
│ ┃ ◌  4      ƒ check_stock  ⇢ 2 ││                      ┄┄[ 5 charge_payment]┄┄[ 6 retry_charge]         fitted to the map      │
│ ┃ ◌  5    ƒ charge_payment     ││                      ┄┄[ 7 cancel_checkout]┄[ 8 release_stock]                               │
│ ┃ ◌  6      ƒ retry_charge     ││                      ┄┄[ 9 create_order]┄┄┄┄[10 CompletedOrder]                              │
│ ┃ ◌  7    ƒ cancel_checkout    │├ STAGE ───────────────────────────────────────────────────────────────────────────────────────┤
│ ┃ ◌  8      ƒ release_stock    ││ ▶ Tour of checkout_order                                                                 ⌄   │
│ ┃ ◌  9    ƒ create_order       ││                                                                                              │
│ ┃ ◌ 10      ◆ CompletedOrder   ││   10 stops · about 7 min · checkout_order, what it calls, and what those call                │
│ ┃ writing 1 of 10       ■ Stop ││   app/checkout.py · written for the code as it is at f4d6e15                                 │
│                                ││                                                                                              │
│                                ││   ◌ the agent is writing stop 1 of 10 …                                        [ Start › ]   │
└────────────────────────────────┘└──────────────────────────────────────────────────────────────────────────────────────────────┘
```

- This is the **cover** (W10). The map is fitted to the whole route, once. It is the only time the tour changes the zoom.
- **Start is enabled at once.** You do not have to wait for stop 1 (frame 6 shows what an unwritten stop looks like).
- When you asked with a question (`--query "how is the total computed"`), the cover shows it in quotes above the sizes. It is the tour's subject.

**Canvas hidden** (you are in a code tab, the canvas is closed or behind another tab): nothing opens.

```
                                           ┌──────────────────────────────────────────────────────────┐
                                           │ ⓘ Tour of checkout_order: 10 stops, about 7 min.         │
                                           │                                   [Open Tour]  [Not Now] │
                                           └──────────────────────────────────────────────────────────┘
 status bar   …   ▶ Tour: checkout_order · writing 1/10   …
```

**Open Tour**, the status bar item, and a click on any row of the route all do the same: show the canvas (beside the editor, as Open in Canvas
does) with the tour's context active.

## 3. Start

`Start ›`, `Space`, or `→`. Stop 1 is written by now.

```
├ STAGE ───────────────────────────────────────────────────────────────────────────────────────┤
│ ● 1  ƒ checkout_order   checkout.py:11                             stop 1 of 10      ↗   ⌄   │
│ ───────────────────────────────────────────────────────┬──────────────────────────────────── │
│  11 ▏  def checkout_order(order: OrderRequest) -> …    │ checkout_order turns an order       │
│  12        """ID: 859e6adb-…"""                        │ request into a completed order: it  │
│  13        if not check_stock(order.sku, order.quan…   │ holds the stock, takes the money,   │
│  14            raise ValueError(f"Out of stock: {or…   │ and records the sale, in that order.│
│  15                                                    │                                     │
│  16        reserve_stock(order.sku, order.quantity)    │                                     │
│  17                                                    │                                     │
│  18        try:                                        │ ▰▱▱▱▱▱▱▱▱▱         ‹ Prev   Next ›  │
└──────────────────────────────────────────────────────────────────────────────────────────────┘
```

| Part | Change |
|---|---|
| route | stop 1 is `●`; under it, its three steps: *what it is for* (current), *Check the stock, then hold it*, *Charge, and give the stock back…* |
| map | the ring is on card 1. The map does not move: card 1 is in view |
| stage | the **intro** step: no title, the paragraph in the lead size; the code from the stop's first line, nothing lit, a thin mark `▏` on the `def` line |
| bar | the crumbs: `checkout_order` |

## 4. Next: a block

```
├ STAGE ───────────────────────────────────────────────────────────────────────────────────────┤
│ ● 1  ƒ checkout_order   checkout.py:11                    stop 1 of 10 · 2 of 3      ↗   ⌄   │
│ ───────────────────────────────────────────────────────┬──────────────────────────────────── │
│  11 ▌  def checkout_order(order: OrderRequest) -> …    │ Check the stock, then hold it       │
│  12 ▌      """ID: 859e6adb-…"""                        │                                     │
│  13 ▌      if not check_stock(order.sku, order.quan…   │ Nothing is charged until the items  │
│  14 ▌          raise ValueError(f"Out of stock: {or…   │ are set aside. check_stock only     │
│  15 ▌                                                  │ looks; reserve_stock takes the units│
│  16 ▌      reserve_stock(order.sku, order.quantity)    │ out of the count, so a second buyer │
│  17                                                    │ cannot get them while this payment  │
│  18        try:                                        │ runs.                               │
│  19            transaction_id = charge_payment(order)  │ ▰▱▱▱▱▱▱▱▱▱         ‹ Prev   Next ›  │
└──────────────────────────────────────────────────────────────────────────────────────────────┘
```

| Part | Change |
|---|---|
| stage | lines 11 to 16 are lit (`▌` and a background); the others are dim. The title is the block's `focus`. **The code did not scroll**: the block was already in view |
| route | the `▸` moves to *Check the stock, then hold it* |
| map | nothing. A step inside a stop never moves the map |

## 5. Next, into a call

From the last step of stop 1 to stop 2. This is the only kind of move where all three parts change.

```
   map, before                                        map, after
   ┏━━━━━━━━━━━━━━━━━━━━┓      ┌──────────────────┐   ┌────────────────────┐      ┏━━━━━━━━━━━━━━━━━━┓
   ┃ ●1 ƒ checkout_order┃┄┄┄┄┄┄│  2 ƒ check_stock │   │ ✓1 ƒ checkout_order│━━━━━━┃ ●2 ƒ check_stock ┃
   ┗━━━━━━━━━━━━━━━━━━━━┛      └──────────────────┘   └────────────────────┘      ┗━━━━━━━━━━━━━━━━━━┛

├ STAGE ───────────────────────────────────────────────────────────────────────────────────────┤
│ ● 2  ƒ check_stock   inventory.py:12   ↳ called on line 13         stop 2 of 10      ↗   ⌄   │
│ ───────────────────────────────────────────────────────┬──────────────────────────────────── │
│  12 ▌  def check_stock(sku: str, quantity: int) -> …   │ check_stock answers one question:   │
│  13 ▌      """ID: b47e48c0-…"""                        │ are there at least `quantity` units │
│  14 ▌      available = _STOCK.get(sku, 0)              │ of this SKU? An unknown SKU counts  │
│  15 ▌      return available >= quantity                │ as zero. It reads and never changes │
│                                                        │ the stock.                          │
│                                                        │ ▰▰▱▱▱▱▱▱▱▱         ‹ Prev   Next ›  │
└──────────────────────────────────────────────────────────────────────────────────────────────┘
```

| Part | Change | How it moves |
|---|---|---|
| map | card 1 gets `✓`, the ring goes to card 2, the edge between them turns heavy | the ring fades over 150 ms. The map pans **only if** card 2 is not fully in view, and then by the least that shows it. Zoom is never touched |
| stage | another file. The header says how you got here: `↳ called on line 13` (a link to that line in stop 1) | the code and the text cross-fade, 150 ms. The dock keeps its height |
| route | stop 1 folds to one row with `✓`; stop 2 is `●`. It is a **short stop** (W9): one step, the whole node lit, no step rows under it | |
| bar | crumbs: `checkout_order › check_stock` | |

With *Reduce motion*, nothing fades and the map jumps instead of panning.

## 6. You catch up with the agent

You pressed Next on the last written step. The next stop is on the route but its text is not written.

```
├ STAGE ───────────────────────────────────────────────────────────────────────────────────────┤
│ ◌ 6  ƒ retry_charge   payments.py:7   ↳ called on line 23          stop 6 of 10      ↗   ⌄   │
│ ───────────────────────────────────────────────────────┬──────────────────────────────────── │
│   7    def retry_charge(order, max_attempts=2):        │ ░░░░░░░░░░░░░░░░░░░░░░░░░░░░░       │
│   8        """ID: 7a4e1aac-…"""                        │ ░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░  │
│   9        for attempt in range(max_attempts):         │ ░░░░░░░░░░░░░░░░░░░                 │
│  10            try:                                    │                                     │
│  11                return send_charge(                 │ The agent is writing this stop.     │
│  12                    order.total,                    │ You can read the code meanwhile.    │
│  13                    order.order_id                  │                                     │
│  14                )                                   │ ▰▰▰▰▰▰▱▱▱▱         ‹ Prev   Next ◌  │
└──────────────────────────────────────────────────────────────────────────────────────────────┘
```

- **You move on at once**, and wait on the stop itself with its code in front of you. When the text is saved it fades in where the grey bars
  were. Nothing jumps, because you are already there.
- This replaces the prototype's `pendingAdvance` (remember the click, move when the step arrives): no hidden "armed" state to explain or cancel.
- `Next` is dim with a `◌` until the stop is written. `Prev`, the route and the map work.
- You can also go past it: click stop 9 on the route. Every unwritten stop looks like this one.

## 7. A stop explained before

Stop 4: `reserve_stock` calls `check_stock`, which was stop 2. The core made it *contextual*; the agent wrote one sentence.

```
├ STAGE ───────────────────────────────────────────────────────────────────────────────────────┤
│ ● 4  ƒ check_stock   inventory.py:12   ↳ called on line 21         stop 4 of 10      ↗   ⌄   │
│ ───────────────────────────────────────────────────────┬──────────────────────────────────── │
│  12    def check_stock(sku: str, quantity: int) -> …   │ Called again here: reserve_stock    │
│  13        """ID: b47e48c0-…"""                        │ does not trust its caller to have   │
│  14        available = _STOCK.get(sku, 0)              │ checked.                            │
│  15        return available >= quantity                │                                     │
│                                                        │ ⇢ Explained at stop 2   [Go there]  │
│                                                        │ ▰▰▰▰▱▱▱▱▱▱         ‹ Prev   Next ›  │
└──────────────────────────────────────────────────────────────────────────────────────────────┘
```

- The code is shown dim and whole, nothing lit: you have read it.
- **Go there** goes to stop 2. The bar's `←` then comes back here: it is "where I was before I jumped" ([03](03-stage.md) §4).
- On the map and the route the stop carries `⇢ 2`. It has no children: what `check_stock` calls is under stop 2.

## 8. ■ Stop

`■` is on the bar and at the foot of the route, only while the agent writes.

```
 bar, before      …  checkout_order › reserve_stock                    writing 6/10 ■   ⌖ ⓘ ⋯
 bar, at once     …  checkout_order › reserve_stock        stopping: the agent finishes stop 6   ⌖ ⓘ ⋯
 bar, after       …  checkout_order › reserve_stock                    stopped at 6 of 10   ⌖ ⓘ ⋯

 route foot       │ ┃ stopped: 6 of 10 written       │
```

- A click cannot interrupt the agent (the phase document's rule 5). So the truth has two beats: **stopping** (the file says stopped, the agent
  is still inside one stop) and **stopped** (the agent's next command was answered with "stopped"). The UI shows both; it never claims
  "stopped" early. What it needs for the second beat: [05](05-states.md) §7.
- No dialog asks "are you sure". What was written stays; nothing is lost by stopping.
- Stops 7 to 10 stay on the route and the map as `◌`. Going to one shows its code and, where the text would be: *"Not written: the tour was
  stopped here. Ask the agent to continue it."*
- You stay on the step you were on.

## 9. The end

`Next` on the last step.

```
├ STAGE ───────────────────────────────────────────────────────────────────────────────────────┤
│ ✓ End of the tour of checkout_order                                                      ⌄   │
│                                                                                              │
│   10 stops · 20 steps · all seen                                                             │
│                                                                                              │
│   [ ‹ Back to the last step ]        [ Start Again ]        [ Close Tour ]                   │
└──────────────────────────────────────────────────────────────────────────────────────────────┘
```

- The map is fitted to the whole route again, every card `✓`.
- If you skipped stops: `10 stops · 3 not seen`, and the first button is **Go to the first one I skipped**.
- If the tour was stopped: *"End of what was written: 6 of 10 stops."*
- **Close Tour** closes the tour's context. The canvas draws MAIN, where you left it. The tour stays in **Open Tour…**.
- The end card is where phase 3 will offer "Save as doc" and the routes one level deeper. In basic it has three buttons.

## 10. Leave and come back

| You | Result |
|---|---|
| click the **Main** chip, or a row in MAIN's panel | the canvas draws MAIN. The tour's panel stays in the sidebar, its chip on the bar, the status bar says `▶ Tour 3/10` |
| click the tour's chip, its panel, or the status bar item | the tour again, on the same step, the map where it was |
| `✕` on the chip or the panel · `Esc` on the canvas | the tour's context closes. If the agent is still writing, it keeps writing; the status bar says `▶ Tour: checkout_order · writing 7/10` and a click opens it again |
| close and reopen VS Code | the tour's context is back, on the same step (the session is saved, the file is in the cache) |
| `‹ Tours` on the route's header | the sidebar shows **the list of tours**; the tour stays open ([07](07-tour-list.md)) |
| **V-NOC: Open Tour…** (palette) | the same list as a QuickPick, for the keyboard |

```
┌ Open Tour ───────────────────────────────────────────────────────┐
│ checkout_order      10 stops · today 18:40 · you are on stop 3   │
│ app/                 9 stops · yesterday · finished              │
│ reserve_stock        3 stops · 12 Nov · the code changed since   │
└──────────────────────────────────────────────────────────────────┘
```

Next: [`03-stage.md`](03-stage.md)
