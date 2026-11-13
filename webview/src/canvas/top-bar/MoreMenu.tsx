import { useReactFlow } from "@xyflow/react";
import { IconDots } from "@tabler/icons-react";

import { IconButton } from "@/components/IconButton.tsx";
import { DropdownMenu, DropdownMenuCheckboxItem, DropdownMenuContent, DropdownMenuItem, DropdownMenuSeparator, DropdownMenuTrigger } from "@/components/ui/dropdown-menu.tsx";
import { refresh } from "@/state/data.ts";
import { dispatch } from "@/state/session.ts";
import { setMinimap, useView } from "@/state/view.ts";

/** The rare actions: they change the view, never the code. */
export function MoreMenu() {
  const { fitView } = useReactFlow();
  const minimap = useView((v) => v.minimap);
  return (
    <DropdownMenu>
      <DropdownMenuTrigger render={<IconButton label="More" />}>
        <IconDots />
      </DropdownMenuTrigger>
      <DropdownMenuContent>
        <DropdownMenuItem onClick={() => void fitView({ duration: 300 })}>Fit View</DropdownMenuItem>
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
