import type { TourView } from "@vnoc/shared/tours.ts";
import { Codicon } from "@/components/Codicon.tsx";

/** `10 stops`, or `6 of 10 stops` while some are not written; `about 7 min`. The core's numbers, written out. */
const size = (v: TourView) => `${v.planned === 0 ? v.written : `${v.written} of ${v.written + v.planned}`} ${v.written + v.planned === 1 ? "stop" : "stops"} · about ${v.minutes} min`;

/** The cover (design 02 frame 2): one column. The title, the question if it has one, the size, what the agent is doing, Start. */
export function Cover(p: { view: TourView; onStart: () => void }) {
  const { view } = p;
  const writing = view.writing === null ? null : view.stops.find((s) => s.id === view.writing);
  return (
    <div className="stage-one" data-part="cover">
      <h3 className="stage-title">{view.title}</h3>
      {view.question && <p className="stage-lead">{view.question}</p>}
      <p className="stage-paragraph stage-dimtext">{size(view)}</p>
      {writing && (
        <p className="stage-paragraph stage-dimtext">
          <Codicon name="loading" className="text-[12px]" /> The agent is writing stop {writing.number} of {view.written + view.planned}…
        </p>
      )}
      {view.state === "stopped" && <p className="stage-paragraph stage-dimtext">The tour was stopped: {view.written} of {view.written + view.planned} stops are written.</p>}
      <div>
        <button type="button" className="stage-btn stage-primary" onClick={p.onStart}>
          Start ›
        </button>
      </div>
    </div>
  );
}

/** The end card (design 02 frame 9): the counts the core gave, and three buttons. */
export function End(p: { view: TourView; onBack: () => void; onAgain: () => void; onClose: () => void }) {
  const { view } = p;
  return (
    <div className="stage-one" data-part="end">
      <h3 className="stage-title">
        <Codicon name="check" /> End of the tour of {view.title}
      </h3>
      <p className="stage-paragraph stage-dimtext">
        {view.state === "stopped" ? `End of what was written: ${view.written} of ${view.written + view.planned} stops.` : `${view.written} ${view.written === 1 ? "stop" : "stops"} · ${view.steps} ${view.steps === 1 ? "step" : "steps"}`}
      </p>
      <div className="stage-buttons">
        <button type="button" className="stage-btn stage-secondary" onClick={p.onBack}>
          ‹ Back to the last step
        </button>
        <button type="button" className="stage-btn stage-secondary" onClick={p.onAgain}>
          Start Again
        </button>
        <button type="button" className="stage-btn stage-primary" onClick={p.onClose}>
          Close Tour
        </button>
      </div>
    </div>
  );
}
