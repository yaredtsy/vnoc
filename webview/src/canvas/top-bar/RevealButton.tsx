import { BarButton } from "@/components/BarButton.tsx";
import { dispatch, useSession } from "@/state/session.ts";

/** "Where is the thing I selected?": opens what hides it and brings its card into view. */
export function RevealButton() {
  const selected = useSession((s) => s.contexts[s.active].selected);
  return <BarButton icon="target" label="Reveal the selection" disabled={selected === null} onClick={() => selected && dispatch({ type: "reveal", id: selected, canvas: true })} />;
}
