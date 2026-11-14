import { useMemo } from "react";

import { TooltipProvider } from "@/components/ui/tooltip.tsx";
import { MAIN, openOf } from "@vnoc/shared/session.ts";
import { useData } from "../state/data.ts";
import { useSession } from "../state/session.ts";
import { flatten } from "./flatten.ts";
import { FocusBar } from "./FocusBar.tsx";
import { sidebarBody } from "./sidebarBody.ts";
import { States } from "./States.tsx";
import { Tree } from "./Tree.tsx";

/** Whether a folder is open: the host knew when it made the page (06 §8; a window that gets its first folder restarts the extension). */
const folder = document.getElementById("root")?.dataset.folder === "true";

/** A right-click on nothing shows nothing (no Cut / Copy / Paste); a row says its own (`Row`). */
const PAGE_CONTEXT = JSON.stringify({ preventDefaultContextMenuItems: true });

/**
 * The STRUCTURE view (docs/16-ui-redesign/06-sidebar.md): MAIN's panel, a focus bar while it has a focus, then its rows or one of the states. The other
 * contexts' panels are 8.6 and the search box is part 2. Reads the stores and draws; nothing here keeps state of its own.
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
  return (
    <TooltipProvider>
      <div className="flex h-screen flex-col bg-[color:var(--side-bar-bg)] text-[color:var(--side-bar-fg)]" data-vscode-context={PAGE_CONTEXT}>
        {body === "ready" && structure ? (
          <>
            {focusId !== null && <FocusBar structure={structure} focus={focus} />}
            <Tree rows={rows} selected={selected} cursor={cursor} />
          </>
        ) : (
          <States body={body === "ready" ? "loading" : body} message={message} />
        )}
      </div>
    </TooltipProvider>
  );
}
