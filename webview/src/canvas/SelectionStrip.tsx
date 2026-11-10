import { IconFocus2, IconCurrentLocation, IconX } from "@tabler/icons-react";

import { Button } from "@/components/ui/button.tsx";
import type { TreeNode } from "@vnoc/shared/messages.ts";
import { request } from "../host.ts";
import { KINDS } from "../node-card/kinds.ts";
import { dispatch } from "../state/session.ts";

// The answer to "what is selected?", always on screen: also when the card is scrolled away or hidden inside a closed card
// (docs/10-ui-rework/06-how-it-looks.md §4).

export function SelectionStrip({ node }: { node: TreeNode }) {
  const where = node.range ? `${node.path}:${node.range.start.line + 1}` : node.path;
  return (
    <div className="flex items-center gap-2 border-b border-border px-3 py-1 text-xs" aria-label="The selected node">
      <span className="opacity-80">{KINDS[node.kind].icon}</span>
      <strong className="truncate">{KINDS[node.kind].title(node.name)}</strong>
      <span className="min-w-0 flex-1 truncate font-mono text-muted-foreground" title={where}>
        {where}
      </span>
      <Button variant="ghost" size="xs" title="Open what hides it and bring it into view" onClick={() => dispatch({ type: "reveal", id: node.id, canvas: true })}>
        <IconCurrentLocation /> Show
      </Button>
      <Button variant="ghost" size="xs" title="Focus on it" onClick={() => dispatch({ type: "focus", id: node.id })}>
        <IconFocus2 /> Focus
      </Button>
      {node.kind !== "folder" && (
        <Button variant="ghost" size="xs" title="Go to the code" onClick={() => void request("goToCode", { key: node.id }).catch(() => {})}>
          ↗ Code
        </Button>
      )}
      <Button variant="ghost" size="icon-xs" title="Clear the selection" aria-label="Clear the selection" onClick={() => dispatch({ type: "select", id: null })}>
        <IconX />
      </Button>
    </div>
  );
}
