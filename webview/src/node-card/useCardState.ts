import { useSession } from "../state/session.ts";
import { useView } from "../state/view.ts";

/**
 * How the session and the view mark one card (docs/10-ui-rework/06-how-it-looks.md §2). Each selector returns a boolean, so a click draws again
 * only the two cards whose state changed. Every hook is always called: `a || useX()` would skip the second one when the first is true.
 */
export function useCardState(id: string, depth: number) {
  const contextId = useSession((s) => s.active);
  const selected = useSession((s) => s.contexts[s.active].selected === id);
  // the card at the top of a focused view (the focused node is always the first card drawn)
  const focusRoot = useSession((s) => s.contexts[s.active].focus.length > 0) && depth === 0;
  const pointedAt = useSession((s) => s.highlight === id); // the search list, a crumb under the pointer
  const hovered = useView((v) => v.hover === id);
  const cursor = useSession((s) => s.cursor === id);
  const revealed = useView((v) => v.revealed === id); // brought into view a moment ago
  return { contextId, selected, focusRoot, cursor, revealed, highlighted: pointedAt || hovered };
}
