import type { NextState } from "@vnoc/shared/tourWriting.ts";
import type { TourView } from "@vnoc/shared/tours.ts";
import { nextLabel, segmentsOf } from "../layout.ts";

/**
 * The foot (design 03 §2.4): one segment per stop (filled: seen or here; dotted: not written; a click jumps to the stop), and Prev and Next. `Next` is dim with a `◌` when the next
 * step is a stop the agent has not written (pressing it still goes there, and the stage says the stop is being written) or has not added yet (a tour with no plan: there is
 * nowhere to go, so it does nothing).
 */
export function Foot(p: { view: TourView; next: NextState; onJump: (stop: string) => void; onMove: (move: "next" | "prev") => void }) {
  const label = nextLabel(p.view, p.next);
  const dim = p.next !== "ready";
  return (
    <div className="stage-foot" data-part="foot">
      <div className="stage-segments">
        {segmentsOf(p.view).map((s) =>
          s.adding ? (
            <span key="adding" className="stage-seg stage-planned stage-adding" title={s.name} data-segment="adding" />
          ) : (
            <button key={s.stop} type="button" className={`stage-seg${s.filled ? " stage-filled" : ""}${s.planned ? " stage-planned" : ""}${s.current ? " stage-here" : ""}`} title={s.name} aria-label={`Stop ${s.stop}: ${s.name}`} onClick={() => p.onJump(s.stop)} />
          ),
        )}
      </div>
      <button type="button" className="stage-btn stage-secondary" disabled={p.view.place.at === "cover"} onClick={() => p.onMove("prev")}>
        ‹ Prev
      </button>
      <button
        type="button"
        className={`stage-btn stage-primary${dim ? " stage-dimmed" : ""}`}
        aria-disabled={p.next === "adding"}
        title={p.next === "adding" ? "The agent is adding the next stop" : p.next === "planned" ? "The next stop is not written yet: you can go there while the agent writes it" : undefined}
        onClick={() => p.next !== "adding" && p.onMove("next")}
      >
        {label} {dim ? "◌" : "›"}
      </button>
    </div>
  );
}
