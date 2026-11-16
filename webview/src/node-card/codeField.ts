// The element a card's code is typed into, and the one rule that says a key typed there is typing (9.2c). React Flow takes Space (pan), Backspace (delete) and its other
// keys unless the key's target is an `input`, `select`, `textarea`, an element with `contenteditable`, or inside `.nokey`; Monaco types into a `native-edit-context`
// div that is none of those, so its container carries `nokey`. `useCanvasKeys` (F, the arrows, Enter…) asks the same question with `TEXT_FIELD`, so there is one rule,
// not a list of Monaco's elements.

/** `nodrag nopan nowheel`: selecting text must not drag the card, and the wheel scrolls the code, not the canvas. `nokey`: its keys are typing. */
export const CODE_FIELD_CLASS = "node-code monaco-host nodrag nopan nowheel nokey";

/** What a key's target may be inside for the canvas to leave the key alone. */
export const TEXT_FIELD = "input, textarea, select, [contenteditable], .nokey";
