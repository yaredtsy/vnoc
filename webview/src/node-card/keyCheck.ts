// A check for the extension's test suite (9.2c): do the keys typed into an editing card's code stay in the code? The suite cannot press keys in a webview, but a
// `keydown` dispatched on the editor's own element runs every handler the page has, and a handler that takes the key calls `preventDefault()` (React Flow does for
// Space: it pans). So the page dispatches each key at the editor and says which ones were taken. Monaco itself does not prevent these.

export const CHECKED_KEYS: { name: string; key: string; code: string; meta?: boolean; shift?: boolean; alt?: boolean }[] = [
  { name: "Space", key: " ", code: "Space" },
  { name: "Shift+Space", key: " ", code: "Space", shift: true },
  { name: "Alt+Space", key: " ", code: "Space", alt: true },
  { name: "Backspace", key: "Backspace", code: "Backspace" },
  { name: "Delete", key: "Delete", code: "Delete" },
  { name: "Enter", key: "Enter", code: "Enter" },
  { name: "Tab", key: "Tab", code: "Tab" },
  { name: "ArrowUp", key: "ArrowUp", code: "ArrowUp" },
  { name: "ArrowDown", key: "ArrowDown", code: "ArrowDown" },
  { name: "ArrowLeft", key: "ArrowLeft", code: "ArrowLeft" },
  { name: "ArrowRight", key: "ArrowRight", code: "ArrowRight" },
  { name: "f", key: "f", code: "KeyF" },
  { name: "-", key: "-", code: "Minus" },
  { name: "+", key: "+", code: "Equal" },
  { name: "⌘A", key: "a", code: "KeyA", meta: true },
  { name: "⌘Z", key: "z", code: "KeyZ", meta: true },
  { name: "⌘C", key: "c", code: "KeyC", meta: true },
  { name: "⌘V", key: "v", code: "KeyV", meta: true },
];

/** `found`: whether a code editor's element was there to dispatch on, and `target` its tag (a `TEXTAREA`: cut and paste work in it). `taken`: the keys some handler called `preventDefault()` on. `hosts`: the code areas on the page, editor made or not (what a failed `found` says it saw). */
export function checkKeys(): { found: boolean; target: string; taken: string[]; hosts: number } {
  const hosts = document.querySelectorAll(".monaco-host").length;
  const target = document.querySelector(".monaco-host .native-edit-context, .monaco-host textarea");
  if (!target) return { found: false, target: "", taken: [], hosts };
  const taken = CHECKED_KEYS.filter((k) => {
    const event = new KeyboardEvent("keydown", { key: k.key, code: k.code, metaKey: !!k.meta, shiftKey: !!k.shift, altKey: !!k.alt, bubbles: true, cancelable: true, composed: true });
    target.dispatchEvent(event);
    return event.defaultPrevented;
  }).map((k) => k.name);
  return { found: true, target: target.nodeName, taken, hosts };
}
