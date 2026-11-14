// Which buttons a card shows (docs/16-ui-redesign/04-node-cards.md §3). Pure, so the table is tested; ActionStrip only draws it.

import type { Placed } from "../canvas/layout.ts";

export type ButtonId = "focus" | "explore" | "goToCode" | "code" | "addDoc";
export interface CardButton {
  id: ButtonId;
  disabled: boolean;
}

/**
 * folder: Focus · Add Doc. file: Focus · Go to Code · Add Doc. class, function: Focus · Go to Code · Code in Card · Add Doc.
 * A call card: its callee known → Explore · Go to Code · Code in Card (the callee's, when its range is known); an unknown callee → none (its sites are the
 * meta line's links, and there is no Go to Call Site button: on a merged node it could not know which site you mean, 04 §2.1).
 * The opened folder (id "/") is not a node: no buttons (Focus on it would push "/" into the focus path, and no doc attaches to it).
 * On the root of a focused view Focus is there but does nothing. A file's code is the whole file, so `</>` is for a definition with a range.
 */
export function cardButtons(node: Pick<Placed, "id" | "kind" | "range" | "role" | "key">, focusRoot: boolean): CardButton[] {
  if (node.id === "/") return [];
  if (node.role === "call") {
    if (node.key === null) return [];
    const ids: ButtonId[] = ["explore", "goToCode"];
    if (node.range) ids.push("code");
    return ids.map((id) => ({ id, disabled: false }));
  }
  const ids: ButtonId[] = ["focus"];
  if (node.kind !== "folder") ids.push("goToCode");
  if ((node.kind === "class" || node.kind === "function") && node.range) ids.push("code");
  ids.push("addDoc");
  return ids.map((id) => ({ id, disabled: id === "focus" && focusRoot }));
}
