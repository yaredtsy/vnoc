import { Codicon } from "@/components/Codicon.tsx";
import { cn } from "@/lib/utils.ts";
import type { Kind } from "@vnoc/shared/messages.ts";
import type { TourRow } from "@vnoc/shared/tours.ts";
import { KINDS } from "../../node-card/kinds.ts";
import { tourStopContext } from "../../node-card/nodeContext.ts";
import { useTour } from "../../tour/store.ts";
import { isOpen, listItem } from "./list.ts";
import type { RouteRow } from "./route.ts";

const kindOf = (kind: string | null): Kind => (kind && kind in KINDS ? (kind as Kind) : "function");
const MARKS = { seen: "✓", here: "●", ahead: "○", planned: "◌" } as const;

/** One tour's row in the list (design 07 §3): the kind's icon and the title, then the core's words and the question. 40 px. */
export function TourListRow({ row, domId, open, focused, line2, onOpen }: { row: TourRow; domId: string; open: string | null; focused: boolean; /** the row's second line in this view: its own words, or in the second view how it is on the node */ line2: string; onOpen: () => void }) {
  const item = listItem(row);
  const kind = KINDS[kindOf(item.kind)];
  return (
    <div
      role="option"
      id={domId}
      aria-selected={isOpen(row, open)}
      aria-disabled={!item.clickable}
      data-tour={item.id}
      className={cn(
        "flex h-10 flex-none flex-col justify-center border-l-2 pr-2 pl-2 text-[13px] leading-[18px]",
        isOpen(row, open) ? "border-l-[color:var(--ring)]" : "border-l-transparent",
        item.clickable ? "cursor-pointer hover:bg-[color:var(--list-hover-bg)]" : "opacity-60",
        focused && "group-focus-within:outline group-focus-within:outline-1 group-focus-within:-outline-offset-1 group-focus-within:outline-[color:var(--ring)]",
      )}
      title={item.question ?? undefined}
      onClick={() => item.clickable && onOpen()}
    >
      <div className="flex min-w-0 items-center gap-1.5">
        {item.kind !== null && (
          <span style={{ color: kind.color }} className="flex-none">
            <Codicon name={kind.icon} />
          </span>
        )}
        <span className={cn("min-w-0 truncate", isOpen(row, open) && "font-bold")}>{item.title}</span>
      </div>
      <div className="flex min-w-0 gap-1 pl-[20px] text-[11px] text-muted-foreground">
        <span className="flex-none">{line2}</span>
        {item.question !== null && <span className="min-w-0 truncate">· “{item.question}”</span>}
      </div>
    </div>
  );
}

/** One row of the route (design 04 §1.1, §1.2): a stop (mark, number, icon, name, one word) or a step of the stop you are in (dim, `▸` on the current one). */
export function RouteRowView({ row, domId, focused, onClick, onGoToCode }: { row: RouteRow; domId: string; focused: boolean; onClick: () => void; onGoToCode: () => void }) {
  if (row.type === "step") {
    return (
      <div
        role="option"
        id={domId}
        data-step={row.id}
        aria-current={row.current ? "step" : undefined}
        className={cn("flex h-5 flex-none cursor-pointer items-center truncate text-[12px] text-muted-foreground hover:bg-[color:var(--list-hover-bg)]", row.current && "text-foreground", focused && "group-focus-within:outline group-focus-within:outline-1 group-focus-within:-outline-offset-1 group-focus-within:outline-[color:var(--ring)]")}
        style={{ paddingLeft: 50 + row.depth * 12 }}
        onClick={onClick}
      >
        <span className="w-3 flex-none">{row.current ? "▸" : ""}</span>
        <span className="truncate">{row.text}</span>
      </div>
    );
  }
  const kind = KINDS[kindOf(row.kind)];
  return (
    <div
      role="option"
      id={domId}
      data-stop={row.id}
      data-vscode-context={tourStopContext({ tour: useTour.getState().id ?? "", stop: row.id, nodeKey: row.key, surface: "sidebar" })}
      aria-current={row.current ? "step" : undefined}
      className={cn("group/row flex h-[22px] flex-none cursor-pointer items-center gap-1.5 pr-2 text-[13px] hover:bg-[color:var(--list-hover-bg)]", focused && "group-focus-within:outline group-focus-within:outline-1 group-focus-within:-outline-offset-1 group-focus-within:outline-[color:var(--ring)]", row.mark === "planned" && "text-muted-foreground")}
      style={{ paddingLeft: 8 + row.depth * 12 }}
      onClick={onClick}
      onDoubleClick={onGoToCode}
    >
      <span className={cn("w-3 flex-none text-center text-[11px]", row.mark === "here" ? "text-[color:var(--ring)]" : "text-muted-foreground")}>{MARKS[row.mark]}</span>
      <span className="w-5 flex-none text-right text-[11px] text-muted-foreground">{row.number}</span>
      <span style={{ color: kind.color }} className="flex-none">
        <Codicon name={kind.icon} />
      </span>
      <span className={cn("min-w-0 truncate", row.current && "font-bold", row.struck && "line-through")}>{kind.title(row.name)}</span>
      {row.description && <span className="ml-auto flex-none pl-2 text-[11px] text-muted-foreground">{row.description}</span>}
      {/* the only hover button of a stop's row (design 04 §1.1): Go to Code */}
      <button
        type="button"
        title="Go to Code"
        aria-label={`Go to Code: ${row.name}`}
        className={cn("flex size-4 flex-none items-center justify-center rounded-sm opacity-0 group-hover/row:opacity-100 hover:bg-toolbar-hover focus-visible:opacity-100", !row.description && "ml-auto")}
        onClick={(e) => {
          e.stopPropagation();
          onGoToCode();
        }}
      >
        <Codicon name="link-external" className="text-[13px]" />
      </button>
    </div>
  );
}
