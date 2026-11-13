import { IconChevronRight } from "@tabler/icons-react";

import { Button } from "@/components/ui/button.tsx";
import { DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuTrigger } from "@/components/ui/dropdown-menu.tsx";
import type { Step } from "./foldCrumbs.ts";

/** `… ›`: the steps that do not fit, as a menu. A step jumps there. */
export function FoldedCrumbs({ steps, onJump }: { steps: Step[]; onJump: (depth: number) => void }) {
  return (
    <span className="flex flex-none items-center">
      <DropdownMenu>
        <DropdownMenuTrigger render={<Button variant="ghost" size="xs" className="px-1.5" aria-label="Earlier steps" title="Earlier steps" />}>…</DropdownMenuTrigger>
        <DropdownMenuContent align="start">
          {steps.map((s) => (
            <DropdownMenuItem key={s.id} onClick={() => onJump(s.depth)}>
              {s.label}
            </DropdownMenuItem>
          ))}
        </DropdownMenuContent>
      </DropdownMenu>
      <IconChevronRight className="size-3 flex-none opacity-50" />
    </span>
  );
}
