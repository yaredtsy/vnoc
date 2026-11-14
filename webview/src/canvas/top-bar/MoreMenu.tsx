import { useReactFlow } from "@xyflow/react";

import { BarButton } from "@/components/BarButton.tsx";
import { DropdownMenu, DropdownMenuCheckboxItem, DropdownMenuContent, DropdownMenuItem, DropdownMenuSeparator, DropdownMenuTrigger } from "@/components/ui/dropdown-menu.tsx";
import { motionMs, prefersReducedMotion } from "@/lib/motion.ts";
import { refresh } from "@/state/data.ts";
import { dispatch } from "@/state/session.ts";
import { setMinimap, useView } from "@/state/view.ts";

/** The rare actions: they change the view, never the code. */
export function MoreMenu() {
  const { fitView } = useReactFlow();
  const minimap = useView((v) => v.minimap);
  return (
    <DropdownMenu>
      <DropdownMenuTrigger render={<BarButton icon="ellipsis" label="More" />} />
      <DropdownMenuContent>
        <DropdownMenuItem onClick={() => void fitView({ duration: motionMs(300, prefersReducedMotion()) })}>Fit View</DropdownMenuItem>
        <DropdownMenuItem onClick={() => dispatch({ type: "collapseAll" })}>Collapse All</DropdownMenuItem>
        <DropdownMenuCheckboxItem checked={minimap} onCheckedChange={setMinimap}>
          Show Minimap
        </DropdownMenuCheckboxItem>
        <DropdownMenuSeparator />
        <DropdownMenuItem onClick={() => refresh()}>Refresh Structure</DropdownMenuItem>
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
