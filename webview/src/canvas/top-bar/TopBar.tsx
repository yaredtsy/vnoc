import type { StructureResult } from "@vnoc/shared/messages.ts";
import { ProgressLine } from "@/components/ProgressLine.tsx";
import { useSession } from "@/state/session.ts";
import { BackButton } from "./BackButton.tsx";
import { Crumbs } from "./Crumbs.tsx";
import { DocsToggle } from "./DocsToggle.tsx";
import { MoreMenu } from "./MoreMenu.tsx";
import { RevealButton } from "./RevealButton.tsx";
import { useCrumbs } from "./useCrumbs.ts";
import { useElementWidth } from "./useElementWidth.ts";
import { Warning } from "./Warning.tsx";

/** The project's numbers: the first crumb's tooltip. */
const statsLine = (s: StructureResult) =>
  `${s.stats.folders} folders · ${s.stats.files} files · ${s.stats.classes} classes · ${s.stats.functions} functions`;

/** The canvas's one bar (docs/16-ui-redesign/02-top-bar.md): back, where you are, ⚠, reveal, docs, more. Draws; keeps no state of its own. */
export function TopBar({ structure, status, message }: { structure: StructureResult | null; status: "loading" | "ready" | "error"; message: string }) {
  const focus = useSession((s) => s.contexts[s.active].focus);
  const crumbs = useCrumbs(structure?.root ?? null, focus);
  const [ref, width] = useElementWidth<HTMLElement>();
  return (
    <header ref={ref} className="relative flex h-[30px] flex-none items-center gap-1 border-b border-border px-1.5">
      <BackButton crumbs={crumbs} />
      <Crumbs project={structure?.root.name ?? "V-NOC"} stats={structure ? statsLine(structure) : ""} crumbs={crumbs} width={width} />
      <Warning structure={structure} status={status} message={message} />
      <RevealButton />
      <DocsToggle />
      <MoreMenu />
      {status === "loading" && <ProgressLine label="Reading the code" />}
    </header>
  );
}
