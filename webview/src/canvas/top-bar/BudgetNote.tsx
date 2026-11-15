import { budgetNote } from "./budgetNote.ts";

/** Dim, before the buttons on the right: the canvas drew fewer cards than were open (03 §6.1). Nothing when it drew them all. */
export function BudgetNote({ drawn, total }: { drawn: number; total: number }) {
  const note = budgetNote(drawn, total);
  if (!note) return null;
  return (
    <span className="flex-none whitespace-nowrap px-1 text-[length:var(--font-size)] text-muted-foreground" title={note.tip}>
      {note.text}
    </span>
  );
}
