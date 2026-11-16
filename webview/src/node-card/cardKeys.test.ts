import assert from "node:assert/strict";
import { test } from "node:test";

import { type KeyPress, type Platform, saveTip, swallows } from "./cardKeys.ts";
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
for (const s of swallowed) {
  for (const state of STATES) {
    test(`${s.platform}: ${s.name} is ${state === "editing" ? "swallowed" : "not swallowed"} in ${state}`, () => assert.equal(swallows(state, s.key, s.platform), state === "editing"));
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
