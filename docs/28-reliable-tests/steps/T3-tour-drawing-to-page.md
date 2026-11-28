# T3 — The tour's drawing checks move to the Page layer

## Goal
What the tour's pages draw is checked where the size is exact and nothing else runs. The two tests skipped on CI run again, there.

## What moves (from `tour.test.ts`, by suite)

| Suite | Tests | Goes to Page | Stays in Window |
|---|---|---|---|
| on the open tour | 11 | Next/Prev, the keys, 520 px, the chip row, the code pane, the selection | `vnoc tour go` moves the stage; `tour where` agrees; Go to Code opens the file |
| the TOURS section | 8 | fold, rows, route, keys, the end card | one: a click on a row opens the tour |
| the tour on the map | 5 | cards, edges, ring, jump | Main ⇄ tour keeps the step (no pixel check) |
| the tour's bar | 6 | chip, crumbs, ←, Start Again | `moved by the agent` after a real `tour go`; Esc gives MAIN back |
| a step's text is Markdown | 2 | both | none |
| a tour read while it is written | 10 | what each state draws (from recorded events) | one: an `add` reaches an open page |
| TR24s: the screen | 4 | all (G6, G8, G9, G10) | none |

About 40 tests move. The split per test is settled in the prompt, with this table as the start.

## How
- One fixture per scenario, recorded once (`VNOC_RECORD`): the finished tour, the tour being written, the long file.
- Each moved test is one claim (rule 6): "a canvas 520 px wide: …; …; …" becomes four tests.
- Real clicks and keys. The 6 s and "few seconds" timers use `page.clock`.
- A moved test is deleted from `tour.test.ts` in the same commit.
- Then delete the product's test-only paths nothing uses any more (`canvasWidth`, parts of `stageClick`, `stage/measure.ts` if the page tests measure for themselves).

## Done when
- The page suite is green 10 times in a row, under 2 minutes.
- `tour.test.ts` has no `if (process.env.CI) this.skip()`, no pixel assert, and is about 40 tests shorter.
- A layout fault made on purpose (the mini bar 31 px tall) turns one page test red, with a message that says the height.
