// Which buttons a card shows (docs/16-ui-redesign/04-node-cards.md §3). Pure, so the table is tested; ActionStrip only draws it.

import type { Placed } from "../canvas/layout.ts";

export type ButtonId = "focus" | "goToCode" | "code" | "addDoc";
export interface CardButton {
  id: ButtonId;
  disabled: boolean;
}

/**
 * folder: Focus · Add Doc. file: Focus · Go to Code · Add Doc. class, function: Focus · Go to Code · Code in Card · Add Doc.
 * The opened folder (id "/") is not a node: no buttons (Focus on it would push "/" into the focus path, and no doc attaches to it).
 * On the root of a focused view Focus is there but does nothing. A file's code is the whole file, so `</>` is for a definition with a range.
 */
export function cardButtons(node: Pick<Placed, "id" | "kind" | "range">, focusRoot: boolean): CardButton[] {
  if (node.id === "/") return [];
  const ids: ButtonId[] = ["focus"];
  if (node.kind !== "folder") ids.push("goToCode");
  if ((node.kind === "class" || node.kind === "function") && node.range) ids.push("code");
  ids.push("addDoc");
  return ids.map((id) => ({ id, disabled: id === "focus" && focusRoot }));
}
