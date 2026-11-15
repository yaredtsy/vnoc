import assert from "node:assert/strict";
import { test } from "node:test";

import { type RowState, nameClasses, rowClasses } from "./rowLook.ts";
import { rowDescription } from "./rowFacts.ts";

const state = (over: Partial<RowState> = {}): RowState => ({ selected: false, highlighted: false, treeFocused: false, ...over });
const has = (classes: string, ...wanted: string[]) => wanted.forEach((w) => assert.ok(classes.split(" ").includes(w), `${w} in ${classes}`));
const hasNot = (classes: string, ...unwanted: string[]) => unwanted.forEach((w) => assert.ok(!classes.split(" ").includes(w), `${w} not in ${classes}`));

// the table of 09 §3: a state → the channels that are not colour alone
const table: { name: string; state: RowState; outline: boolean; dotted: boolean; contrast: string | null }[] = [
  { name: "selected, the list has the keyboard: the focus outline (the High Contrast border)", state: state({ selected: true, treeFocused: true }), outline: true, dotted: false, contrast: "--row-outline-focused" },
  { name: "selected, the list has not: the High Contrast border still marks it", state: state({ selected: true }), outline: true, dotted: false, contrast: "--row-outline-selected" },
  { name: "highlighted from elsewhere: a dotted outline, as a card is pointed at", state: state({ highlighted: true }), outline: true, dotted: true, contrast: null },
  { name: "selected and highlighted: the selection wins, and it is not dotted", state: state({ selected: true, highlighted: true, treeFocused: true }), outline: true, dotted: false, contrast: "--row-outline-focused" },
  { name: "neither: only the hover colour (and the buttons, which appear)", state: state(), outline: false, dotted: false, contrast: null },
];
for (const t of table) {
  test(`row look: ${t.name}`, () => {
    const classes = rowClasses(t.state);
    assert.equal(classes.split(" ").includes("outline"), t.outline);
    assert.equal(classes.split(" ").includes("outline-dotted"), t.dotted);
    assert.equal(t.contrast === null ? !classes.includes("--row-outline") : classes.includes(`outline-[color:var(${t.contrast})]`), true, classes);
  });
}

test("every state that is drawn has an outline: no state is a colour and nothing else", () => {
  for (const selected of [false, true]) {
    for (const highlighted of [false, true]) {
      for (const treeFocused of [false, true]) {
        const classes = rowClasses({ selected, highlighted, treeFocused });
        if (selected || highlighted) has(classes, "outline");
      }
    }
  }
});

test("selected rows differ by whether the list has the keyboard, in their colours", () => {
  has(rowClasses(state({ selected: true, treeFocused: true })), "bg-[var(--card-selected-bg)]");
  has(rowClasses(state({ selected: true })), "bg-[var(--list-inactive-bg)]");
  hasNot(rowClasses(state({ selected: true })), "bg-[var(--card-selected-bg)]");
});

test("a name with a syntax error is in the error colour, and the row says `syntax error` in words too", () => {
  assert.match(nameClasses(true), /text-destructive/);
  assert.doesNotMatch(nameClasses(false), /text-destructive/);
  assert.equal(rowDescription({ syntax_error: true, docs: 0 }), "syntax error");
});
