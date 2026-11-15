// The bar's note when the card budget held something back (docs/16-ui-redesign/03-canvas.md §6.1). Pure, so the words are tested.

export interface BudgetNote {
  text: string;
  tip: string;
}

/** "200 of 640 cards" when some open cards were drawn closed; nothing when everything asked for is drawn. */
export function budgetNote(drawn: number, total: number): BudgetNote | null {
  if (total <= drawn) return null;
  return { text: `${drawn} of ${total} cards`, tip: "Some open cards are drawn closed to keep the canvas fast. Focus on a part to see all of it." };
}
