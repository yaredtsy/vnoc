import type { TourView } from "@vnoc/shared/tours.ts";
import { nextLabel, segmentsOf } from "../layout.ts";

/** The foot (design 03 §2.4): one segment per stop (filled: seen or here; dotted: not written; a click jumps to the stop), and Prev and Next. */
export function Foot(p: { view: TourView; onJump: (stop: string) => void; onMove: (move: "next" | "prev") => void }) {
  const label = nextLabel(p.view);
  return (
    <div className="stage-foot" data-part="foot">
      <div className="stage-segments">
        {segmentsOf(p.view).map((s) => (
          <button key={s.stop} type="button" className={`stage-seg${s.filled ? " stage-filled" : ""}${s.planned ? " stage-planned" : ""}${s.current ? " stage-here" : ""}`} title={s.name} aria-label={`Stop ${s.stop}: ${s.name}`} onClick={() => p.onJump(s.stop)} />
        ))}
      </div>
      <button type="button" className="stage-btn stage-secondary" disabled={p.view.place.at === "cover"} onClick={() => p.onMove("prev")}>
        ‹ Prev
      </button>
      <button type="button" className="stage-btn stage-primary" onClick={() => p.onMove("next")}>
        {label} ›
      </button>
    </div>
  );
}
