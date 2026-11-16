import type { StructureResult } from "@vnoc/shared/messages.ts";
import { ProgressLine } from "@/components/ProgressLine.tsx";
import { useSession } from "@/state/session.ts";
import { BackButton } from "./BackButton.tsx";
import { ContextChips } from "./ContextChips.tsx";
import { BudgetNote } from "./BudgetNote.tsx";
import { Crumbs } from "./Crumbs.tsx";
import { DocsToggle } from "./DocsToggle.tsx";
import { MoreMenu } from "./MoreMenu.tsx";
import { RevealButton } from "./RevealButton.tsx";
import { UnsavedDrafts } from "./UnsavedDrafts.tsx";
import { statsLine } from "./statsLine.ts";
import { useCrumbs } from "./useCrumbs.ts";
import { useElementWidth } from "./useElementWidth.ts";
import { Warning } from "./Warning.tsx";

/** The canvas's one bar (docs/16-ui-redesign/02-top-bar.md): back, the contexts, where you are, ⚠, reveal, docs, more. Draws; keeps no state of its own. */
export function TopBar({ structure, status, message, cards }: { structure: StructureResult | null; status: "loading" | "ready" | "error"; message: string; cards: { drawn: number; total: number } | null }) {
  const focus = useSession((s) => s.contexts[s.active].focus);
  const crumbs = useCrumbs(structure?.root ?? null, focus);
  const [ref, width] = useElementWidth<HTMLElement>();
  return (
    <header ref={ref} className="relative flex h-[35px] flex-none items-center gap-1 border-b border-border px-2">
      <BackButton crumbs={crumbs} />
      <ContextChips width={width} />
      <Crumbs project={structure?.root.name ?? "V-NOC"} stats={structure ? statsLine(structure) : ""} crumbs={crumbs} width={width} />
      {cards && <BudgetNote drawn={cards.drawn} total={cards.total} />}
      <UnsavedDrafts />
      <Warning structure={structure} status={status} message={message} />
      <RevealButton />
      <DocsToggle />
      <MoreMenu />
      {status === "loading" && <ProgressLine label="Reading the code" />}
    </header>
  );
}
