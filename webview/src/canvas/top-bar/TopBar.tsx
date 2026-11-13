import type { StructureResult } from "@vnoc/shared/messages.ts";
import { useSession } from "@/state/session.ts";
import { BackButton } from "./BackButton.tsx";
import { Crumbs } from "./Crumbs.tsx";
import { DocsToggle } from "./DocsToggle.tsx";
import { MoreMenu } from "./MoreMenu.tsx";
import { RevealButton } from "./RevealButton.tsx";
import { useCrumbs } from "./useCrumbs.ts";

/** "reading the code…" is the bar's thin progress line (the canvas says it in words); the project's numbers are the first crumb's tooltip. */
const statsLine = (s: StructureResult) =>
  `${s.stats.folders} folders · ${s.stats.files} files · ${s.stats.classes} classes · ${s.stats.functions} functions`;

/** The canvas's one bar (docs/16-ui-redesign/02-top-bar.md): back, where you are, reveal, docs, more. Draws; keeps no state of its own. */
export function TopBar({ structure, loading }: { structure: StructureResult | null; loading: boolean }) {
  const focus = useSession((s) => s.contexts[s.active].focus);
  const crumbs = useCrumbs(structure?.root ?? null, focus);
  return (
    <header className="relative flex h-[30px] flex-none items-center gap-1 border-b border-border px-1.5">
      <BackButton crumbs={crumbs} />
      <Crumbs project={structure?.root.name ?? "V-NOC"} stats={structure ? statsLine(structure) : ""} crumbs={crumbs} />
      <RevealButton />
      <DocsToggle />
      <MoreMenu />
      {loading && <div className="absolute inset-x-0 bottom-0 h-0.5 animate-pulse bg-primary" role="progressbar" aria-label="Reading the code" />}
    </header>
  );
}
