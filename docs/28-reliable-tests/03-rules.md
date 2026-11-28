# 28.03 — How to write a UI test that gives the same answer every time

The rules for every new test, and for every test a step of this plan touches. Each rule names the failure it prevents ([01-review.md](01-review.md)). The helpers named here are built in T1 and T2 ([04-design.md](04-design.md)).

## Before writing: pick the layer

Ask in this order. Stop at the first yes.

1. **Is it a rule?** (an order, a label, a count, a layout sum) → a **unit test** of a pure function, `node --test`. No page.
2. **Is it what the page shows or does, given what the host sent?** (a fold, a row, a key, a width, a button) → a **Page test**: the real page in Chromium, the host played by a script.
3. **Does it need VS Code, the host, the core, a file or git to really talk?** → a **Window test**. One or two per feature: the path works end to end. Not every case of it.

A check in the window that could be a page test is a fault in review.

## The seven rules

### 1. Wait for the whole thing you will check
Put everything you are about to assert into the wait. Never wait for one field and assert others.

```ts
// wrong: waits for the id, then checks a drawing that may be half made
check(await stageShows("s1.b3", atStep("s1.b3")), row);

// right: one step. It retries until the drawing equals all of it, or fails with the difference
await stage.shows("s1.b3 after Next", { step: "s1.b3", kind: "detail", lit: [11, 12, 13, 14], thin: [9, 10] }, { after: click });
```

In a Page test this is Playwright's own `expect(locator)…`, which retries. Prevents: the half-made drawing (5 of the 9 CI failures).

### 2. Tie the wait to your action
Every action gives back a ticket. A wait takes `{ after: ticket }` and looks only at drawings the page made after it handled that action. A drawing from before can never match, even when it looks the same.

```ts
const click = await api.stageClick("next");
await stage.shows("step 2", { step: "s2" }, { after: click });
```

Prevents: matching the old state (Next from step 1 to step 2 when both show the same function).

### 3. After touching the world, wait for the core by name
A file written, a `vnoc` or `git` command, a restart: follow it with `await api.settled({ paths })`. It makes the core read those paths now and answers when the core, the host and every page have caught up. Do not poll the structure. Do not wait for the watcher.

Prevents: asking before the core has read (Write IDs), and six copies of one poll loop.

### 4. To prove "nothing happens", wait for done, then check
`await api.settled()` after the action, then assert that the state is unchanged. Never `sleep(n)` then assert.

Prevents: the 28 sleeps that pass when the product is slow and wrong.

### 5. The test owns the clock
No test waits real time for a product timer. A timer in the page is moved by the page's test clock (`page.clock`). A timer in the host or the core has a variable that shortens it for the test window, as `VNOC_HOLD_FIRST_BUILD_MS` does today. Nothing depends on two things happening in different seconds.

Prevents: the 6.5 s undo wait, the 20 × 1.1 s id waits, the 200 ms gap in `calls`.

### 6. One test, one claim, on its own state
- The title is one sentence with no `;`.
- The test makes what it needs, or its suite's setup does. It reads no variable another suite set.
- It passes alone: `VNOC_GREP="its title"` is green.
- No `if` around an assert. If the thing may be absent, that is another test.

Prevents: chains that fail for an earlier test's reason, and asserts that do not run.

### 7. Fix the size and the motion
A Page test sets its viewport in pixels and runs with motion off. A Window test never asserts a size in pixels: sizes are the Page layer's.

Prevents: the 520 px test on CI's small window.

## What is banned

| Banned | Use |
|---|---|
| `sleep(` in a suite (a check in CI counts them: the number may only go down, then it is zero) | rules 3, 4, 5 |
| `last()` / `.at(-1)` followed by an assert | rule 1 |
| a loop that sends an action again until it works | the page says when it listens (`pageReady`, design §2); send once |
| `Date.now()` compared to a limit as a pass mark | a measurement job, not a test |
| `if (process.env.CI) this.skip()` | fix it or delete it |
| a new `viewCommand` that only the suite uses | a Page test clicks the real button |

## What a failure must say

One failed run must be enough to know why. A failed wait prints: what it waited for, the difference between the last drawing and the expected one, the ticket it waited after, the core's place, the page's errors, the last 20 lines of the core's log. On CI the same goes into a file that the job uploads.

## Review checklist (for each new or changed test)

- [ ] It is in the lowest layer that can prove it.
- [ ] Every wait is `shows(…, { after })`, `settled()`, or an awaited command.
- [ ] No `sleep(`, no `last()` before an assert, no resend loop.
- [ ] Its title is one claim. It passes alone with `VNOC_GREP`.
- [ ] It passes 5 times in a row under stress (`VNOC_STRESS=1`), locally.
- [ ] I broke the product on purpose once, and the test went red with a message that says what broke.
