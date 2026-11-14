import { Codicon } from "@/components/Codicon.tsx";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip.tsx";
import { cn } from "@/lib/utils.ts";
import { ActionStrip } from "../node-card/ActionStrip.tsx";
import { KINDS, kindIcon } from "../node-card/kinds.ts";
import { nodeContext } from "../node-card/nodeContext.ts";
import { activate } from "./actions.ts";
import type { FlatRow } from "./flatten.ts";
import { hoverParts } from "./hover.ts";
import { rowDescription, rowFacts, rowLabel } from "./rowFacts.ts";

/** One row's height, and what each level of depth indents it by (07 §1: 22 px; 16 px per level, 8 before the first, as the Explorer). */
export const ROW_H = 22;
const INDENT = 16;
const INDENT_FIRST = 8;

/**
 * One row of the structure (07 §1): chevron, the kind's codicon in its colour, the name, the dim description, the cursor dot, and over the end of the row
 * the same buttons as the card (07 §3) on the hovered and the selected row. The row's element carries `data-vscode-context`: a right-click gives VS Code the
 * object, which shows the entries of `webview/context` for `webviewId == 'vnoc.structure'` and passes it to the command (07 §6). After the hover delay the
 * row says what it is (07 §5). Draws; decides nothing.
 */
export function Row({ row, domId, start, contextId, selected, cursor, treeFocused, onClick, onDoubleClick, onToggle, onContextMenu, onHover }: {
  row: FlatRow;
  /** the row's element id: unique across the panels (the list's `aria-activedescendant` names it) */
  domId: string;
  /** the row's top, in the list */
  start: number;
  contextId: string;
  selected: boolean;
  /** the text cursor is in this node */
  cursor: boolean;
  /** the list has the keyboard: the selection is bright, else dim */
  treeFocused: boolean;
  onClick: () => void;
  onDoubleClick: () => void;
  onToggle: () => void;
  onContextMenu: () => void;
  /** the pointer is over the row, or left it: the card on the canvas is pointed at (06 §6) */
  onHover: (on: boolean) => void;
}) {
  const { node } = row;
  const description = rowDescription(node);
  const hover = hoverParts(node);
  const line = (
    <div
      role="treeitem"
      id={domId}
      aria-level={row.depth + 1}
      aria-expanded={row.hasChildren ? row.open : undefined}
      aria-selected={selected}
      aria-setsize={row.setSize}
      aria-posinset={row.posInSet}
      aria-label={rowLabel(node)}
      data-vscode-context={nodeContext({ node: rowFacts(row), contextId, isFocusRoot: row.focusRoot, surface: "sidebar" })}
      className={cn(
        "group absolute inset-x-0 flex cursor-pointer items-center pr-2 select-none",
        selected
          ? treeFocused
            ? "bg-[var(--card-selected-bg)] text-[color:var(--list-active-fg)] [--row-bg:var(--card-selected-bg)]"
            : "bg-[var(--list-inactive-bg)] text-[color:var(--list-inactive-fg)] [--row-bg:var(--list-inactive-bg)]"
          : "hover:bg-[var(--list-hover-bg)] hover:[--row-bg:var(--list-hover-bg)]",
      )}
      style={{ top: start, height: ROW_H, paddingLeft: INDENT_FIRST + row.depth * INDENT }}
      onClick={(e) => !(e.target as Element).closest("button") && onClick()}
      onDoubleClick={(e) => !(e.target as Element).closest("button") && onDoubleClick()}
      onContextMenu={onContextMenu}
      onMouseEnter={() => onHover(true)}
      onMouseLeave={() => onHover(false)}
    />
  );
  return (
    <Tooltip>
      <TooltipTrigger render={line}>
        {Array.from({ length: row.depth }, (_, i) => (
          <span key={i} aria-hidden className="pointer-events-none absolute inset-y-0 w-px bg-[var(--indent-guide)] opacity-0 transition-opacity group-hover/tree:opacity-60" style={{ left: INDENT_FIRST + i * INDENT + 7 }} />
        ))}
        {row.hasChildren ? (
          <button
            type="button"
            tabIndex={-1}
            aria-hidden
            className="flex size-4 flex-none items-center justify-center rounded-sm hover:bg-toolbar-hover"
            onClick={(e) => {
              e.stopPropagation();
              if (!row.focusRoot) onToggle();
            }}
          >
            <Codicon name="chevron-right" className={cn("text-[16px] transition-transform", row.open && "rotate-90")} />
          </button>
        ) : (
          <span className="size-4 flex-none" />
        )}
        <span className="mx-1 flex size-4 flex-none items-center justify-center" style={{ color: KINDS[node.kind].color }}>
          <Codicon name={kindIcon(node.kind, row.inClass)} className="text-[16px]" />
        </span>
        <span className="min-w-0 truncate">{node.name}</span>
        {description && <span className={cn("ml-2 flex-none text-[0.92em]", selected ? "opacity-80" : "text-muted-foreground")}>{description}</span>}
        {cursor && (
          <span className="ml-auto flex-none pl-2 text-[color:var(--card-cursor)]" title="the cursor is here">
            ●
          </span>
        )}
        <div className="absolute inset-y-0 right-0 hidden items-center bg-[var(--row-bg)] pl-1 group-hover:flex data-[on=true]:flex" data-on={selected}>
          <ActionStrip node={{ id: row.id, key: row.id, role: "node", kind: node.kind, range: node.range, codeOpen: false }} contextId={contextId} focusRoot={row.focusRoot} selected={selected} surface="sidebar" before={() => activate(contextId)} />
        </div>
      </TooltipTrigger>
      <TooltipContent side="right" align="start" arrow={false} className="border border-[color:var(--hover-border)] bg-[color:var(--hover-bg)] text-[length:var(--font-size)] text-[color:var(--hover-fg)]">
        <span>
          <b>{hover.name}</b> · {hover.kind} · <span className="font-[family-name:var(--editor-font)]">{hover.where}</span>
          {hover.docs && <> · {hover.docs}</>}
          {hover.link && (
            <>
              {" "}
              · <i>{hover.link}</i>
            </>
          )}
        </span>
      </TooltipContent>
    </Tooltip>
  );
}
