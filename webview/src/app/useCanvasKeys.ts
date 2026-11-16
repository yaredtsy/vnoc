import { useEffect } from "react";

import type { TreeNode } from "@vnoc/shared/messages.ts";
import { FIT_VIEW_EVENT, canvasChord, canvasKey, canvasViewKey } from "@/canvas/keys.ts";
import type { Layout } from "@/canvas/layout.ts";
import { request } from "@/host.ts";
import { TEXT_FIELD } from "@/node-card/codeField.ts";
import { goToCallSite } from "@/node-card/goToSite.ts";
import { dispatch } from "@/state/session.ts";

/**
 * The keys on the canvas (docs/10-ui-rework/06-how-it-looks.md §7, 16/03 §7). Alt+← goes back one focus, like the ← button; both stay out of a text field
 * and a code card (Option+← is "word left" on macOS), and the other keys leave a focused button alone (Space and Enter are the button's).
 * Ctrl/Cmd+Enter is Go to Code on a structure card and Go to Call Site on a call card. `F` is Fit View, also with a button focused: it is the canvas, inside
 * React Flow, that moves the view, so the page tells it with an event.
 */
export function useCanvasKeys(laid: Layout | null, root: TreeNode | null, selected: string | null): void {
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const target = e.target instanceof Element ? e.target : null;
      const inField = !!target?.closest(TEXT_FIELD); // a code card's editor is one: its container is `nokey`, as React Flow wants
      if (inField) return;
      if (e.altKey && e.key === "ArrowLeft") return void dispatch({ type: "focusBack" });
      if ((e.ctrlKey || e.metaKey) && !e.altKey && !e.shiftKey && laid && !target?.closest("button, a")) {
        const chord = canvasChord(e.key, laid, selected);
        if (chord) {
          e.preventDefault();
          if (chord.type === "goToCode") void request("goToCode", { key: chord.key }).catch(() => {});
          else goToCallSite(chord.sites);
          return;
        }
      }
      const modified = e.altKey || e.ctrlKey || e.metaKey;
      // `F` is not a button's key: a chevron that was just clicked has the focus, and the key still fits the view (P5 of 18.04)
      if (laid && canvasViewKey(e.key, { inField, modified, onLink: !!target?.closest("a") }) === "fitView") {
        e.preventDefault();
        window.dispatchEvent(new Event(FIT_VIEW_EVENT));
        return;
      }
      if (modified || target?.closest("button, a")) return;
      const intent = laid && root ? canvasKey(e.key, laid, root, selected) : null;
      if (!intent) return;
      e.preventDefault();
      dispatch(intent);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [laid, root, selected]);
}
