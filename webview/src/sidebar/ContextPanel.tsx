import { useMemo } from "react";

import type { StructureResult } from "@vnoc/shared/messages.ts";
import { NONE, openOf } from "@vnoc/shared/open.ts";
import { cn } from "@/lib/utils.ts";
import { useSession } from "../state/session.ts";
import { activate } from "./actions.ts";
import { flatten } from "./flatten.ts";
import { FocusBar } from "./FocusBar.tsx";
import { PanelHeader } from "./PanelHeader.tsx";
import type { Panel } from "./panels.ts";
import { Tree } from "./Tree.tsx";

/**
 * One context's panel (06 §4): its header (with two or more contexts), its own focus bar while it has a focus, and its own list of rows: `flatten` of THAT
 * context's open set and focus, its own selection. A press anywhere in it makes it the active context, and the press then does what it does. The active one has
 * a 2 px bar on its left. `height`: px, or none to fill the view (one panel). `splitter` is drawn on its top edge. Reads the session and draws.
 */
export function ContextPanel({ panel, headers, structure, height, splitter, sectionRef, onList, onTab }: {
  panel: Panel;
  headers: boolean;
  structure: StructureResult;
  height: number | null;
  splitter: React.ReactNode;
  /** the panel's element: a drag sets its height directly */
  sectionRef: (el: HTMLElement | null) => void;
  onList: (el: HTMLDivElement | null) => void;
  onTab: (shift: boolean) => boolean;
}) {
  const focus = useSession((s) => s.contexts[panel.id]?.focus ?? NONE);
  const selected = useSession((s) => s.contexts[panel.id]?.selected ?? null);
  const open = useSession((s) => (s.contexts[panel.id] ? openOf(s.contexts[panel.id]) : NONE));
  const cursor = useSession((s) => s.cursor);
  const highlight = useSession((s) => s.highlight);
  const focusId = focus.length > 0 ? focus[focus.length - 1] : null;
  const rows = useMemo(() => flatten(structure.root, new Set(open), focusId), [structure, open, focusId]);
  return (
    <section
      ref={sectionRef}
      aria-label={panel.label}
      data-active={panel.active}
      className={cn("relative flex min-h-0 flex-col border-l-2", headers && "border-t border-t-border", headers && panel.active ? "border-l-[color:var(--ring)]" : "border-l-transparent", height === null && "flex-1")}
      style={height === null ? undefined : { height, flex: "none" }}
      onPointerDown={() => activate(panel.id)}
    >
      {splitter}
      {headers && <PanelHeader panel={panel} />}
      {focusId !== null && <FocusBar structure={structure} focus={focus} context={panel.id} />}
      <Tree context={panel.id} rows={rows} selected={selected} cursor={cursor} highlight={highlight} onList={onList} onTab={onTab} />
    </section>
  );
}
