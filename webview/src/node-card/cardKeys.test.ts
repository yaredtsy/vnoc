import assert from "node:assert/strict";
import { test } from "node:test";

import { type KeyPress, type Platform, saveTip, staysInCode, swallows } from "./cardKeys.ts";
import type { EditState } from "./editState.ts";

// docs/17-edit-in-canvas.md §11 f: state × key. The key comes in as a plain object, so no DOM is needed.

const press = (key: string, mods: Partial<Pick<KeyPress, "meta" | "ctrl" | "alt" | "shift">> = {}, code = `Key${key.toUpperCase()}`): KeyPress => ({ key, code, meta: false, ctrl: false, alt: false, shift: false, ...mods });
const STATES: EditState[] = ["closed", "reading", "editing", "saving", "conflict", "gone"];

const swallowed: { name: string; platform: Platform; key: KeyPress }[] = [
  { name: "⌘S", platform: "mac", key: press("s", { meta: true }) },
  { name: "⌘⌥S (the browser says `ß`; the code is KeyS)", platform: "mac", key: press("ß", { meta: true, alt: true }, "KeyS") },
  { name: "⌘⇧S (shift makes the key `S`)", platform: "mac", key: press("S", { meta: true, shift: true }) },
  { name: "Ctrl+S", platform: "other", key: press("s", { ctrl: true }) },
  { name: "Ctrl+Shift+S", platform: "other", key: press("S", { ctrl: true, shift: true }) },
];
// 9.7: wherever the code is shown, with a draft or without; a card whose code is closed has no editor to type in
for (const s of swallowed) {
  for (const state of STATES) {
    test(`${s.platform}: ${s.name} is ${state !== "closed" ? "swallowed" : "not swallowed"} in ${state}`, () => assert.equal(swallows(state, s.key, s.platform), state !== "closed"));
  }
}

const passes: { name: string; platform: Platform; key: KeyPress }[] = [
  { name: "⌘P", platform: "mac", key: press("p", { meta: true }) },
  { name: "⌘W", platform: "mac", key: press("w", { meta: true }) },
  { name: "⌘B", platform: "mac", key: press("b", { meta: true }) },
  { name: "⌘C", platform: "mac", key: press("c", { meta: true }) },
  { name: "⌘⇧P", platform: "mac", key: press("P", { meta: true, shift: true }) },
  { name: "a plain s", platform: "mac", key: press("s") },
  { name: "⇧S (a capital S)", platform: "mac", key: press("S", { shift: true }) },
  { name: "⌥S alone", platform: "mac", key: press("ß", { alt: true }, "KeyS") },
  { name: "Ctrl+S on macOS (not Save there)", platform: "mac", key: press("s", { ctrl: true }) },
  { name: "⌘⌥⇧S", platform: "mac", key: press("ß", { meta: true, alt: true, shift: true }, "KeyS") },
  { name: "Ctrl+P", platform: "other", key: press("p", { ctrl: true }) },
  { name: "Ctrl+W", platform: "other", key: press("w", { ctrl: true }) },
  { name: "Ctrl+B", platform: "other", key: press("b", { ctrl: true }) },
  { name: "Ctrl+C", platform: "other", key: press("c", { ctrl: true }) },
  { name: "Ctrl+K (the first key of Save All's chord)", platform: "other", key: press("k", { ctrl: true }) },
  { name: "the S of Ctrl+K S: a plain s", platform: "other", key: press("s") },
  { name: "Meta+S on Windows or Linux (the Windows key)", platform: "other", key: press("s", { meta: true }) },
  { name: "Ctrl+Alt+S (AltGr)", platform: "other", key: press("s", { ctrl: true, alt: true }) },
];
for (const p of passes) {
  for (const state of STATES) test(`${p.platform}: ${p.name} passes in ${state}`, () => assert.equal(swallows(state, p.key, p.platform), false));
}

test("the Save tooltip names the key of the platform", () => {
  assert.equal(saveTip("mac"), "Save (⌘S)");
  assert.equal(saveTip("other"), "Save (Ctrl+S)");
});

// 9.2c: Space with a modifier that types a space is kept from React Flow's pan key, while editing
const space = (mods: Partial<Pick<KeyPress, "meta" | "ctrl" | "alt" | "shift">> = {}, key = " "): KeyPress => press(key, mods, "Space");
const staying: { name: string; key: KeyPress; stays: boolean }[] = [
  { name: "Shift+Space", key: space({ shift: true }), stays: true },
  { name: "Alt+Space (a non-breaking space on macOS: the key is not a plain space)", key: space({ alt: true }, "\u00a0"), stays: true },
  { name: "Shift+Alt+Space", key: space({ shift: true, alt: true }), stays: true },
  { name: "plain Space (`nokey` keeps it, no rule needed)", key: space(), stays: false },
  { name: "Ctrl+Space (Monaco's suggest)", key: space({ ctrl: true }), stays: false },
  { name: "⌘Space (the system's)", key: space({ meta: true }), stays: false },
  { name: "Ctrl+Shift+Space", key: space({ ctrl: true, shift: true }), stays: false },
  { name: "Shift+S", key: press("S", { shift: true }), stays: false },
  { name: "Shift+Enter", key: press("Enter", { shift: true }, "Enter"), stays: false },
];
for (const s of staying) {
  for (const state of STATES) {
    test(`${s.name} ${s.stays && state !== "closed" ? "stays in the code" : "is left alone"} in ${state}`, () => assert.equal(staysInCode(state, s.key), s.stays && state !== "closed"));
  }
}
