import { useEffect } from "react";

import { Codicon } from "@/components/Codicon.tsx";
import { cn } from "@/lib/utils.ts";
import { KINDS, kindIcon } from "../node-card/kinds.ts";
import { hoverParts } from "./hover.ts";
import { type Result, splitMatch } from "./search.ts";

/** The id of a result's element (the box's `aria-activedescendant`). */
export const resultId = (i: number): string => `result-${i}`;

/**
 * The flat list that takes the panels' place while there is a query (06 §5): the kind's icon, the name with the matching letters bold, the path dim. The
 * active result has the list's selection colour and is kept in view. A click picks. Draws; `useSearch` decides.
 */
export function SearchResults({ results, active, onPick }: { results: Result[]; active: number; onPick: (result: Result) => void }) {
  useEffect(() => {
    document.getElementById(resultId(active))?.scrollIntoView({ block: "nearest" });
  }, [active, results]);
  if (results.length === 0) return <p className="flex-1 p-3 text-muted-foreground">No matches.</p>;
  return (
    <div id="search-results" role="listbox" aria-label="Search results" className="min-h-0 flex-1 overflow-y-auto">
      {results.map((r, i) => {
        const [before, match, after] = splitMatch(r.node.name, r.match);
        return (
          <div
            key={r.node.id}
            id={resultId(i)}
            role="option"
            aria-selected={i === active}
            className={cn("flex h-[22px] cursor-pointer items-center gap-1 px-2 select-none", i === active ? "bg-[var(--card-selected-bg)] text-[color:var(--list-active-fg)]" : "hover:bg-[var(--list-hover-bg)]")}
            onMouseDown={(e) => e.preventDefault()} // the box keeps the keyboard
            onClick={() => onPick(r)}
          >
            <span className="flex size-4 flex-none items-center justify-center" style={{ color: KINDS[r.node.kind].color }}>
              <Codicon name={kindIcon(r.node.kind, r.inClass)} className="text-[16px]" />
            </span>
            <span className="flex-none truncate">
              {before}
              {match && <b className="text-[color:var(--list-highlight)]">{match}</b>}
              {after}
            </span>
            <span className={cn("min-w-0 flex-1 truncate text-[0.92em]", i === active ? "opacity-80" : "text-muted-foreground")}>{hoverParts(r.node).where}</span>
          </div>
        );
      })}
    </div>
  );
}
