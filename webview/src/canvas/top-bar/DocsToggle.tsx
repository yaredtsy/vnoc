import { IconInfoCircle } from "@tabler/icons-react";

import { IconButton } from "@/components/IconButton.tsx";
import { setDocsOpen, useView } from "@/state/view.ts";

/** Shows or hides the docs panel on the right (pressed while it is open). */
export function DocsToggle() {
  const open = useView((v) => v.docsOpen);
  return (
    <IconButton label="Docs of the selected node" variant={open ? "secondary" : "ghost"} aria-pressed={open} onClick={() => setDocsOpen(!open)}>
      <IconInfoCircle />
    </IconButton>
  );
}
