# 08 The interactive tour: ask, go deeper, come back, find it again

```
interactive
├── 1. the idea: one thing, a side trip
├── 2. five rules
├── 3. ask
├── 4. the answer
├── 5. go deeper
├── 6. finding it again
├── 7. when a side trip is too big: a new tour
└── 8. for the mockup: the fixture, what to fake, the frames
```

A first design, to be judged in the mockup. Only what it looks like and how it feels. How the click reaches the agent is not here: the
mockup fakes it with a clock, as it does for writing.

## 1. The idea

While you read a step you can do two things, and they are **one thing**:

| You | You get |
|---|---|
| **ask** a question about what is on the stage | a **side trip** of one step: the answer |
| **go deeper** into a call the tour left out | a **side trip** of a few stops: that function, explained |

A side trip hangs from the step where you started it. It has a way in, steps like any other step, and one way back. The tour's own route
never changes.

```
   main route      1 ── 2 ── 3 ── 4 ── 5 ── 6 ── 7 ── 8 ── 9 ── 10
                             │              │
   side trips                ? what if two  ↳ send_charge
                               orders…        (3 steps)
```

## 2. Five rules

| # | Rule | So that |
|---|---|---|
| **1** | **The main route never changes.** Stop 4 is stop 4 before and after any side trip | you cannot get lost: "stop 3 of 10" stays true |
| **2** | **Every side trip has one way back**, always in the same place: the stage's `‹ Back to stop 3` | no "where was I" |
| **3** | **You never wait.** Ask, and keep reading; a mark says when the answer is there | a tour does not turn into a chat you stare at |
| **4** | **An answer is a step**: a title, one short paragraph, lit lines. Not a chat bubble, no thread | no wall of text; an answer points at code like everything else |
| **5** | **What you asked stays with the tour**, at the place you asked it | coming back to the stop, or opening the tour next week, you find it |

## 3. Ask

The stage's text pane gets one line, in the space basic left empty. It is always there on a written step.

```
├ STAGE ───────────────────────────────────────────────────────────────────────────────────────┤
│ ● 3  ƒ reserve_stock   inventory.py:19   ↳ called on line 16    stop 3 of 10 · 3 of 3  ↗  ⌄  │
│ ───────────────────────────────────────────────────────┬──────────────────────────────────── │
│  21        if not check_stock(sku, quantity):          │ Take the units, warn when few are   │
│  22            raise ValueError(f"Not enough stock…")  │ left                                │
│  23                                                    │ The count drops at once; there is   │
│  24 ▌      _STOCK[sku] -= quantity                     │ no separate "reserved" number. With │
│  25 ▌      remaining = _STOCK[sku]                     │ LOW_STOCK_THRESHOLD (2) or fewer    │
│  26 ▌                                                  │ left it logs a warning and carries  │
│  27 ▌      if remaining <= LOW_STOCK_THRESHOLD:        │ on: low stock is news, not an error.│
│  28 ▌          logger_core.logger.warning(f"Only {r…   │                                     │
│                                                        │ ┌ Ask about these lines…         ↵ ┐│
│                                                        │ ▰▰▰▱▱▱▱▱▱▱         ‹ Prev   Next ›  │
└──────────────────────────────────────────────────────────────────────────────────────────────┘
```

| Thing | Rule |
|---|---|
| the line | one line; it grows to three while you type. `Enter` sends, `Esc` leaves it. `/` puts the cursor in it from anywhere on the canvas |
| what the question is about | the step you are on, with its lit lines. Select other lines in the code pane first and the line reads *"Ask about lines 21 to 24…"* |
| after `Enter` | the line empties and under it one dim line appears: `? What if two orders reserve the last…  · answering`. **You stay on the step.** `Next ›` works |
| nothing else | no model picker, no attachments, no history above the line. It is a question about this code |

## 4. The answer

When it is ready, the dim line under the ask line turns into a link, and the same row appears in the route under the stop (§6):

```
│                                                        │ ┌ Ask about these lines…         ↵ ┐│
│                                                        │ ? What if two orders reserve th… ›  │   ready: click, or Enter
```

Opening it is going on the side trip. The stage is the same stage, in the side trip's colour (a second accent, not the focus blue):

