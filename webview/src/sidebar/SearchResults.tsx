import { useEffect } from "react";

import { Codicon } from "@/components/Codicon.tsx";
import { cn } from "@/lib/utils.ts";
import type { SearchNode } from "@vnoc/shared/messages.ts";
import { KINDS, kindIcon } from "../node-card/kinds.ts";
import { nodesHeader, placeOf, splitMatch } from "./search.ts";

/** The id of a result's element (the box's `aria-activedescendant`). */
export const resultId = (i: number): string => `result-${i}`;

/**
 * The flat list that takes the panels' place while there is a query (06 §5): a header with the count, then the core's rows: the kind's icon, the name with the
 * letters the core found bold, where the node is dim. The active row has the list's selection colour and is kept in view. A click picks. Draws; `useSearch` decides.
 * `noMatches`: the answer for the box's text came and has no rows.
 */
export function SearchResults({ rows, total, noMatches, active, onPick }: { rows: SearchNode[]; total: number; noMatches: boolean; active: number; onPick: (row: SearchNode) => void }) {
  useEffect(() => {
    document.getElementById(resultId(active))?.scrollIntoView({ block: "nearest" });
  }, [active, rows]);
  if (rows.length === 0) return noMatches ? <p className="flex-1 p-3 text-muted-foreground">No matches.</p> : <div className="flex-1" />;
  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <div className="flex-none px-2 py-0.5 text-[0.85em] text-muted-foreground select-none">{nodesHeader(rows.length, total)}</div>
      <div id="search-results" role="listbox" aria-label="Search results" className="min-h-0 flex-1 overflow-y-auto">
        {rows.map((r, i) => (
          <div
            key={r.id}
            id={resultId(i)}
            role="option"
            aria-selected={i === active}
            className={cn("flex h-[22px] cursor-pointer items-center gap-1 px-2 select-none", i === active ? "bg-[var(--card-selected-bg)] text-[color:var(--list-active-fg)]" : "hover:bg-[var(--list-hover-bg)]")}
            onMouseDown={(e) => e.preventDefault()} // the box keeps the keyboard
            onClick={() => onPick(r)}
          >
            <span className="flex size-4 flex-none items-center justify-center" style={{ color: KINDS[r.kind].color }}>
              <Codicon name={kindIcon(r.kind, r.in_class)} className="text-[16px]" />
            </span>
            <span className="flex-none truncate">
              {splitMatch(r.name, r.match).map((piece, k) =>
                piece.bold ? (
                  <b key={k} className="text-[color:var(--list-highlight)]">
                    {piece.text}
                  </b>
                ) : (
                  piece.text
                ),
              )}
            </span>
            <span className={cn("min-w-0 flex-1 truncate text-[0.92em]", i === active ? "opacity-80" : "text-muted-foreground")}>{placeOf(r)}</span>
          </div>
        ))}
      </div>
    </div>
  );
}
