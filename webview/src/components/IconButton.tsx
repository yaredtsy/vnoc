import type { ComponentProps } from "react";

import { Button } from "@/components/ui/button.tsx";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip.tsx";

type Props = Omit<ComponentProps<typeof Button>, "aria-label" | "title"> & {
  /** what the button does; it is the tooltip and the accessible name */
  label: string;
  /** the key that does the same, shown after the label: "Back (Alt+←)" */
  keys?: string;
};

/** An icon-only button with a tooltip. Every toolbar button in the webview is one, so its name is never missing. */
export function IconButton({ label, keys, variant = "ghost", size = "icon-xs", children, ...props }: Props) {
  const text = keys ? `${label} (${keys})` : label;
  return (
    <Tooltip>
      <TooltipTrigger render={<Button variant={variant} size={size} aria-label={label} {...props} />}>{children}</TooltipTrigger>
      <TooltipContent>{text}</TooltipContent>
    </Tooltip>
  );
}
