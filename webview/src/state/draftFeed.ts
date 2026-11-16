// The page's side of the host's copy of the drafts (docs/17-edit-in-canvas.md E6, §7, 9.5c). The canvas page only: it tells the host its whole list of drafts as they change, and takes the
// host's copy when it starts with none of its own. What counts as a change, and what a start takes, are the rules in `draftRules.ts` and `shared/drafts.ts`.

import { adopt } from "@vnoc/shared/drafts.ts";

import { post, request } from "../host.ts";
import { feedFor } from "./draftRules.ts";
import { PAGE } from "./page.ts";
import { useDrafts } from "./drafts.ts";
import { revealCard } from "./edit.ts";
import { setCodeOpen, setDocsOpen } from "./view.ts";

/** Typing is sent this long after the last keystroke. */
export const TYPING_MS = 500;

let timer: ReturnType<typeof setTimeout> | undefined;
let pending = false;

/** Tells the host the whole list now. */
function send(): void {
  clearTimeout(timer);
  pending = false;
  post({ type: "drafts", page: PAGE, drafts: Object.values(useDrafts.getState().drafts) });
}

useDrafts.subscribe((state, before) => {
  const feed = feedFor(before.drafts, state.drafts);
  if (feed === "now") send();
  else if (feed === "later") {
    pending = true;
    clearTimeout(timer);
    timer = setTimeout(send, TYPING_MS);
  }
});

// A page that is hidden or going away sends what typing was still waiting to send. Best effort: a panel that is closed may not get to run these.
const flush = () => pending && send();
document.addEventListener("visibilitychange", () => document.visibilityState === "hidden" && flush());
window.addEventListener("pagehide", flush);

// Asked once when the page starts, as `dirtyFiles` is (an event sent before a new page listens is lost). Its own saved drafts win if it has any (a webview reload keeps them), and
// the host's copy is told so; else it takes the host's copy, each draft in its saved state. A draft the page takes shows its code, so the card says what it holds.
if (Object.keys(useDrafts.getState().drafts).length > 0) send();
else
  request("drafts", { page: PAGE }).then(
    ({ drafts }) => {
      const own = useDrafts.getState().drafts;
      const next = adopt(own, drafts);
      const taken = Object.values(next).filter((d) => !own[d.key]);
      if (taken.length === 0) return;
      for (const d of taken) {
        if (d.kind !== "doc") setCodeOpen(d.card, true);
        // a doc's draft shows in the panel, for the node it was started on: the canvas opens the panel and brings the node into view; the docs tab pins itself to it (`DocsApp`)
        else if (PAGE === "canvas") {
          setDocsOpen(true);
          revealCard(d.card);
        }
      }
      useDrafts.setState({ drafts: next });
    },
    () => {},
  );