```
├ STAGE ───────────────────────────────────────────────────────────────────────────────────────┤
│ ? side trip from stop 3 · reserve_stock   inventory.py:19                             ↗   ⌄  │
│ ───────────────────────────────────────────────────────┬──────────────────────────────────── │
│  19    def reserve_stock(sku: str, quantity: int) -> … │ “What if two orders reserve the     │
│  20        """ID: 8a9cc864-…"""                        │  last unit at the same time?”       │
│  21 ▌      if not check_stock(sku, quantity):          │                                     │
│  22 ▌          raise ValueError(f"Not enough stock…")  │ Nothing stops two buyers            │
│  23 ▌                                                  │ The check on line 21 and the        │
│  24 ▌      _STOCK[sku] -= quantity                     │ subtraction on line 24 are two      │
│  25        remaining = _STOCK[sku]                     │ separate steps on a plain dict, with│
│  26                                                    │ no lock. Two requests can both pass │
│  27        if remaining <= LOW_STOCK_THRESHOLD:        │ the check, and the count goes below │
│                                                        │ zero.                               │
│                                                        │ ┌ Ask a follow-up…               ↵ ┐│
│                                                        │ ‹ Back to stop 3          ＋ Keep   │
└──────────────────────────────────────────────────────────────────────────────────────────────┘
```

| Part | Rule |
|---|---|
| header | `? side trip from stop 3`, no "stop 3 of 10": you are beside the route, not on it |
| your question | on top, in quotes, dim: the answer is read against it |
| the answer | a title and one paragraph, the same budget as a step. The agent chooses the lines to light; they can differ from the step's |
| the foot | **`‹ Back to stop 3`** where `Next ›` is on the route (rule 2). No segments: a side trip is not progress |
| a follow-up | the ask line. Its answer is the next step of the same side trip; then the foot gets `‹` `›` to move inside it |
| **`＋ Keep`** | saves the answer as a doc on `reserve_stock`. The tour is in a cache; a doc stays |
| keys | `Esc` or `Alt+←`: back to stop 3. `←` `→`: inside the side trip when it has more than one step |

## 5. Go deeper

A call the tour left out is offered where you see it:

| Where | Looks like |
|---|---|
| in the stage's code, on the lit lines | the call's name has a dotted underline; hover: *"Not on this tour. Click to go deeper."* |
| over the ask line | a chip, only when the lit lines hold such a call: `↳ Go deeper: send_charge` |
| on the map | the card's `+1` (basic drew it dim) is now a button |

Stop 6, `retry_charge`, the block that calls `send_charge`:

```
│  11 ▌              return send_charge(                 │ works returns the transaction id,   │
│  12 ▌                  order.total,                    │ and that ends the loop.             │
│  13 ▌                  order.order_id                  │                                     │
│  14 ▌              )                                   │ [ ↳ Go deeper: send_charge ]        │
│  15            except TimeoutError:                    │ ┌ Ask about these lines…         ↵ ┐│
│                                                        │ ▰▰▰▰▰▰▱▱▱▱         ‹ Prev   Next ›  │
```

One click. On the map a card grows out of stop 6, at once, not written yet; when it is written you are told the same way as for an answer.

```
   ┌──────────────────────┐        ┏━━━━━━━━━━━━━━━━━━━━━━┓        ╭┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄╮
   │  5  ƒ charge_payment │━━━━━━━━┃ ●6  ƒ retry_charge   ┃┄ ┄ ┄ ┄ ┆ ↳   ƒ send_charge    ┆
   └──────────────────────┘        ┗━━━━━━━━━━━━━━━━━━━━━━┛        ╰┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄╯
                                                                     a side trip's card: round corners,
                                                                     the side trip's colour, no number
```

| Rule | Why |
|---|---|
| a side trip's card has **no stop number**, round corners and the second accent | rule 1: it is beside the route, and looks it |
| inside it the stage is the stage of §4, with stops: header `↳ side trip from stop 6 · send_charge · 2 of 3`, foot `‹ Back to stop 6` `‹` `›` | one look for every side trip |
| after its last step, `›` reads **`Back to stop 6 ›`** | the end of a side trip is the way back |
| you can ask and go deeper inside a side trip | it nests; the header then reads `from stop 6 › send_charge`, and Back goes up one level |
| a call that is already a stop is never offered | basic already links it: a click jumps to that stop |

## 6. Finding it again

What you asked is at the place you asked it, on all three parts.

**The route.** Side trips are rows under their stop, between its steps, shown while the stop is unfolded. A folded stop says how many it has.

```
│ ┃ ✓  1  ƒ checkout_order                      ? 1 │   folded: one question here
│ ┃ ✓  2    ƒ check_stock                           │
│ ┃ ●  3    ƒ reserve_stock                         │
│ ┃           what it is for                        │
│ ┃           Refuse what is not there              │
│ ┃         ▸ Take the units, warn when few are le… │
│ ┃             ? What if two orders reserve the l… │   a question, under the step it was asked on
│ ┃ ○  4      ƒ check_stock                     ⇢ 2 │
│ ┃ ○  5    ƒ charge_payment                        │
│ ┃ ○  6      ƒ retry_charge                    ↳ 1 │   folded: one deeper trip here
```

