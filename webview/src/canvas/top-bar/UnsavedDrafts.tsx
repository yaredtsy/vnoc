import { useEffect, useMemo } from "react";

import { post } from "@/host.ts";
import { cn } from "@/lib/utils.ts";
import { useDrafts } from "@/state/drafts.ts";
import { revealNextDraft } from "@/state/edit.ts";
import { CHIP, outlined } from "./chipStyle.ts";
import { unsavedDrafts } from "./unsavedDrafts.ts";

/**
 * "1 unsaved draft" / "N unsaved drafts" (docs/17-edit-in-canvas.md §7): drafts live in the page, so the bar says there is something to lose. A click reveals the next
 * draft's card. Draws only; the count, the label and the order are `unsavedDrafts`.
 */
export function UnsavedDrafts() {
  const drafts = useDrafts((s) => s.drafts);
  const found = useMemo(() => unsavedDrafts(drafts), [drafts]);
  const count = found?.count ?? 0;
  const label = found?.label ?? "";
  // what the bar says, for the extension's test suite
  useEffect(() => post({ type: "draftsDrawn", count, label }), [count, label]);
  if (!found) return null;
  return (
    <button type="button" className={cn(CHIP, outlined, "outline-none")} title="Show the next draft that is not saved" onClick={revealNextDraft}>
      <span className="h-1.5 w-1.5 flex-none rounded-full" style={{ background: "var(--vscode-editorWarning-foreground)" }} />
      <span className="whitespace-nowrap">{found.label}</span>
    </button>
  );
}
