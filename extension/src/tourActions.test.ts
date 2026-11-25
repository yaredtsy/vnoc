import assert from "node:assert/strict";
import { test } from "node:test";

import type { TourPinAnswer, TourSettingsAnswer, TourView } from "@vnoc/shared/tours.ts";
import { type ActionDeps, BUTTONS, TourActions, idsToSave, pinRefusal, saveFirstMessage, settingItems, valueItems } from "./tourActions.ts";

const view = { title: "Checkout", stops: [{ id: "s1", path: "shop.py", planned: false }, { id: "s2", path: "util.py", planned: false, now: { path: "lib/util.py" } }, { id: "s3", path: "later.py", planned: true }] } as unknown as TourView;

test("the save-first message counts the dirty files of the tour's stops: not planned stops, not other files; the file a stop moved to counts", () => {
  assert.deepEqual(idsToSave(["other.py", "shop.py", "later.py", "lib/util.py"], view), ["shop.py", "lib/util.py"]);
  assert.deepEqual(idsToSave(["other.py"], view), [], "none of them is dirty: nothing to ask");
  assert.equal(saveFirstMessage(2), "Save 2 files first? The IDs are written into them.");
  assert.equal(saveFirstMessage(1), "Save 1 file first? The IDs are written into it.");
});

const answer = (over: Partial<TourPinAnswer>): TourPinAnswer => ({ tour: "t-1", state: "not_pinned", ...over });
const uncommitted = { code: "uncommitted" as const, words: "2 files have changes that are not committed", files: [] };
const stale = { code: "stale" as const, words: "s2 changed since it was written: replace what changed, then pin", changed: ["s2"], gone: [] };

test("a refusal is one message in the core's words; only files that are not committed offer Commit and Pin…", () => {
  assert.deepEqual(pinRefusal(answer({ reasons: [uncommitted] })), { text: "Not pinned: 2 files have changes that are not committed.", offerCommit: true });
  assert.deepEqual(pinRefusal(answer({ state: "ask", reasons: [uncommitted] })).offerCommit, true, "`ask`");
  assert.deepEqual(pinRefusal(answer({ reasons: [stale] })), { text: `Not pinned: ${stale.words}.`, offerCommit: false });
  assert.equal(pinRefusal(answer({ reasons: [stale, uncommitted] })).text, `Not pinned: ${stale.words} · ${uncommitted.words}.`);
  assert.equal(pinRefusal(answer({ reasons: [stale, uncommitted] })).offerCommit, false, "something else is in the way too");
});

test("the settings picks: the two settings with their value now, then the three values with the current one marked", () => {
  const shown = { ids: { value: "ask", from: "default" }, commit: { value: "never", from: "file" } } as TourSettingsAnswer;
  assert.deepEqual(settingItems(shown).map((i) => [i.key, i.description]), [["ids", "now: ask (the default)"], ["commit", "now: never"]]);
  assert.deepEqual(valueItems("ask").map((i) => [i.label, i.description, i.picked]), [["always", "", false], ["ask", "current", true], ["never", "", false]]);
});

/** the window, as the actions see it: the messages said (with the button each is answered with), the files saved, the requests made */
function window(over: Partial<ActionDeps> & { answers?: (string | undefined)[]; picks?: string[] } = {}) {
  const said: { level: string; text: string; buttons: string[] }[] = [];
  const asked: [string, unknown][] = [];
  const saved: string[][] = [];
  const dialogs: [string, string | null][] = [];
  const answers = [...(over.answers ?? [])];
  const picks = [...(over.picks ?? [])];
  const deps: ActionDeps = {
    request: (async (method: string, params: unknown) => {
      asked.push([method, params]);
      if (method === "vnoc/tourIds") return { changed: ["shop.py", "util.py"], words: "IDs written for 3 stops in 2 files." };
      if (method === "vnoc/tourSettings") return { ids: { value: "ask", from: "file" }, commit: { value: "ask", from: "file" } };
      return { tour: "t-1", state: "pinned", commit: "f4d6e15aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa" };
    }) as ActionDeps["request"],
    view: async () => view,
    dirtyFiles: () => ["shop.py"],
    save: async (paths) => void saved.push(paths),
    say: async (level, text, buttons) => (said.push({ level, text, buttons }), answers.shift()),
    pick: (async (items: { label: string }[]) => {
      const want = picks.shift();
      return items.find((i) => i.label === want);
    }) as ActionDeps["pick"],
    openDialog: (id, title) => void dialogs.push([id, title]),
    showChanges: () => void said.push({ level: "info", text: "(source control)", buttons: [] }),
    ...over,
  };
  return { actions: new TourActions(deps), said, asked, saved, dialogs };
}

