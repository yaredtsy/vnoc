import assert from "node:assert/strict";
import { test } from "node:test";

import type { OffersView, TourPinAnswer, TourView } from "@vnoc/shared/tours.ts";
import { coverPin, defaultMessage, dialogFiles, dryKey, offersOf, wantsDryPin } from "./pin.ts";

const view = (over: Partial<TourView> = {}): TourView => ({ id: "t-1", state: "finished", written: 2, pinned: false, offers: null, stops: [], ...over }) as unknown as TourView;
const uncommitted = (...paths: string[]): TourPinAnswer["reasons"] => [{ code: "uncommitted", words: `${paths.length} files have changes that are not committed`, files: paths.map((path) => ({ status: " M", path })) }];
const answer = (over: Partial<TourPinAnswer> = {}): TourPinAnswer => ({ tour: "t-1", state: "not_pinned", reasons: uncommitted("a.py", "b.py"), ...over });

test("the cover asks why only for a finished tour that is not pinned", () => {
  assert.equal(wantsDryPin({ state: "finished", pinned: false }), true);
  assert.equal(wantsDryPin({ state: "finished", pinned: true }), false);
  assert.equal(wantsDryPin({ state: "writing", pinned: false }), false);
  assert.equal(wantsDryPin({ state: "stopped", pinned: false }), false);
});

test("the cover's lines from a dry answer: not pinned, then the first reason in the core's words", () => {
  const said = coverPin(view(), answer());
  assert.deepEqual(said.pin, { text: "not pinned" });
  assert.equal(said.reason, "2 files have changes that are not committed", "the core's sentence, not made here");
  assert.equal(coverPin(view(), undefined).reason, null, "the answer has not come");
  assert.equal(coverPin(view(), null).reason, null, "the core could not say");
  assert.equal(coverPin(view(), answer({ state: "pinned", reasons: undefined })).reason, null, "it would be pinned: no reason");
  assert.equal(coverPin(view(), answer({ reasons: [{ code: "stale", words: "s2 changed since it was written: replace what changed, then pin", changed: ["s2"], gone: [] }, ...uncommitted("a.py")!] })).reason, "s2 changed since it was written: replace what changed, then pin", "the first");
});

test("a pinned tour says where, and gives no reason; a tour being written says nothing of the pin", () => {
  const pinned = coverPin(view({ pinned: true, pin: "f4d6e15aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", pin_in_repository: true }), answer());
  assert.equal(pinned.pin?.text, "pinned at f4d6e15");
  assert.equal(pinned.reason, null);
  const writing = coverPin(view({ state: "writing" }), answer());
  assert.deepEqual([writing.pin, writing.reason, writing.offers], [null, null, []]);
});

test("the offers per setting: `ask` offers both, with their buttons; `always` and `never` offer nothing (the core sends none)", () => {
  const ask: OffersView = { ids: 4, commit: ["a.py", "b.py"] };
  assert.deepEqual(offersOf(ask), [
    { kind: "ids", text: "4 stops have no ID: the tour will not follow a rename.", button: "Write IDs" },
    { kind: "commit", text: "2 files have changes that are not committed.", button: "Commit and Pin…" },
  ]);
  assert.deepEqual(offersOf({ ids: 1, commit: ["a.py"] }).map((o) => o.text), ["1 stop has no ID: the tour will not follow a rename.", "1 file has changes that are not committed."]);
  assert.deepEqual(offersOf(null), [], "ids and commit `never`, or `always`: nothing to offer");
  assert.deepEqual(offersOf({ ids: 0, commit: ["a.py"] }).map((o) => o.button), ["Commit and Pin…"], "`ids` is not `ask`: only the commit");
  assert.deepEqual(offersOf({ ids: 3, commit: [] }).map((o) => o.button), ["Write IDs"], "`commit` is not `ask`: only the IDs");
  assert.deepEqual(coverPin(view({ offers: ask }), answer()).offers.map((o) => o.button), ["Write IDs", "Commit and Pin…"]);
  assert.deepEqual(coverPin(view({ offers: null }), answer()).offers, [], "`never`: `not pinned` is still said");
  assert.equal(coverPin(view({ offers: null }), answer()).pin?.text, "not pinned");
});

test("the dialog's files are the uncommitted reason's, then the tour's node.toml files, each once", () => {
  assert.deepEqual(dialogFiles(answer()), ["a.py", "b.py"]);
  assert.deepEqual(dialogFiles(answer({ node_files: ["a.py", ".vnoc/n.toml"] })), ["a.py", "b.py", ".vnoc/n.toml"]);
  assert.deepEqual(dialogFiles(answer({ reasons: [{ code: "not_checked", why: "no driver", words: "the tour could not be checked against the code: no driver" }], node_files: ["x.toml"] })), [], "another reason: nothing to commit");
  assert.deepEqual(dialogFiles({ reasons: undefined }), []);
});

test("the message the dialog proposes is the core's default", () => {
  assert.equal(defaultMessage("checkout_order"), 'Code for the tour "checkout_order"');
});

test("the dry pin is asked again only when what it reads changed", () => {
  const stops = [{ id: "s1", key: "a.py::f", planned: false, check: "fresh" }] as unknown as TourView["stops"];
  const a = dryKey(view({ stops }));
  assert.equal(dryKey(view({ stops, place: { at: "s1" } } as Partial<TourView>)), a, "a move of the reader is not a change");
  assert.notEqual(dryKey(view({ stops, offers: { ids: 0, commit: ["a.py"] } })), a, "the offers changed");
  assert.notEqual(dryKey(view({ stops: [{ ...stops[0], check: "changed" }] as unknown as TourView["stops"] })), a, "a check said changed");
  assert.notEqual(dryKey(view({ stops, pinned: true, pin: "abc" })), a, "pinned");
});
