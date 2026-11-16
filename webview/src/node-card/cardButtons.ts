// Which buttons a card shows (docs/16-ui-redesign/04-node-cards.md §3). Pure, so the table is tested; ActionStrip only draws it.

import type { Placed } from "../canvas/layout.ts";
import { type CardEdit, type EditState, NOT_EDITING } from "./editState.ts";

export type ButtonId = "focus" | "explore" | "goToCode" | "code" | "save" | "discard" | "addDoc";
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
 * A sidebar row has the same buttons without `</>`: it changes a card, and a row has none (07 §3).
 * The code's slot (docs/17-edit-in-canvas.md §4): code closed, or shown with no draft: `</>` (there is no Edit button: the code takes typing, and the first keystroke starts the draft,
 * 9.7); a draft in any of its states: Save · Discard in the same place (so the code cannot be closed with its button in the middle of a draft). Save is enabled only while editing a draft that differs from its start;
 * Discard is disabled while it saves. In `conflict` and `gone` Save is disabled and Discard is not. A call card has the same slot, for its callee's code.
 * A sidebar row never shows any of them.
 */
export function cardButtons(node: Pick<Placed, "id" | "kind" | "range" | "role" | "key">, focusRoot: boolean, surface: "canvas" | "sidebar" = "canvas", edit: CardEdit = NOT_EDITING): CardButton[] {
  if (node.id === "/") return [];
  const state = surface === "canvas" ? edit.state : "closed";
  const disabled = (id: ButtonId) => (id === "focus" && focusRoot) || (id === "save" && (state !== "editing" || !edit.changed)) || (id === "discard" && state === "saving");
  let ids: ButtonId[];
  if (node.role === "call") {
    if (node.key === null) return [];
    ids = ["explore", "goToCode"];
    if (node.range) ids.push(...codeSlot(state));
  } else {
    ids = ["focus"];
    if (node.kind !== "folder") ids.push("goToCode");
    if (surface === "canvas" && (node.kind === "class" || node.kind === "function") && node.range) ids.push(...codeSlot(state));
    ids.push("addDoc");
  }
  return ids.map((id) => ({ id, disabled: disabled(id) }));
}

function codeSlot(state: EditState): ButtonId[] {
  return state === "closed" || state === "reading" ? ["code"] : ["save", "discard"];
}