test("Write IDs with a dirty stop file asks once, saves, writes, then says where; Cancel writes nothing", async () => {
  const w = window({ answers: [BUTTONS.saveAndWrite, BUTTONS.showChanges] });
  assert.equal(await w.actions.writeIds("t-1"), "written");
  assert.deepEqual(w.said.map((m) => [m.text, m.buttons]), [
    ["Save 1 file first? The IDs are written into it.", [BUTTONS.saveAndWrite, BUTTONS.cancel]],
    ["IDs written for 3 stops in 2 files.", [BUTTONS.showChanges, BUTTONS.always]],
    ["(source control)", []],
  ]);
  assert.deepEqual(w.saved, [["shop.py"]]);
  assert.deepEqual(w.asked.map((a) => a[0]), ["vnoc/tourIds"], "saved before the core wrote");

  const cancelled = window({ answers: [BUTTONS.cancel] });
  assert.equal(await cancelled.actions.writeIds("t-1"), "cancelled");
  assert.deepEqual([cancelled.saved, cancelled.asked], [[], []], "nothing saved, nothing written");

  const clean = window({ dirtyFiles: () => [], answers: [BUTTONS.always] });
  await clean.actions.writeIds("t-1");
  assert.deepEqual(clean.said.map((m) => m.text), ["IDs written for 3 stops in 2 files."], "no dirty file: no question");
  assert.deepEqual(clean.asked, [["vnoc/tourIds", { id: "t-1" }], ["vnoc/tourSettings", { ids: "always" }]], "Always Write IDs in This Project");
});

test("the core's refusal of Write IDs is one message in its words", async () => {
  const w = window({ dirtyFiles: () => [], request: (async () => { throw new Error('this project writes no ID lines ([ids] default = "none" in .vnoc/vnoc.toml)'); }) as ActionDeps["request"] });
  assert.equal(await w.actions.writeIds("t-1"), "refused");
  assert.deepEqual(w.said, [{ level: "error", text: 'this project writes no ID lines ([ids] default = "none" in .vnoc/vnoc.toml)', buttons: [] }]);
});

test("Pin says where it pinned; a refusal offers Commit and Pin… when files are the one thing in the way, and picking it opens the dialog", async () => {
  const ok = window();
  assert.equal(await ok.actions.pin("t-1"), "pinned");
  assert.equal(ok.said[0].text, "Pinned at f4d6e15.");
  const files = window({ request: (async () => answer({ state: "ask", reasons: [uncommitted] })) as ActionDeps["request"], answers: [BUTTONS.commitAndPin] });
  assert.equal(await files.actions.pin("t-1"), "not_pinned");
  assert.deepEqual(files.said[0], { level: "warn", text: "Not pinned: 2 files have changes that are not committed.", buttons: [BUTTONS.commitAndPin] });
  assert.deepEqual(files.dialogs, [["t-1", "Checkout"]]);
  const other = window({ request: (async () => answer({ reasons: [stale] })) as ActionDeps["request"] });
  await other.actions.pin("t-1");
  assert.deepEqual([other.said[0].buttons, other.dialogs], [[], []]);
});

test("Tour Settings… picks a setting, then a value, and writes that one through tourSettings; closing a pick writes nothing", async () => {
  const w = window({ picks: ["Commit before pinning", "never"] });
  assert.equal(await w.actions.settings(), "commit");
  assert.deepEqual(w.asked, [["vnoc/tourSettings", undefined], ["vnoc/tourSettings", { commit: "never" }]]);
  const none = window({ picks: ["IDs for the stops"] });
  assert.equal(await none.actions.settings(), null);
  assert.equal(none.asked.length, 1, "only read");
});

test("a message of Write IDs is the core's words as they came, and a tour with nothing written offers no button", async () => {
  const lost = "1 stop's ID is lost from the code: see Needs Attention.";
  const w = window({ dirtyFiles: () => [], request: (async () => ({ changed: [], words: lost })) as ActionDeps["request"] });
  await w.actions.writeIds("t-1");
  assert.deepEqual(w.said, [{ level: "info", text: lost, buttons: [] }]);
});
