import { IconCurrentLocation } from "@tabler/icons-react";

import { IconButton } from "@/components/IconButton.tsx";
import { dispatch, useSession } from "@/state/session.ts";

/** "Where is the thing I selected?": opens what hides it and brings its card into view. */
export function RevealButton() {
  const selected = useSession((s) => s.contexts[s.active].selected);
  return (
    <IconButton label="Reveal the selection" disabled={selected === null} onClick={() => selected && dispatch({ type: "reveal", id: selected, canvas: true })}>
      <IconCurrentLocation />
    </IconButton>
  );
}
