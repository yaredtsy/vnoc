import { useMemo, useRef } from "react";

import { TooltipProvider } from "@/components/ui/tooltip.tsx";
import { MAIN, openOf } from "@vnoc/shared/session.ts";
import { useData } from "../state/data.ts";
import { useSession } from "../state/session.ts";
import { flatten } from "./flatten.ts";
import { FocusBar } from "./FocusBar.tsx";
import { isSearchKey } from "./keys.ts";
import { SearchBox } from "./SearchBox.tsx";
import { SearchResults, resultId } from "./SearchResults.tsx";
import { sidebarBody } from "./sidebarBody.ts";
import { States } from "./States.tsx";
import { Tree } from "./Tree.tsx";
import { useSearch } from "./useSearch.ts";

const pageRoot = document.getElementById("root");
/** Whether a folder is open: the host knew when it made the page (06 §8; a window that gets its first folder restarts the extension). */
const folder = pageRoot?.dataset.folder === "true";
/** How long a row must be pointed at before it says what it is: the user's `workbench.hover.delay`, as it was when the page was made. */
const hoverDelay = Number(pageRoot?.dataset.hoverDelay) || 500;

/** A right-click on nothing shows nothing (no Cut / Copy / Paste); a row says its own (`Row`). */
const PAGE_CONTEXT = JSON.stringify({ preventDefaultContextMenuItems: true });

/**
 * The STRUCTURE view (docs/16-ui-redesign/06-sidebar.md): the search box, MAIN's panel (a focus bar while it has a focus, then its rows), or the results
 * while there is a query, or one of the states. The other contexts' panels are 8.6. Reads the stores and draws; nothing here keeps state of its own.
 */
export function Sidebar() {
  const { status, message, structure } = useData();
  const focus = useSession((s) => s.contexts[MAIN].focus);
  const selected = useSession((s) => s.contexts[MAIN].selected);
  const open = useSession((s) => openOf(s.contexts[MAIN]));
  const cursor = useSession((s) => s.cursor);
  const body = sidebarBody(folder, status, structure);
  const focusId = focus.length > 0 ? focus[focus.length - 1] : null;
  const rows = useMemo(() => (structure ? flatten(structure.root, new Set(open), focusId) : []), [structure, open, focusId]);
  const list = useRef<HTMLDivElement>(null);
  const search = useSearch(structure?.root ?? null, list);
  const showBox = body === "ready" || body === "loading";
  return (
    <TooltipProvider delay={hoverDelay}>
      <div
        className="flex h-screen flex-col bg-[color:var(--side-bar-bg)] text-[color:var(--side-bar-fg)]"
        data-vscode-context={PAGE_CONTEXT}
        // `/` and Ctrl/Cmd+F from anywhere in the view but the box itself (where `/` is a character): the cursor goes into the box
        onKeyDown={(e) => {
          if (showBox && e.target !== search.input.current && isSearchKey(e)) {
            e.preventDefault();
            search.focusBox();
          }
        }}
      >
        {showBox && (
          <SearchBox
            value={search.query}
            onChange={search.setQuery}
            onKeyDown={search.onKeyDown}
            inputRef={search.input}
            disabled={body === "loading"}
            expanded={search.searching}
            activeId={search.searching && search.results.length > 0 ? resultId(search.active) : undefined}
            onClear={search.clear}
          />
        )}
        {body === "ready" && structure ? (
          search.searching ? (
            <SearchResults results={search.results} active={search.active} onPick={search.pick} />
          ) : (
            <>
              {focusId !== null && <FocusBar structure={structure} focus={focus} />}
              <Tree rows={rows} selected={selected} cursor={cursor} listRef={list} />
            </>
          )
        ) : (
          <States body={body === "ready" ? "loading" : body} message={message} />
        )}
      </div>
    </TooltipProvider>
  );
}
