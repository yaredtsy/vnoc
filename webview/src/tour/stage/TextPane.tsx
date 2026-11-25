import type { StepView, TourStop, TourView } from "@vnoc/shared/tours.ts";
import type { Look } from "../layout.ts";
import { Rich } from "./Rich.tsx";

/**
 * The text pane (design 03 §2.3), before the foot: a detail's parent in one dim line, the title, the paragraph (the lead size for an intro), and what a step of this
 * kind adds under it: the grey bars of a stop that is not written, `⇢ Explained at stop N`, the mark of a step that changed. The words are the core's; the
 * sentences of a stop that is not written are the design's.
 */
export function TextPane(p: { view: TourView; stop: TourStop; step: StepView; parent: StepView | null; look: Look; onJump: (stop: string) => void }) {
  const { view, stop, step, look } = p;
  const writing = view.writing === stop.id;
  const on = view.writing === null ? null : view.stops.find((s) => s.id === view.writing);
  const explained = stop.same_as == null ? null : view.stops.find((s) => s.id === stop.same_as);
  return (
    <div className="stage-textpane" data-part="text">
      {look.parentLine && p.parent && (
        <button type="button" className="stage-parent" title="Go to the block this is a detail of" onClick={() => p.onJump(p.parent!.id)}>
          {p.parent.title} ›
        </button>
      )}
      {look.title && step.title && <h3 className="stage-title">{step.title}</h3>}
      {look.bars ? (
        <>
          <div className="stage-bars" aria-hidden>
            <i />
            <i />
            <i />
          </div>
          <p className="stage-sentence">
            {view.state === "stopped" || view.state === "stopping"
              ? "Not written: the tour was stopped here. Ask the agent to continue it."
              : writing
                ? "The agent is writing this stop. You can read the code meanwhile."
                : `Not written yet${on ? `: the agent is on stop ${on.number}.` : "."}`}
          </p>
        </>
      ) : (
        <p className={look.lead ? "stage-lead" : "stage-paragraph"}>
          <Rich text={step.text} />
        </p>
      )}
      {look.explained && explained && (
        <p className="stage-explained">
          ⇢ Explained at stop {explained.number}{" "}
          <button type="button" className="stage-link" onClick={() => p.onJump(explained.id)}>
            Go there
          </button>
        </p>
      )}
      {step.fresh === "changed" && <p className="stage-mark-note">The code changed after this was written. The lines may not match.</p>}
    </div>
  );
}
