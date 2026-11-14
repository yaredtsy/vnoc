import type { ComponentProps } from "react";

import { Codicon } from "@/components/Codicon.tsx";
import { IconButton } from "@/components/IconButton.tsx";
import { cn } from "@/lib/utils.ts";

type Props = Omit<ComponentProps<typeof IconButton>, "children" | "size"> & {
  /** the codicon's name */
  icon: string;
  /** the icon's own colour class, for a state (the warning's yellow, an error's red) */
  iconClassName?: string;
};

/**
 * A button of the canvas bar (docs/16-ui-redesign/02-top-bar.md §2, "Sizes"): a codicon at 16 px in a 22 px square, hover `--toolbar-hover`, radius 5 px,
 * as VS Code's own title actions. Not the kit's `icon-xs` preset (24 px button, 12 px icon).
 */
export function BarButton({ icon, iconClassName, className, ...props }: Props) {
  return (
    <IconButton className={cn("size-[22px] flex-none rounded-[5px] p-0 text-muted-foreground hover:bg-toolbar-hover hover:text-foreground aria-expanded:bg-toolbar-hover aria-pressed:bg-toolbar-hover aria-pressed:text-foreground", className)} {...props}>
      <Codicon name={icon} className={cn("text-[16px]", iconClassName)} />
    </IconButton>
  );
}
