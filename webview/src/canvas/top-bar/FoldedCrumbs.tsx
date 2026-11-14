import { Codicon } from "@/components/Codicon.tsx";
import { Button } from "@/components/ui/button.tsx";
import { DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuTrigger } from "@/components/ui/dropdown-menu.tsx";
import type { Step } from "./foldCrumbs.ts";

/** `… ›`: the steps that do not fit, as a menu. A step jumps there. */
export function FoldedCrumbs({ steps, onJump }: { steps: Step[]; onJump: (depth: number) => void }) {
  return (
    <span className="flex flex-none items-center">
      <DropdownMenu>
        <DropdownMenuTrigger render={<Button variant="ghost" className="h-[22px] rounded-[5px] px-1.5 text-[length:var(--font-size)] font-normal text-muted-foreground hover:bg-toolbar-hover" aria-label="Earlier steps" title="Earlier steps" />}>…</DropdownMenuTrigger>
        <DropdownMenuContent align="start">
          {steps.map((s) => (
            <DropdownMenuItem key={s.id} onClick={() => onJump(s.depth)}>
              {s.label}
            </DropdownMenuItem>
          ))}
        </DropdownMenuContent>
      </DropdownMenu>
      <Codicon name="chevron-right" className="flex-none text-[16px] text-muted-foreground" />
    </span>
  );
}