**The code.** On a step that has questions, the gutter has a `?` on the lines each was about. Hover shows the question; a click opens the
answer.

```
│  20 ?          except TimeoutError:                    │
│  21 ?              cancel_checkout(order)              │
│  22 ?              raise                               │
```

**The map.** A deeper trip is its cards. A question is a small `?` with a count on the stop's card footer.

**The tour list.** The row says it: `10 stops · 2 questions · today 18:40`.

| Rule | Why |
|---|---|
| a side trip that is being written has `·` `answering` on its row and a slow pulse on its mark | rule 3 |
| the newest answer you have not opened is bold, on the route and on the stage | so "it is ready" is seen wherever you are looking |
| `Next ›` never enters a side trip | the route is the route; side trips are opened on purpose |
| a row's menu: **Open** · **Keep as Doc** · **Delete** | |

## 7. When a side trip is too big

Going deeper adds one function and what it needs, a handful of stops. When the piece is big (more than six stops), the chip says so and
offers a tour of its own:

```
│ [ ↳ Go deeper: process_refund · 14 stops — as a new tour ]
```

| Rule | Why |
|---|---|
| the new tour is a tour like any other, in the list, **nested under the one it came from**, with `from stop 6 of checkout_order` on its second line | "branch another tour", and you can see where it branched |
| its cover has **`‹ Back to the tour of checkout_order`** | rule 2, between tours |
| the stop it came from shows `▶ 1` beside `? 1` and `↳ 1` | rule 5 |

Minicart is too small to have one. The mockup shows this only as a static row in the tour list.

## 8. For the mockup

Add to `walkthrough/mockup.html`. The basic tour must behave exactly as before for someone who never asks anything.

**Fake agent.** An answer arrives 5 s after `Enter`; a deeper trip's stops 4 s after the click. Any typed question gets the canned answer
of the step it was asked on; steps without one answer *"(the mockup has no answer for this step)"*.

**Fixture.** Real code, from the same Minicart files, plus `app/gateway.py`.

| # | Kind | Hangs from | State at load | Content |
|---|---|---|---|---|
| Q1 | question | stop 1, step 3 (`checkout_order`, lines 18–24) | **answered, opened before** | “What if the charge fails with another error?” · lit 20–22 · **Only a timeout gives the stock back** · The `except` names `TimeoutError` and nothing else. Any other error from `charge_payment` leaves this function with the units still reserved, and nothing in the app gives them back. |
| Q2 | question | stop 3, step 3 (`reserve_stock`, lines 24–28) | **not asked**: the canned answer for that step | “What if two orders reserve the last unit at the same time?” (pre-filled as the ask line's placeholder suggestion) · lit 21–24 · **Nothing stops two buyers** · The check on line 21 and the subtraction on line 24 are two separate steps on a plain dict, with no lock. Two requests can both pass the check, and the count goes below zero. |
| D1 | deeper | stop 6, step 2 (`retry_charge`, lines 7–14), the call on line 11 | **not taken** | `send_charge`, `app/gateway.py` lines 10–19, three steps, below |

`send_charge`:

| Step | Lines | Title | Text |
|---|---|---|---|
| intro | 10–19 | — | send_charge is the payment gateway, faked for the demo: it counts the attempts per order, so the result is always the same. |
| block | 10–13 | Count this attempt for the order | `_ATTEMPTS` remembers how many times each order id was sent. It is never cleared, so the count lasts as long as the app runs. |
| block | 15–19 | The first attempt always times out | Attempt 1 logs an error and raises `TimeoutError`: that is what makes `retry_charge` try again. Attempt 2 returns a transaction id made of the order id and the attempt. |

**Frames to reach.**

| Frame | How |
|---|---|
| the ask line on every written step | any step |
| ask, keep reading, the answer arrives while you are elsewhere | stop 3 step 3: type, `Enter`, `Next ›`; after 5 s the bold row under stop 3 and the mark on the stage |
| the answer as a step, **Back to stop 3** | click the row |
| a question found again: the folded `? 1`, the row, the `?` gutter | stop 1 at load (Q1) |
| go deeper from the chip, the card growing on the map, three steps, **Back to stop 6 ›** | stop 6 step 2 |
| go deeper from the map's `+1` | the `retry_charge` card |
| asking inside a side trip | in `send_charge` |
| the tour list row with `2 questions`, and one static nested tour | `‹ Tours` |
| the basic tour untouched | `Next ›` from the cover to the end: it never enters a side trip |

**Not to build.** Keep as Doc (a message *"Saved as a doc on reserve_stock"* is enough), Delete, the too-big case beyond the static row,
selecting lines to scope a question.

Back: [`README.md`](README.md)
