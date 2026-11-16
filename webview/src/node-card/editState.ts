// The states of a card's code (docs/17-edit-in-canvas.md §4). `closed`: the code is not shown. `reading`: shown, read-only. The other four are a draft's states.
// Pure types only; the rules that use them are `cardButtons.ts` (which buttons), `cardKeys.ts` (which keys) and `state/draftRules.ts`.

export type DraftState = "editing" | "saving" | "conflict" | "gone";
export type EditState = "closed" | "reading" | DraftState;

/** What the header's buttons need of the card's edit state: `changed` is whether the draft differs from the text it started with. */
export interface CardEdit {
  state: EditState;
  changed: boolean;
}

export const NOT_EDITING: CardEdit = { state: "closed", changed: false };

/** The card's state: a draft's own when it has one, else whether its code is shown. */
export function editStateOf(codeOpen: boolean, draft: { state: DraftState } | undefined): EditState {
  if (draft) return draft.state;
  return codeOpen ? "reading" : "closed";
}
