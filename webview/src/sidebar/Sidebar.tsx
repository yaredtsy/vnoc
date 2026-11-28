import { useRef } from "react";

import { TooltipProvider } from "@/components/ui/tooltip.tsx";
import type { ContextId } from "@vnoc/shared/session.ts";
import { paneContext } from "../node-card/nodeContext.ts";
import { useData } from "../state/data.ts";
import { useSession } from "../state/session.ts";
import { isSearchKey } from "./keys.ts";
import { Panels } from "./Panels.tsx";
import { SearchBox } from "./SearchBox.tsx";
import { SearchResults, resultId } from "./SearchResults.tsx";
import { sidebarBody } from "./sidebarBody.ts";
import { States } from "./States.tsx";
import { ToursSection } from "./tours/ToursSection.tsx";
import { useSearch } from "./useSearch.ts";

const pageRoot = document.getElementById("root");
/** Whether a folder is open: the host knew when it made the page (06 §8; a window that gets its first folder restarts the extension). */
const folder = pageRoot?.dataset.folder === "true";
/** How long a row must be pointed at before it says what it is: the user's `workbench.hover.delay`, as it was when the page was made. */
const hoverDelay = Number(pageRoot?.dataset.hoverDelay) || 500;

/** A right-click on nothing is the sidebar's empty-space menu (08 §5: Collapse All, Clear Focus, Refresh Structure); a row or a header says its own. */
const PAGE_CONTEXT = paneContext("sidebar");

/**
 * The STRUCTURE view (docs/16-ui-redesign/06-sidebar.md): the search box, then one panel per context (each with its focus bar and its rows), or the results
 * while there is a query, or one of the states. Reads the stores and draws; nothing here keeps state of its own.
 */
export function Sidebar() {
  const { status, message, structure } = useData();
  const body = sidebarBody(folder, status, structure);
  // each panel's list, by context: where `Tab` goes, and where the keyboard goes when a search ends
  const lists = useRef(new Map<ContextId, HTMLDivElement>());
  const focusList = (id: ContextId) => lists.current.get(id)?.focus({ preventScroll: true });
  const search = useSearch(() => focusList(useSession.getState().active));
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
            activeId={search.searching && search.count > 0 ? resultId(search.active) : undefined}
            onClear={search.clear}
          />
        )}
        {body === "ready" && structure ? (
          search.searching ? (
            <SearchResults rows={search.rows} docs={search.docs} total={search.total} docsTotal={search.docsTotal} notes={search.notes} noMatches={search.noMatches} active={search.active} onPick={search.pickAt} />
          ) : (
            <Panels structure={structure} onList={(id, el) => (el ? lists.current.set(id, el) : lists.current.delete(id))} focusList={focusList} />
          )
        ) : (
          <States body={body === "ready" ? "loading" : body} message={message} />
        )}
        {/* the TOURS section: under the panels (and not under a search's results, which fill the view) */}
        {body === "ready" && structure && !search.searching && <ToursSection />}
      </div>
    </TooltipProvider>
  );
}
