import assert from "node:assert/strict";
import { test } from "node:test";

import type { EditState } from "../node-card/editState.ts";
import { noticeFor } from "../state/draftRules.ts";
import { docButtons, docLine } from "./docEdit.ts";

// docs/17-edit-in-canvas.md §8: the doc panel's buttons by state, and the one line above its editor.

// each row: the state, whether the draft differs from its start, the buttons (id, disabled)
const buttons: { state: EditState; changed: boolean; expect: [string, boolean][] }[] = [
  { state: "closed", changed: false, expect: [] },
  { state: "reading", changed: false, expect: [["edit", false], ["openInEditor", false]] },
  { state: "editing", changed: false, expect: [["save", true], ["discard", false]] },
  { state: "editing", changed: true, expect: [["save", false], ["discard", false]] },
  { state: "saving", changed: true, expect: [["save", true], ["discard", true]] },
  { state: "conflict", changed: true, expect: [["save", true], ["discard", false]] },
  { state: "gone", changed: true, expect: [["save", true], ["discard", false]] },
];
for (const r of buttons) {
  test(`the panel's buttons: ${r.state}${r.state === "editing" ? (r.changed ? ", changed" : ", unchanged") : ""}`, () => assert.deepEqual(docButtons(r.state, r.changed).map((b) => [b.id, b.disabled]), r.expect));
}

test("the panel's buttons are the card's: Save and Discard in the draft's states, with the same disabled rules", () => {
  for (const state of ["editing", "saving", "conflict", "gone"] as const) {
    for (const changed of [false, true]) {
      const [save, discard] = docButtons(state, changed);
      assert.equal(save.disabled, state !== "editing" || !changed, `Save in ${state}`);
      assert.equal(discard.disabled, state === "saving", `Discard in ${state}`);
    }
  }
});

const file = "overview.md";
const lines: { name: string; input: Parameters<typeof docLine>[0]; expect: unknown }[] = [
  { name: "nothing to say", input: { refused: false, busy: false, asking: "docs", state: null, file }, expect: null },
  { name: "a draft that is editing says nothing", input: { refused: false, busy: false, asking: "docs", state: "editing", file }, expect: null },
  { name: "saving says nothing", input: { refused: false, busy: false, asking: "docs", state: "saving", file }, expect: null },
  { name: "the file has unsaved changes: Save <file> first", input: { refused: true, busy: false, asking: "docs", state: null, file }, expect: { kind: "refusal" } },
  { name: "the canvas has it, said on the docs tab", input: { refused: false, busy: true, asking: "docs", state: null, file }, expect: { kind: "busy", text: "Already being edited in the canvas" } },
  { name: "the docs tab has it, said on the canvas", input: { refused: false, busy: true, asking: "canvas", state: null, file }, expect: { kind: "busy", text: "Already being edited in the docs tab" } },
  { name: "a conflict: the card's line and buttons", input: { refused: false, busy: false, asking: "docs", state: "conflict", file }, expect: { kind: "state", text: "Changed in the editor since you started", actions: ["reload", "showDiff"] } },
  { name: "gone: the file's name, Open in Editor and Copy Draft", input: { refused: false, busy: false, asking: "docs", state: "gone", file }, expect: { kind: "state", text: "Not found in overview.md any more", actions: ["openInEditor", "copyDraft"] } },
  { name: "written, not saved", input: { refused: false, busy: false, asking: "docs", state: null, file, notice: noticeFor("written", file) }, expect: { kind: "notice", notice: { kind: "written", text: "Written into overview.md, not saved." } } },
  { name: "a failed save", input: { refused: false, busy: false, asking: "docs", state: "editing", file, notice: noticeFor("error", file, "the host is gone") }, expect: { kind: "notice", notice: { kind: "error", text: "Could not save: the host is gone" } } },
  { name: "the refusal wins over the other page's draft, which wins over a conflict, which wins over a notice", input: { refused: true, busy: true, asking: "docs", state: "conflict", file, notice: noticeFor("written", file) }, expect: { kind: "refusal" } },
  { name: "the other page's draft wins over a conflict and a notice", input: { refused: false, busy: true, asking: "docs", state: "conflict", file, notice: noticeFor("written", file) }, expect: { kind: "busy", text: "Already being edited in the canvas" } },
  { name: "a conflict wins over a notice", input: { refused: false, busy: false, asking: "docs", state: "conflict", file, notice: noticeFor("written", file) }, expect: { kind: "state", text: "Changed in the editor since you started", actions: ["reload", "showDiff"] } },
];
for (const r of lines) test(`the line above the editor: ${r.name}`, () => assert.deepEqual(docLine(r.input), r.expect));
