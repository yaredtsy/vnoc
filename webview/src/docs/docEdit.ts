// What the doc panel shows of a doc's draft (docs/17-edit-in-canvas.md §8): which buttons, and which line above the editor. Pure, so the tables are tested; `DocPanel` only draws them. The
// rules are the card's (`cardButtons.ts`, §4) with the doc panel's two buttons in the reading state; the lines are the card's too (`draftRules.ts`).

import type { DraftState, Page } from "@vnoc/shared/messages.ts";
import { type EditState } from "../node-card/editState.ts";
import { type DraftAction, type Notice, stateLine } from "../state/draftRules.ts";
import { busyText } from "@vnoc/shared/drafts.ts";

export type DocButtonId = "edit" | "openInEditor" | "save" | "discard";
export interface DocButton {
  id: DocButtonId;
  disabled: boolean;
  /** Edit while the source is shown with no draft: the pencil is down, and pressing it shows the rendered doc again */
  pressed: boolean;
}

/**
 * Reading, no draft: the pencil, **Edit**, and **Open in Editor** (the `.md` in a tab). The panel shows rendered Markdown, which cannot take typing, so the pencil switches to the source (Monaco)
 * with no draft yet (9.7): from there it works as on a card, typing starts the draft. `source`: the source is shown; the pencil is down, and pressing it shows the rendered doc again. A draft in
 * any of its states: **Save** · **Discard** in the pencil's place, as on a card: Save is enabled only while editing a draft that differs from its start; Discard is disabled while it saves; in
 * `conflict` and `gone` Save is disabled and Discard is not. Nothing without a doc. `state` is the draft's, or `reading` when there is none, and `closed` when the panel has no doc to show.
 */
export function docButtons(state: EditState, changed: boolean, source = false): DocButton[] {
  if (state === "closed") return [];
  const ids: DocButtonId[] = state === "reading" ? ["edit", "openInEditor"] : ["save", "discard"];
  return ids.map((id) => ({ id, disabled: (id === "save" && (state !== "editing" || !changed)) || (id === "discard" && state === "saving"), pressed: id === "edit" && source }));
}

export type DocLine =
  /** E5: the `.md` has unsaved changes in a tab: "Save `<file>` first" and Save and Edit */
  | { kind: "refusal" }
  /** the other page has a draft of this doc */
  | { kind: "busy"; text: string }
  /** a draft in conflict or gone: what happened, and what can be done about it */
  | { kind: "state"; text: string; actions: DraftAction[] }
  /** what the last Save did: written but not saved, or why it failed */
  | { kind: "notice"; notice: Notice };

/**
 * The one line above the editor, in the card's order: the refusal, the other page's draft, the state of the draft, what the last Save did. `asking` is the page this panel is on, so a
 * busy doc names the other one. A refusal for a doc stays until Edit is asked again and the file is clean (the page is not told when a `.md` outside the opened folder is saved).
 */
export function docLine(i: { refused: boolean; busy: boolean; asking: Page; state: DraftState | null; file: string; notice?: Notice }): DocLine | null {
  if (i.refused) return { kind: "refusal" };
  if (i.busy) return { kind: "busy", text: busyText(i.asking) };
  const line = i.state ? stateLine(i.state, i.file) : null;
  if (line) return { kind: "state", ...line };
  return i.notice ? { kind: "notice", notice: i.notice } : null;
}
