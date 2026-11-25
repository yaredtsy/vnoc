import type { Beat, NextState } from "@vnoc/shared/tourWriting.ts";
import type { StepView, TourStop, TourView } from "@vnoc/shared/tours.ts";
import { ADDING_LINE, type Look, notWrittenSentence } from "../layout.ts";
import { type Choice, type WrittenLook, changedSentence } from "../written.ts";
import { Rich, type TextDrawn } from "./Rich.tsx";

/**
 * The text pane (design 03 §2.3), before the foot: a detail's parent in one dim line, the title, the paragraph (the lead size for an intro), and what a step of this
 * kind adds under it: the grey bars of a stop that is not written, `⇢ Explained at stop N`, the mark of a step that changed. The words are the core's; the
 * sentences of a stop that is not written are the design's.
 */
export function TextPane(p: { view: TourView; stop: TourStop; step: StepView; parent: StepView | null; look: Look; beat: Beat; next: NextState; fade: boolean; written: WrittenLook; onChoose: (choice: Choice) => void; onJump: (stop: string) => void; onTextDrawn?: (d: TextDrawn) => void }) {
  const { view, stop, step, look } = p;
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
          <p className="stage-sentence" data-part="not-written">
            {notWrittenSentence(view, stop, p.beat)}
          </p>
        </>
      ) : (
        // the text of a step that was not written a moment ago fades in where the grey bars were
        <div className={p.fade ? "stage-fadein" : undefined} data-part="written" data-fade={p.fade}>
          <Rich text={step.text} className={look.lead ? "stage-lead" : "stage-paragraph"} onDrawn={p.onTextDrawn} />
        </div>
      )}
      {p.next === "adding" && (
        <p className="stage-sentence" data-part="adding">
          {ADDING_LINE}
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
      {p.written.look === "written" || p.written.look === "now" ? (
        // a step of a pinned tour whose code changed: what it says, and (not for a stop that is gone) the switch between the code as written and as it is now
        <div data-part="written-note">
          <p className="stage-mark-note">{changedSentence(p.written.look === "written" && p.written.gone)}</p>
          {(p.written.look === "now" || p.written.switch) && (
            <div className="stage-switch" role="group" aria-label="Show the code">
              <button type="button" aria-pressed={p.written.look === "now"} data-choice="now" onClick={() => p.onChoose("now")}>
                Now
              </button>
              <button type="button" aria-pressed={p.written.look === "written"} data-choice="written" onClick={() => p.onChoose("written")}>
                As Written
              </button>
            </div>
          )}
        </div>
      ) : (
        step.fresh === "changed" && p.written.look !== "asking" && <p className="stage-mark-note">The code changed after this was written. The lines may not match.</p>
      )}
    </div>
  );
}
