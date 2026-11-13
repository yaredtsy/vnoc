import { useEffect } from "react";

import { IconAlertTriangle } from "@tabler/icons-react";

import { IconButton } from "@/components/IconButton.tsx";
import { DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuTrigger } from "@/components/ui/dropdown-menu.tsx";
import { post, request } from "@/host.ts";
import { cn } from "@/lib/utils.ts";
import { refresh } from "@/state/data.ts";
import { type WarningInput, warningOf } from "./warning.ts";

/**
 * The bar's ⚠ (docs/16-ui-redesign/02-top-bar.md §3 #4): shown when the structure is cut or files could not be read (click: NEEDS ATTENTION), and red
 * when a refresh failed while the code is drawn (click: Try Again · Show Log).
 */
export function Warning(props: WarningInput) {
  const { level, text } = warningOf(props);
  // tell the host what the bar says: the extension's test suite uses it
  useEffect(() => post({ type: "warned", level, text }), [level, text]);
  if (level === "none") return null;

  const icon = <IconAlertTriangle className={cn(level === "error" ? "text-destructive" : "text-warning")} />;
  if (level === "warning") {
    return (
      <IconButton label={text} onClick={() => void request("focusAttention").catch(() => {})}>
        {icon}
      </IconButton>
    );
  }
  return (
    <DropdownMenu>
      <DropdownMenuTrigger render={<IconButton label={text} />}>{icon}</DropdownMenuTrigger>
      <DropdownMenuContent>
        <DropdownMenuItem onClick={() => refresh()}>Try Again</DropdownMenuItem>
        <DropdownMenuItem onClick={() => void request("showLog").catch(() => {})}>Show Log</DropdownMenuItem>
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
