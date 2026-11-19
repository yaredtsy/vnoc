import { useEffect } from "react";

import { Codicon } from "@/components/Codicon.tsx";
import { cn } from "@/lib/utils.ts";
import type { SearchDoc, SearchNode } from "@vnoc/shared/messages.ts";
import { placeOf } from "@vnoc/shared/search.ts";
import { KINDS, kindIcon } from "../node-card/kinds.ts";
import { docsHeader, nodesHeader, splitMatch, tagsLabel } from "./search.ts";
import type { Piece } from "./search.ts";

/** The id of a result's element (the box's `aria-activedescendant`). */
export const resultId = (i: number): string => `result-${i}`;

/** A text cut around its matches: the matched parts bold. */
function Marked({ pieces }: { pieces: Piece[] }) {
  return pieces.map((piece, k) =>
    piece.bold ? (
      <b key={k} className="text-[color:var(--list-highlight)]">
        {piece.text}
      </b>
    ) : (
      piece.text
    ),
  );
}

const HEADER = "flex-none px-2 py-0.5 text-[0.85em] text-muted-foreground select-none";

/**
 * The flat list that takes the panels' place while there is a query (06 §5, 20-search/05 §1): two groups, **Nodes** then **Docs**, each under a header with its count and drawn
 * only when it has rows. A node row: the kind's icon, the name with the letters the core found bold, where the node is dim. A doc row, two lines: the doc icon, the title with its
 * matches bold, then its tags dim (`[plan, billing]`, as the node's name is dim: only when it has some), the node's name dim on the right (a Missing node's has the Missing mark); then the snippet, dim, with its matches bold. The two groups are one list for the keys: the
 * active place counts the node rows first. The active row has the list's selection colour and is kept in view. A click picks. Draws; `useSearch` decides.
 * `notes`: what the core said was wrong with a filter, one line each in the dim colour above the rows. `noMatches`: the answer for the box's text came and has no rows at all.
 */
export function SearchResults({ rows, docs, total, docsTotal, notes, noMatches, active, onPick }: { rows: SearchNode[]; docs: SearchDoc[]; total: number; docsTotal: number; notes: string[]; noMatches: boolean; active: number; onPick: (at: number) => void }) {
  useEffect(() => {
    document.getElementById(resultId(active))?.scrollIntoView({ block: "nearest" });
  }, [active, rows, docs]);
  const noteLines =
    notes.length > 0 ? (
      <div id="search-notes" className={cn(HEADER, "py-0.5")}>
        {notes.map((note) => (
          <div key={note}>{note}</div>
        ))}
      </div>
    ) : null;
  if (rows.length === 0 && docs.length === 0)
    return (
      <div className="flex min-h-0 flex-1 flex-col">
        {noteLines}
        {noMatches && <p className="p-3 text-muted-foreground">No matches.</p>}
      </div>
    );
  const select = (i: number) => cn("cursor-pointer select-none", i === active ? "bg-[var(--card-selected-bg)] text-[color:var(--list-active-fg)]" : "hover:bg-[var(--list-hover-bg)]");
  const dim = (i: number) => (i === active ? "opacity-80" : "text-muted-foreground");
  return (
    <div className="flex min-h-0 flex-1 flex-col">
      {noteLines}
      <div id="search-results" role="listbox" aria-label="Search results" className="min-h-0 flex-1 overflow-y-auto">
        {rows.length > 0 && (
          <div role="group" aria-label="Nodes">
            <div className={HEADER}>{nodesHeader(rows.length, total)}</div>
            {rows.map((r, i) => (
              <div
                key={r.id}
                id={resultId(i)}
                role="option"
                aria-selected={i === active}
                className={cn("flex h-[22px] items-center gap-1 px-2", select(i))}
                onMouseDown={(e) => e.preventDefault()} // the box keeps the keyboard
                onClick={() => onPick(i)}
              >
                <span className="flex size-4 flex-none items-center justify-center" style={{ color: KINDS[r.kind].color }}>
                  <Codicon name={kindIcon(r.kind, r.in_class)} className="text-[16px]" />
                </span>
                <span className="flex-none truncate">
                  <Marked pieces={splitMatch(r.name, r.match)} />
                </span>
                <span className={cn("min-w-0 flex-1 truncate text-[0.92em]", dim(i))}>{placeOf(r)}</span>
              </div>
            ))}
          </div>
        )}
        {docs.length > 0 && (
          <div role="group" aria-label="Docs">
            <div className={HEADER}>{docsHeader(docs.length, docsTotal)}</div>
            {docs.map((d, k) => {
              const i = rows.length + k;
              return (
                <div
                  key={d.doc_path}
                  id={resultId(i)}
                  role="option"
                  aria-selected={i === active}
                  data-missing={d.missing}
                  className={cn("flex flex-col justify-center px-2 py-0.5", select(i))}
                  onMouseDown={(e) => e.preventDefault()}
                  onClick={() => onPick(i)}
                >
                  <div className="flex h-[20px] items-center gap-1">
                    <span className="flex size-4 flex-none items-center justify-center">
                      <Codicon name="markdown" className="text-[16px]" />
                    </span>
                    <span className="min-w-0 flex-1 truncate">
                      <Marked pieces={splitMatch(d.title, d.title_match)} />
                      {tagsLabel(d.tags) && <span className={cn("pl-1.5 text-[0.88em]", dim(i))}>{tagsLabel(d.tags)}</span>}
                    </span>
                    <span className={cn("flex max-w-[45%] flex-none items-center gap-1 truncate text-[0.92em]", dim(i))}>
                      {d.missing && (
                        <span className="flex-none italic" title="Not in the code any more. Its docs are kept.">
                          Missing
                        </span>
                      )}
                      <span className="truncate">{d.node_name}</span>
                    </span>
                  </div>
                  <div className={cn("truncate pl-5 text-[0.88em]", dim(i))}>
                    <Marked pieces={splitMatch(d.snippet, d.snippet_match)} />
                  </div>
                </div>
              );
            })}
          </div>
        )}
      </div>
    </div>
  );
}
