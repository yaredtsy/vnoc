import { Codicon } from "@/components/Codicon.tsx";
import { cn } from "@/lib/utils.ts";
import { panelContext } from "../node-card/nodeContext.ts";
import { dispatch } from "../state/session.ts";
import { activate } from "./actions.ts";
import type { Panel } from "./panels.ts";

/**
 * A panel's header, drawn when there are two or more contexts (06 §4): `MAIN`, or `◇ EXPLORE: reserve_stock · from checkout_order` with a ✕. Bold when the
 * panel is the active one, in italics while it is a preview (09 §4); the tooltip says what the call is. A click activates it; a right-click is the header's
 * menu (08 §5: Close, Close Others). Draws; decides nothing.
 */
export function PanelHeader({ panel }: { panel: Panel }) {
  return (
    <div className="flex h-[22px] flex-none items-center gap-1 bg-[color:var(--section-header-bg)] pr-1 pl-2 text-[11px] text-[color:var(--section-header-fg)]" data-vscode-context={panelContext(panel)}>
      <button type="button" className={cn("min-w-0 flex-1 truncate text-left", panel.active && "font-bold", panel.preview && "italic")} title={panel.tooltip ?? undefined} aria-current={panel.active ? "true" : undefined} onClick={() => activate(panel.id)}>
        {panel.label}
      </button>
      {panel.closable && (
        <button
          type="button"
          aria-label={`Close ${panel.label}`}
          title="Close"
          className="flex size-4 flex-none items-center justify-center rounded-sm hover:bg-toolbar-hover"
          onPointerDown={(e) => e.stopPropagation()} // closing it must not first make it the active one
          onClick={() => dispatch({ type: "closeContext", id: panel.id }, panel.id)}
        >
          <Codicon name="close" className="text-[14px]" />
        </button>
      )}
    </div>
  );
}
