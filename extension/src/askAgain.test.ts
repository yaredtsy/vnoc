import assert from "node:assert/strict";
import { test } from "node:test";

import { ASK_AGAIN_MS, askUntilAnswered, asksAgain } from "./askAgain.ts";

const refusal = (words = "the index is being read and is not ready yet: ask again") => Object.assign(new Error(words), { code: -32803, data: { askAgain: true } });

/** An ask that fails `times` times with the refusal, then answers; the sleeps and the words said are kept. */
function setup(times: number, other?: Error) {
  const log = { asked: 0, slept: [] as number[], said: [] as (string | null)[] };
  const ask = async () => {
    log.asked++;
    if (other) throw other;
    if (log.asked <= times) throw refusal();
    return "answer";
  };
  const options = {
    sleep: async (ms: number) => void log.slept.push(ms),
    waiting: (words: string | null) => void log.said.push(words),
  };
  return { log, ask, options };
}

test("only data.askAgain makes a failure one to ask again, not its words", () => {
  assert.equal(asksAgain(refusal()), true);
  assert.equal(asksAgain(new Error("the index is being read: ask again")), false, "words alone are not the flag");
  assert.equal(asksAgain(Object.assign(new Error("x"), { data: { askAgain: false } })), false);
  assert.equal(asksAgain(undefined), false);
  assert.equal(asksAgain(null), false);
});

test("a refusal that says ask again is asked again every 2 s, with no cap, and the answer is returned", async () => {
  const { log, ask, options } = setup(25);
  assert.equal(await askUntilAnswered(ask, () => true, options), "answer");
  assert.equal(log.asked, 26);
  assert.deepEqual(new Set(log.slept), new Set([ASK_AGAIN_MS]));
  assert.equal(ASK_AGAIN_MS, 2000);
});

test("while it waits the core's own words are given, and null when the wait ends", async () => {
  const { log, ask, options } = setup(2);
  await askUntilAnswered(ask, () => true, options);
  assert.deepEqual(log.said, ["the index is being read and is not ready yet: ask again", "the index is being read and is not ready yet: ask again", null]);
});

test("any other failure is returned as it was, and not asked again", async () => {
  const other = Object.assign(new Error("no tour t-nope"), { code: -32602 });
  const { log, ask, options } = setup(0, other);
  await assert.rejects(askUntilAnswered(ask, () => true, options), (e) => e === other);
  assert.equal(log.asked, 1);
  assert.deepEqual(log.said, [], "nothing was waited for");
});

test("when what asked is not wanted any more, the refusal is thrown and nothing more is asked", async () => {
  const { log, ask, options } = setup(100);
  let wanted = true;
  const wait = options.sleep;
  options.sleep = async (ms) => {
    await wait(ms);
    if (log.asked === 3) wanted = false;
  };
  await assert.rejects(askUntilAnswered(ask, () => wanted, options), (e) => asksAgain(e));
  assert.equal(log.asked, 3);
  assert.equal(log.said.at(-1), null);

  const none = setup(100);
  await assert.rejects(askUntilAnswered(none.ask, () => false, none.options), (e) => asksAgain(e));
  assert.equal(none.log.asked, 1, "not wanted at the first refusal: not asked again");
});
