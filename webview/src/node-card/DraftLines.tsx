// The one-line marks above a draft's code: what happened, and the buttons for it (docs/17-edit-in-canvas.md §4, §7). A card shows them above its code, and the doc panel above its editor (§8),
// so they are drawn here once. `pad` is the left inset: a card's is where its names start; the panel's is its own body padding.

import type { ReactNode } from "react";

import { Button } from "@/components/ui/button.tsx";
import { META_H } from "../canvas/layout.ts";
import { ACTION_TEXT, type DraftAction, type Notice } from "../state/draftRules.ts";
import { NAME_X } from "./rows.ts";

const DOT_WARNING = "var(--vscode-editorWarning-foreground)";

/** The frame of every line: a dot in `dot`'s colour, then `children`, at the meta line's height. */
export function DraftLine({ pad = NAME_X, dot = DOT_WARNING, title, children, rightPad = "pr-2" }: { pad?: number; dot?: string; title?: string; children: ReactNode; rightPad?: string }) {
  return (
    <div className={`flex flex-none items-center gap-1.5 ${rightPad} font-mono text-[11px]`} style={{ height: META_H, paddingLeft: pad, color: "var(--vscode-descriptionForeground)" }} title={title}>
      <span className="h-1.5 w-1.5 flex-none rounded-full" style={{ background: dot }} />
      {children}
    </div>
  );
}

/** Edit was refused because another card holds the node's draft, or another page the doc's (§7, §8). */
export function Elsewhere({ text, tip, pad }: { text: string; tip: string; pad?: number }) {
  return (
    <DraftLine pad={pad} rightPad="pr-3" title={tip}>
      <span className="min-w-0 flex-1 truncate">{text}</span>
    </DraftLine>
  );
}

/** A draft in conflict or gone (§4): what happened, and the buttons for it. */
export function StateLine({ text, actions, act, pad }: { text: string; actions: DraftAction[]; act: Record<DraftAction, () => void>; pad?: number }) {
  return (
    <DraftLine pad={pad}>
      <span className="min-w-0 flex-1 truncate" title={text}>
        {text}
      </span>
      {actions.map((a) => (
        <Button key={a} variant="secondary" size="xs" className="nodrag nopan h-5 flex-none" title={ACTION_TEXT[a].tip} onClick={act[a]}>
          {ACTION_TEXT[a].label}
        </Button>
      ))}
    </DraftLine>
  );
}

/** What the last Save did (9.3b): written but not saved, or the reason a save failed. */
export function SaveNotice({ notice, pad }: { notice: Notice; pad?: number }) {
  return (
    <DraftLine pad={pad} rightPad="pr-3" dot={notice.kind === "error" ? "var(--vscode-editorError-foreground)" : DOT_WARNING} title={notice.text}>
      <span className="min-w-0 flex-1 truncate">{notice.text}</span>
    </DraftLine>
  );
}

/** E5: Edit was refused because the file has unsaved changes. `tip` says why, for a card (its lines come from the last save) or a doc. */
export function Refusal({ path, tip, onSaveAndEdit, pad }: { path: string; tip: (file: string) => string; onSaveAndEdit: () => void; pad?: number }) {
  const file = path.slice(path.lastIndexOf("/") + 1);
  return (
    <DraftLine pad={pad}>
      <span className="min-w-0 flex-1 truncate" title={tip(file)}>
        Save {file} first
      </span>
      <Button variant="secondary" size="xs" className="nodrag nopan h-5 flex-none" onClick={onSaveAndEdit}>
        Save and Edit
      </Button>
    </DraftLine>
  );
}
