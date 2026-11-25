import type { Kind } from "@vnoc/shared/messages.ts";
import type { TourStop } from "@vnoc/shared/tours.ts";
import { Codicon } from "@/components/Codicon.tsx";
import { KINDS } from "../../node-card/kinds.ts";

/** The header, 30 px (design 03 §2.1): the mark and number, the kind and name, where it lives, how the tour got here, the place, Go to Code and fold. */
export function Header(p: {
  stop: TourStop;
  where: string;
  position: string;
  onGoToCode: () => void;
  /** a stop that is not in the code any more has no `↗`: there is nowhere to go */
  canGoToCode?: boolean;
  onFold: () => void;
  /** a jump to the step of the stop this one was reached from that holds the line (`line_step`) */
  onCalledFrom: () => void;
}) {
  const { stop } = p;
  const kind = (stop.kind && stop.kind in KINDS ? stop.kind : "function") as Kind;
  return (
    <div className="stage-header" data-part="header">
      <span className="stage-mark" title={stop.planned ? "Not written yet" : "Written"}>
        <Codicon name={stop.planned ? "circle-large-outline" : "circle-filled"} className="text-[10px]" />
      </span>
      <span className="stage-number">{stop.number}</span>
      <span className="stage-kind" style={{ color: KINDS[kind].color }}>
        <Codicon name={KINDS[kind].icon} />
      </span>
      <span className="stage-name">{KINDS[kind].title(stop.name)}</span>
      <button type="button" className="stage-where" title="Go to Code" onClick={p.onGoToCode}>
        {p.where}
      </button>
      {stop.line != null && stop.from != null && (
        <button type="button" className="stage-called" title={`Go to the step of ${stop.from} that holds line ${stop.line}`} onClick={p.onCalledFrom}>
          <Codicon name="call-outgoing" className="text-[12px]" /> called on line {stop.line}
        </button>
      )}
      <span className="stage-grow" />
      <span className="stage-position">{p.position}</span>
      {p.canGoToCode !== false && (
        <button type="button" className="stage-icon" title="Go to Code (Ctrl+Enter)" aria-label="Go to Code" onClick={p.onGoToCode}>
          <Codicon name="link-external" />
        </button>
      )}
      <button type="button" className="stage-icon" title="Fold the stage" aria-label="Fold the stage" onClick={p.onFold}>
        <Codicon name="chevron-down" />
      </button>
    </div>
  );
}
