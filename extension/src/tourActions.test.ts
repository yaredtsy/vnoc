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
function window(over: Partial<ActionDeps> & { answers?: (string | undefined)[]; picks?: string[]; typed?: (string | undefined)[]; files?: (string | undefined)[]; replies?: Record<string, unknown> } = {}) {
  const said: { level: string; text: string; buttons: string[] }[] = [];
  const asked: [string, unknown][] = [];
  const saved: string[][] = [];
  const dialogs: [string, string | null][] = [];
  const events: unknown[] = [];
  const typed = [...(over.typed ?? [])];
  const answers = [...(over.answers ?? [])];
  const picks = [...(over.picks ?? [])];
  const files = [...(over.files ?? [])];
  const deps: ActionDeps = {
    request: (async (method: string, params: unknown) => {
      asked.push([method, params]);
      if (over.replies && method in over.replies) return over.replies[method];
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
    input: async () => typed.shift(),
    closeIfOpen: (id) => void events.push(["close", id]),
    deleted: (ids, tokens, words) => void events.push(["deleted", ids, tokens, words]),
    refreshList: () => void events.push(["list"]),
    saveJson: async (suggested, text) => (events.push(["save", suggested, text]), files.shift()),
    openJson: async (id, text) => void events.push(["open", id, text]),
    pickJson: async () => {
      const path = files.shift();
      return path === undefined ? undefined : { target: `file://${path}`, text: `{"title":"from ${path}"}` };
    },
    showRefusals: async (target, refusals) => void events.push(["problems", target, refusals.map((r) => [r.place, r.line])]),
    clearRefusals: (target) => void events.push(["clear", target]),
    checkAll: async () => void events.push(["checkAll"]),
    copy: async (text) => void events.push(["copy", text]),
    ...over,
  };
  return { actions: new TourActions(deps), said, asked, saved, dialogs, events };
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

/** a window whose core answers each request with `answers[method]` */
const core = (answers: Record<string, unknown | Error>): Partial<ActionDeps> => ({
  request: (async (method: string) => {
    const answer = answers[method];
    if (answer instanceof Error) throw answer;
    return answer;
  }) as ActionDeps["request"],
});

test("Rename… sends the title typed; an empty, unchanged or dismissed box sends nothing; the core's sentence is the message, its refusal too", async () => {
  const w = window({ typed: ["  A better title  "], ...core({ "vnoc/tourEdit": { tour: "t-1", title: "A better title", question: null, words: "Renamed to “A better title”." } }) });
  assert.equal(await w.actions.rename("t-1"), "renamed");
  assert.deepEqual(w.said, [{ level: "info", text: "Renamed to “A better title”.", buttons: [] }]);
  const asked: unknown[] = [];
  const spy = (typed: (string | undefined)[]) => window({ typed, request: (async (...a: unknown[]) => void asked.push(a)) as ActionDeps["request"] });
  for (const typed of [[""], ["   "], ["Checkout"], [undefined]]) assert.equal(await spy(typed).actions.rename("t-1"), "unchanged", String(typed[0]));
  assert.deepEqual(asked, [], "nothing was sent");
  const refused = window({ typed: ["two\nlines"], ...core({ "vnoc/tourEdit": new Error("a title is one line") }) });
  assert.equal(await refused.actions.rename("t-1"), "refused");
  assert.deepEqual(refused.said, [{ level: "error", text: "a title is one line", buttons: [] }]);
});

test("Share says the core's words; a tour that is not pinned has Pin on the message, and Make Private Again has none", async () => {
  const sharedWords = "Moved to `.vnoc/tours/`. Commit it to share it. It is not pinned: teammates will see changed stops without the old code.";
  const w = window({ answers: [BUTTONS.pin], ...core({ "vnoc/tourShare": { tour: "t-1", folder: "project", path: "x", pinned: false, words: sharedWords }, "vnoc/tourPin": { tour: "t-1", state: "pinned", commit: "f4d6e15aaa" } }) });
  assert.equal(await w.actions.share("t-1", true), "moved");
  assert.deepEqual(w.said.map((m) => [m.text, m.buttons]), [[sharedWords, [BUTTONS.pin]], ["Pinned at f4d6e15.", []]]);
  const pinned = window({ ...core({ "vnoc/tourShare": { tour: "t-1", folder: "project", path: "x", pinned: true, words: "Moved to `.vnoc/tours/`. Commit it to share it." } }) });
  await pinned.actions.share("t-1", true);
  assert.deepEqual(pinned.said.map((m) => m.buttons), [[]], "pinned: no Pin");
  const back = window({ ...core({ "vnoc/tourShare": { tour: "t-1", folder: "private", path: "x", pinned: false, words: "Moved to `.vnoc/local/tours/`. It is private again." } }) });
  await back.actions.share("t-1", false);
  assert.deepEqual(back.said.map((m) => [m.text, m.buttons]), [["Moved to `.vnoc/local/tours/`. It is private again.", []]]);
  const refused = window({ ...core({ "vnoc/tourShare": new Error("finish it before moving it, or stop it") }) });
  assert.equal(await refused.actions.share("t-1", true), "refused");
  assert.equal(refused.said[0].text, "finish it before moving it, or stop it");
});

test("Delete tells the pages the core's words and the token, closes the tour if it is the open one, asks no question; a refusal is a message and tells nothing", async () => {
  const w = window({ ...core({ "vnoc/tourDelete": { token: "tok-1", words: "Deleted “How the shop checks out”." } }) });
  assert.equal(await w.actions.delete("t-1"), "deleted");
  assert.deepEqual(w.events, [["close", "t-1"], ["deleted", ["t-1"], ["tok-1"], "Deleted “How the shop checks out”."], ["list"]]);
  assert.deepEqual(w.said, [], "no dialog, no message: the row says it");
  const refused = window({ ...core({ "vnoc/tourDelete": new Error("the tour t-1 is being written: stop it first") }) });
  assert.equal(await refused.actions.delete("t-1"), "refused");
  assert.deepEqual(refused.events, []);
  assert.equal(refused.said[0].text, "the tour t-1 is being written: stop it first");
});

test("Delete Finished Tours is one event for all of them; nothing to delete is the core's words", async () => {
  const w = window({ ...core({ "vnoc/tourDeleteFinished": { deleted: [{ id: "a", token: "ta" }, { id: "b", token: "tb" }], words: "Deleted 2 finished tours." } }) });
  assert.equal(await w.actions.deleteFinished(), 2);
  assert.deepEqual(w.events, [["close", "a"], ["close", "b"], ["deleted", ["a", "b"], ["ta", "tb"], "Deleted 2 finished tours."], ["list"]]);
  const none = window({ ...core({ "vnoc/tourDeleteFinished": { deleted: [], words: "There are no finished tours of yours to delete." } }) });
  assert.equal(await none.actions.deleteFinished(), 0);
  assert.deepEqual([none.events, none.said.map((m) => m.text)], [[], ["There are no finished tours of yours to delete."]]);
});

test("Undo restores every token, in order, and asks the list again; a token the trash lost is the core's refusal", async () => {
  const asked: string[] = [];
  const w = window({ request: (async (method: string, params: { token: string }) => void asked.push(`${method} ${params.token}`)) as ActionDeps["request"] });
  assert.equal(await w.actions.undo(["ta", "tb"]), true);
  assert.deepEqual(asked, ["vnoc/tourRestore ta", "vnoc/tourRestore tb"]);
  assert.deepEqual(w.events, [["list"]]);
  const lost = window({ ...core({ "vnoc/tourRestore": new Error("`ta` is not in the trash") }) });
  assert.equal(await lost.actions.undo(["ta"]), false);
  assert.equal(lost.said[0].text, "`ta` is not in the trash");
});

// ── TR22c: JSON, check all, ask ──────────────────────────────────────────────────────────────────────────────────────────

const refusal = { refused: [{ place: "stops[0].text", words: "stops[0].text: 412 of 280 characters", line: 7 }, { place: "title", words: "title: is empty", line: null }], words: "Not written: 2 problems. Fix them, then import the whole document again." };

test("Export as JSON… saves the core's text where the dialog says, and a dismissed dialog saves nothing and says nothing", async () => {
  const w = window({ replies: { "vnoc/tourExport": { text: "{\n  \"schema\": 1\n}\n" } }, files: ["/tmp/shop.json", undefined] });
  assert.equal(await w.actions.exportJson("t-1"), "saved");
  assert.deepEqual(w.asked, [["vnoc/tourExport", { id: "t-1" }]]);
  assert.deepEqual(w.events, [["save", "t-1.json", "{\n  \"schema\": 1\n}\n"]]);
  assert.deepEqual(w.said.map((m) => m.text), ["Saved the tour to /tmp/shop.json."]);
  assert.equal(await w.actions.exportJson("t-1"), "cancelled");
  assert.equal(w.said.length, 1);
});

test("Open as JSON gives the core's text to a tab for this tour", async () => {
  const w = window({ replies: { "vnoc/tourExport": { text: "{}" } } });
  assert.equal(await w.actions.openJson("t-1"), "opened");
  assert.deepEqual(w.events, [["open", "t-1", "{}"]]);
});

test("Copy Tour ID puts the id on the clipboard and says so in a few words", async () => {
  const w = window({});
  await w.actions.copyId("how-it-works-ab12");
  assert.deepEqual(w.events, [["copy", "how-it-works-ab12"]]);
  assert.deepEqual(w.said.map((m) => m.text), ["Copied how-it-works-ab12"]);
});

test("Import into This Tour is import --replace with the text; success is the core's words and the list again, refusals are diagnostics and the core's words", async () => {
  const written = window({ replies: { "vnoc/tourImport": { id: "t-1", words: "Replaced “Retry”: 3 stops (3 kept, 0 new, 0 gone)." } } });
  assert.equal(await written.actions.importInto("t-1", "{...}", "untitled:1"), "imported");
  assert.deepEqual(written.asked, [["vnoc/tourImport", { text: "{...}", replace: "t-1" }]]);
  assert.deepEqual(written.events, [["clear", "untitled:1"], ["list"]]);
  assert.deepEqual(written.said, [{ level: "info", text: "Replaced “Retry”: 3 stops (3 kept, 0 new, 0 gone).", buttons: [] }]);
  const refused = window({ replies: { "vnoc/tourImport": refusal } });
  assert.equal(await refused.actions.importInto("t-1", "{...}", "untitled:1"), "refused");
  assert.deepEqual(refused.events, [["problems", "untitled:1", [["stops[0].text", 7], ["title", null]]]], "no list again: nothing was written");
  assert.deepEqual(refused.said, [{ level: "warn", text: refusal.words, buttons: [] }]);
});

test("Import a Tour from JSON… makes a new tour from a file; a refused file has its refusals on the file; a dismissed dialog does nothing", async () => {
  const w = window({ replies: { "vnoc/tourImport": { id: "t-9", words: "Imported “Retry”: 3 stops." } }, files: ["/tmp/a.json", undefined] });
  assert.equal(await w.actions.importFile(), "imported");
  assert.deepEqual(w.asked, [["vnoc/tourImport", { text: '{"title":"from /tmp/a.json"}' }]], "no `replace`: a new tour");
  assert.equal(await w.actions.importFile(), "cancelled");
  assert.equal(w.asked.length, 1);
  const bad = window({ replies: { "vnoc/tourImport": refusal }, files: ["/tmp/b.json"] });
  assert.equal(await bad.actions.importFile(), "refused");
  assert.deepEqual(bad.events[0], ["problems", "file:///tmp/b.json", [["stops[0].text", 7], ["title", null]]]);
});

test("Check All Tours asks the host to check every tour; Ask for a Tour… copies the core's sentence and says the core's words", async () => {
  const w = window({ replies: { "vnoc/tourAsk": { text: "Walk me through a.py::f", words: "Copied. Paste it to your agent." } } });
  assert.equal(await w.actions.checkAll(), "checked");
  assert.deepEqual(w.events, [["checkAll"]]);
  assert.equal(await w.actions.ask({ key: "a.py::f" }), "asked");
  assert.deepEqual(w.asked, [["vnoc/tourAsk", { key: "a.py::f" }]]);
  assert.deepEqual(w.events.slice(1), [["copy", "Walk me through a.py::f"]]);
  assert.deepEqual(w.said, [{ level: "info", text: "Copied. Paste it to your agent.", buttons: [] }]);
  // with no node nothing is sent but the question; a row asks by its tour
  await w.actions.ask();
  await w.actions.ask({ tour: "t-1" });
  assert.deepEqual(w.asked.slice(1), [["vnoc/tourAsk", undefined], ["vnoc/tourAsk", { tour: "t-1" }]]);
  // the form's parts go to the core as they are: it makes the sentence
  await w.actions.ask({ key: "a.py::f", question: "why?", depth: 2, detail: "quick" });
  assert.deepEqual(w.asked.at(-1), ["vnoc/tourAsk", { key: "a.py::f", question: "why?", depth: 2, detail: "quick" }]);
  // parts that are all undefined are nothing
  await w.actions.ask({ key: undefined });
  assert.deepEqual(w.asked.at(-1), ["vnoc/tourAsk", undefined]);
});

test("the status bar item's click offers Show Tour and Stop Playing, and Stop Writing only while the agent writes", async () => {
  const offered: string[][] = [];
  const w = window({ pick: (async (items: { label: string }[]) => void offered.push(items.map((i) => i.label))) as ActionDeps["pick"] });
  await w.actions.statusPick(false);
  await w.actions.statusPick(true);
  assert.deepEqual(offered, [["Show Tour", "Stop Playing"], ["Show Tour", "Stop Writing", "Stop Playing"]]);
});
