import { useEffect, useMemo } from "react";

import { callFacts } from "@vnoc/shared/calls.ts";
import { Codicon } from "@/components/Codicon.tsx";
import { DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuTrigger } from "@/components/ui/dropdown-menu.tsx";
import { post } from "@/host.ts";
import { useCalls } from "@/state/calls.ts";
import { dispatch, useSession } from "@/state/session.ts";
import { cn } from "@/lib/utils.ts";
import { type ContextChip, chipsFor } from "./chipsFor.ts";

const CHIP = "flex h-[22px] flex-none items-center gap-1 rounded-[5px] border px-1.5 text-[length:var(--font-size)]";
const filled = "border-transparent bg-chip-active text-chip-active-foreground";
const outlined = "border-border text-muted-foreground hover:bg-toolbar-hover hover:text-foreground";

/** One chip: the name activates the context; an EXPLORE chip has a ✕ that closes it (and its children). */
function Chip({ chip }: { chip: ContextChip }) {
  return (
    <span className={cn(CHIP, "max-w-40 pr-1", chip.active ? filled : outlined, !chip.closable && "pr-1.5")} data-active={chip.active} data-context={chip.id}>
      <button type="button" className="flex min-w-0 items-center gap-1 outline-none" title={chip.tip} aria-pressed={chip.active} onClick={() => dispatch({ type: "activate", id: chip.id })}>
        {chip.closable && <Codicon name="compass" className="flex-none text-[13px]" />}
        <span className="truncate">{chip.label}</span>
      </button>
      {chip.closable && (
        <button type="button" className="flex size-4 flex-none items-center justify-center rounded-[3px] hover:bg-toolbar-hover" title={`Close ${chip.label}`} aria-label={`Close ${chip.label}`} onClick={() => dispatch({ type: "closeContext", id: chip.id })}>
          <Codicon name="close" className="text-[12px]" />
        </button>
      )}
    </span>
  );
}

/** A menu of contexts: `+2 ▾` for the folded ones, the active one's name and a ▾ on a narrow bar. */
function ContextMenu({ label, items, filledChip, tip }: { label: string; items: ContextChip[]; filledChip: boolean; tip: string }) {
  return (
    <DropdownMenu>
      <DropdownMenuTrigger render={<button type="button" className={cn(CHIP, "max-w-48", filledChip ? filled : outlined)} title={tip} aria-label={tip} />}>
        <span className="truncate">{label}</span>
        <Codicon name="chevron-down" className="flex-none text-[13px]" />
      </DropdownMenuTrigger>
      <DropdownMenuContent align="start">
        {items.map((c) => (
          <DropdownMenuItem key={c.id} title={c.tip} onClick={() => dispatch({ type: "activate", id: c.id })}>
            {c.active ? <Codicon name="check" /> : <span className="inline-block w-[14px]" />}
            {c.label}
          </DropdownMenuItem>
        ))}
      </DropdownMenuContent>
    </DropdownMenu>
  );
}

/** MAIN and the EXPLORE contexts (docs/16-ui-redesign/02-top-bar.md §3 #2): which one the canvas draws, and the way back. Only with two contexts or more. */
export function ContextChips({ width }: { width: number }) {
  const contexts = useSession((s) => s.contexts);
  const order = useSession((s) => s.order);
  const active = useSession((s) => s.active);
  const trees = useCalls((s) => s.trees);
  const chips = useMemo(() => chipsFor({ contexts, order, active }, width, (call) => callFacts(trees, call)), [contexts, order, active, width, trees]);
  // what the bar draws, for the extension's test suite: the contexts, and which one is active
  const report = useMemo(() => (chips === null ? [] : (chips.mode === "one" ? chips.all : [...chips.shown, ...chips.folded]).map((c) => ({ id: c.id, label: c.label, active: c.active }))), [chips]);
  useEffect(() => post({ type: "chipsDrawn", chips: report }), [report]);
  if (!chips) return null;
  return (
    <div role="group" aria-label="Contexts" className="flex min-w-0 flex-none items-center gap-1 border-r border-border pr-2">
      {chips.mode === "one" ? (
        <ContextMenu label={chips.current.label} items={chips.all} filledChip tip="Contexts" />
      ) : (
        <>
          {/* MAIN first; the folded ones are the earliest EXPLORE chips, so they sit right after it */}
          <Chip chip={chips.shown[0]} />
          {chips.folded.length > 0 && <ContextMenu label={`+${chips.folded.length}`} items={chips.folded} filledChip={false} tip="More explored calls" />}
          {chips.shown.slice(1).map((c) => (
            <Chip key={c.id} chip={c} />
          ))}
        </>
      )}
    </div>
  );
}
