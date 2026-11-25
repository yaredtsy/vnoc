import { type Beat, soFar } from "@vnoc/shared/tourWriting.ts";
import type { TourView } from "@vnoc/shared/tours.ts";
import { Codicon } from "@/components/Codicon.tsx";
import { coverNotes, pinNote } from "../written.ts";

/**
 * The size line (05 §3): `10 stops · about 7 min` when the size is known (a plan, a finished or a stopped tour); `6 stops so far · the agent is adding more` for a tour with no plan
 * that is being written. The core's numbers, written out.
 */
const size = (v: TourView) => (v.of !== null ? `${v.of} ${v.of === 1 ? "stop" : "stops"} · about ${v.minutes} min` : `${soFar(v)} · the agent is adding more`);

/** The cover (design 02 frame 2): one column. The title, the question if it has one, the size, what the agent is doing, Start. */
export function Cover(p: { view: TourView; beat: Beat; onStart: () => void }) {
  const { view } = p;
  const writing = view.writing === null || p.beat !== "writing" ? null : view.stops.find((s) => s.id === view.writing);
  const pin = pinNote(view);
  const notes = coverNotes(view);
  return (
    <div className="stage-one" data-part="cover">
      <h3 className="stage-title">{view.title}</h3>
      {view.question && <p className="stage-lead">{view.question}</p>}
      <p className="stage-paragraph stage-dimtext">{size(view)}</p>
      {pin && (
        <p className="stage-paragraph stage-dimtext" data-part="pin" title={pin.title}>
          {pin.text}
        </p>
      )}
      {notes.gone && (
        <p className="stage-paragraph" data-part="first-gone">
          {notes.gone}
        </p>
      )}
      {notes.most && (
        <p className="stage-paragraph" data-part="most-changed">
          {notes.most}
        </p>
      )}
      {writing && (
        <p className="stage-paragraph stage-dimtext">
          <Codicon name="loading" className="text-[12px]" /> The agent is writing stop {writing.number} of {view.of}…
        </p>
      )}
      {p.beat === "stopping" && <p className="stage-paragraph stage-dimtext">Stopping: the agent finishes the stop it is on.</p>}
      {p.beat === "quiet" && <p className="stage-paragraph stage-dimtext">No news from the agent. Look at its terminal, or ask it to continue the tour.</p>}
      {view.state === "stopped" && <p className="stage-paragraph stage-dimtext">The tour was stopped: {view.written} of {view.of} stops are written.</p>}
      <div>
        <button type="button" className="stage-btn stage-primary" onClick={p.onStart}>
          Start ›
        </button>
      </div>
    </div>
  );
}

/** The end card (design 02 frame 9): the counts the core gave, and three buttons. */
export function End(p: { view: TourView; onBack: () => void; onSkipped: () => void; onAgain: () => void; onClose: () => void }) {
  const { view } = p;
  return (
    <div className="stage-one" data-part="end">
      <h3 className="stage-title">
        <Codicon name="check" /> End of the tour of {view.title}
      </h3>
      <p className="stage-paragraph stage-dimtext">
        {view.state === "stopped" ? `End of what was written: ${view.written} of ${view.of} stops.` : `${view.written} ${view.written === 1 ? "stop" : "stops"} · ${view.steps} ${view.steps === 1 ? "step" : "steps"}`}
        {` · ${view.place.unseen === 0 ? "all seen" : `${view.place.unseen} not seen`}`}
      </p>
      <div className="stage-buttons">
        {/* the core says which stop was skipped first (`first_unseen`): with one, that is the first button (design 02 frame 9) */}
        {view.place.first_unseen !== null ? (
          <button type="button" className="stage-btn stage-secondary" onClick={p.onSkipped}>
            Go to the first one I skipped
          </button>
        ) : (
          <button type="button" className="stage-btn stage-secondary" onClick={p.onBack}>
            ‹ Back to the last step
          </button>
        )}
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
