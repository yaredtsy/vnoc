import { BarButton } from "@/components/BarButton.tsx";
import { Codicon } from "@/components/Codicon.tsx";
import { cn } from "@/lib/utils.ts";
import type { Kind } from "@vnoc/shared/messages.ts";
import type { TourRow } from "@vnoc/shared/tours.ts";
import { KINDS } from "../../node-card/kinds.ts";
import { tourStopContext } from "../../node-card/nodeContext.ts";
import { useTour } from "../../tour/store.ts";
import { isOpen, listItem } from "./list.ts";
import type { Tomb } from "./deleted.ts";
import { routeLabel } from "../../tour/labels.ts";
import { tourRowContext } from "./menu.ts";
import type { RouteRow } from "./route.ts";
import { STOP_PLAYING, STOP_PLAYING_HOVER, STOP_PLAYING_ICON, rowMarks } from "./writing.ts";

const kindOf = (kind: string | null): Kind => (kind && kind in KINDS ? (kind as Kind) : "function");
const MARKS = { seen: "✓", here: "●", ahead: "○", planned: "◌" } as const;

/** The live dot of a tour being written: it pulses (still under reduced motion). */
function LiveDot({ mode }: { mode: "pulse" | "still" }) {
  return <span data-part="live-dot" data-live={mode} aria-hidden="true" className={cn("inline-block size-[7px] flex-none rounded-full bg-[color:var(--ring)]", mode === "pulse" && "motion-safe:animate-pulse")} />;
}

/**
 * One tour's row in the list (design 07 §3): the kind's icon and the title, then the core's words and the question. 40 px. A tour being written has a live dot and the core's word at the right of line 1,
 * and **■ Stop** at the right of line 2, always shown; the open tour says `▶ playing` and has the **Stop Playing** icon, always shown. On hover, at the right of line 1: `🗑` Delete (not on a tour being written:
 * it is not deleted under the agent). A row that cannot be read is dim and has only the delete.
 */
export function TourListRow({ row, domId, open, focused, line2, stateWord, onOpen, onDelete, onStop, onStopPlaying }: { row: TourRow; domId: string; open: string | null; focused: boolean; /** the row's second line in this view: its own words, or in the second view how it is on the node */ line2: string; /** the state word at the right of line 1, in the second view */ stateWord: string | null; onOpen: () => void; onDelete: () => void; onStop: () => void; onStopPlaying: () => void }) {
  const item = listItem(row);
  const kind = KINDS[kindOf(item.kind)];
  // `■ Stop` only while the agent can hear it (`writing`); a tour that is `stopping` has been asked already, and its delete is the core's to refuse in words
  const marks = rowMarks(row, open);
  const stop = marks.stop;
  const hover = "absolute top-1 right-1 flex h-[18px] flex-none items-center justify-center rounded-sm bg-[color:var(--list-hover-bg)] px-1 text-[11px] text-foreground opacity-0 group-hover/row:opacity-100 hover:bg-toolbar-hover focus-visible:opacity-100";
  return (
    <div
      role="option"
      id={domId}
      aria-selected={isOpen(row, open)}
      aria-disabled={!item.clickable}
      data-tour={item.id}
      data-vscode-context={tourRowContext(row)}
      className={cn(
        "group/row relative flex h-10 flex-none flex-col justify-center border-l-2 pr-2 pl-2 text-[13px] leading-[18px]",
        isOpen(row, open) ? "border-l-[color:var(--ring)]" : "border-l-transparent",
        item.clickable ? "cursor-pointer hover:bg-[color:var(--list-hover-bg)]" : "opacity-60",
        focused && "group-focus-within:outline group-focus-within:outline-1 group-focus-within:-outline-offset-1 group-focus-within:outline-[color:var(--ring)]",
      )}
      title={item.question === null ? item.id : `${item.question}\n${item.id}`}
      onClick={() => item.clickable && onOpen()}
    >
      <div className="flex min-w-0 items-center gap-1.5">
        {item.kind !== null && (
          <span style={{ color: kind.color }} className="flex-none">
            <Codicon name={kind.icon} />
          </span>
        )}
        <span className={cn("min-w-0 truncate", isOpen(row, open) && "font-bold")}>{item.title}</span>
        {(marks.playing || marks.word !== null || stateWord !== null) && (
          <span data-part="row-state" className="ml-auto flex flex-none items-center gap-1.5 pl-2 text-[11px] text-muted-foreground">
            {marks.playing && <span data-part="row-playing">▶ playing</span>}
            {marks.word !== null ? (
              <span data-part="row-live" className="flex items-center gap-1">
                {marks.dot !== null && <LiveDot mode={marks.dot} />}
                {marks.word}
              </span>
            ) : (
              stateWord !== null && <span data-state-word>{stateWord}</span>
            )}
          </span>
        )}
      </div>
      <div className="flex min-w-0 items-center gap-1 pl-[20px] text-[11px] text-muted-foreground">
        <span className={cn("min-w-0 truncate", item.question === null && "flex-1")}>{line2}</span>
        {item.question !== null && <span className="min-w-0 flex-1 truncate">· “{item.question}”</span>}
        {(stop || marks.playing) && (
          <span className="ml-auto flex flex-none items-center gap-1 pl-1">
            {stop && (
              <button
                type="button"
                data-part="row-stop"
                title="Stop the agent: it finishes the stop it is on"
                aria-label={`Stop: ${item.title}`}
                className="flex h-4 items-center rounded-sm border border-border px-1 text-[11px] text-foreground hover:bg-toolbar-hover"
                onClick={(e) => {
                  e.stopPropagation();
                  onStop();
                }}
              >
                ■ Stop
              </button>
            )}
            {marks.playing && (
              <BarButton
                icon={STOP_PLAYING_ICON}
                label={STOP_PLAYING}
                data-part="row-stop-playing"
                className={cn("size-4", STOP_PLAYING_HOVER)}
                onClick={(e) => {
                  e.stopPropagation();
                  onStopPlaying();
                }}
              />
            )}
          </span>
        )}
      </div>
      {!stop && (
        <button
          type="button"
          data-part="row-delete"
          title="Delete"
          aria-label={`Delete: ${item.title}`}
          className={cn(hover, "w-[18px] px-0")}
          onClick={(e) => {
            e.stopPropagation();
            onDelete();
          }}
        >
          <Codicon name="trash" className="text-[13px]" />
        </button>
      )}
    </div>
  );
}

/** A tour that was deleted (design 07 §7): the core's words and **Undo**, in the row's place, for six seconds. No dialog. */
export function DeletedRow({ tomb, domId, focused, onUndo }: { tomb: Tomb; domId: string; focused: boolean; onUndo: () => void }) {
  return (
    <div
      role="option"
      id={domId}
      data-deleted={tomb.ids.join(",")}
      className={cn("flex h-10 flex-none items-center gap-2 border-l-2 border-l-transparent pr-2 pl-2 text-[12px] text-muted-foreground", focused && "group-focus-within:outline group-focus-within:outline-1 group-focus-within:-outline-offset-1 group-focus-within:outline-[color:var(--ring)]")}
    >
      <span data-part="deleted-words" className="min-w-0 flex-1 truncate" title={tomb.words}>
        {tomb.words}
      </span>
      <button type="button" data-part="undo" className="flex-none rounded-sm px-1 text-foreground underline hover:bg-toolbar-hover" onClick={onUndo}>
        Undo
      </button>
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
        aria-label={routeLabel(row)}
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
      aria-label={routeLabel(row)}
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
