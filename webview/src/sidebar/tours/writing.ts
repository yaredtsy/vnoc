// What the TOURS section says of a tour being written and of the tour being played (docs/26-tours/05-on-screen.md §5, "The user's by-hand pass on the 0.4.0 VSIX"). Pure: every word that
// says what a tour is doing is the core's (`state`, `state_word`, the route header's `agentWords`); what is made here is which marks go where. Two words stay apart: **Stop** stops the agent that
// writes a tour, **Stop Playing** closes the tour that is open (the place is kept).

import type { TourRow } from "@vnoc/shared/tours.ts";

/** What **Stop Playing** says, as a tooltip and an accessible name: it is an icon button everywhere (the row, the strip, the route header). */
export const STOP_PLAYING = "Stop Playing: the place is kept";

/** A row whose tour is being written now (the core's `state`). */
const isWriting = (row: TourRow): boolean => row.readable && row.state === "writing";

/** What a row says at the right of line 1 when its tour is being written, has been told to stop or has stopped: a live dot while it writes, and the core's own word. */
export interface RowMarks {
  /** `pulse` while the agent writes (static under reduced motion, in CSS), `still` after Stop was pressed, none for a tour that stopped */
  dot: "pulse" | "still" | null;
  /** the core's `state_word`: `writing`, `no news`, `stopping`, `stopped`; null when the tour is none of those */
  word: string | null;
  /** the row's **Stop**, always shown: only while the agent can hear it */
  stop: boolean;
  /** the row is the open tour: `▶ playing`, and its **Stop Playing**, always shown */
  playing: boolean;
}

export function rowMarks(row: TourRow, open: string | null): RowMarks {
  const live = row.readable && (row.state === "writing" || row.state === "stopping" || row.state === "stopped");
  return {
    dot: !live ? null : row.state === "writing" ? (row.quiet ? "still" : "pulse") : row.state === "stopping" ? "still" : null,
    word: live && row.state_word ? row.state_word : null,
    stop: isWriting(row),
    playing: row.readable && open !== null && row.id === open && row.private_copy !== true,
  };
}

/** The line under the section's header: what is being written, and the open tour while the list is shown. Null when it has nothing to say. */
export interface Strip {
  /** `● writing`, or `● 2 writing` when more than one is */
  writing: string | null;
  /** the one tour's id, when exactly one is being written: its **Stop** */
  stop: string | null;
  /** `▶ <title>` of the open tour, with **Stop Playing**; only while the list is shown (the route has its own header) */
  playing: string | null;
}

export function stripOf(rows: readonly TourRow[], open: { id: string; title: string } | null, mode: "list" | "route"): Strip | null {
  const writing = rows.filter(isWriting);
  // in the route, the open tour's own header says that it is being written
  const covered = mode === "route" && writing.length === 1 && writing[0].id === open?.id;
  const shown = covered ? [] : writing;
  const strip: Strip = {
    writing: shown.length === 0 ? null : shown.length === 1 ? "● writing" : `● ${shown.length} writing`,
    stop: shown.length === 1 ? shown[0].id : null,
    playing: mode === "list" && open !== null ? `▶ ${open.title}` : null,
  };
  return strip.writing === null && strip.playing === null ? null : strip;
}
