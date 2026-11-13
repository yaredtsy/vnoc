import { useEffect } from "react";

import type { TreeNode } from "@vnoc/shared/messages.ts";
import { canvasKey } from "@/canvas/keys.ts";
import type { Layout } from "@/canvas/layout.ts";
import { dispatch } from "@/state/session.ts";

/**
 * The keys on the canvas (docs/10-ui-rework/06-how-it-looks.md §7). Alt+← goes back one focus, like the ← button; both stay out of a text field
 * and a code card (Option+← is "word left" on macOS), and the other keys leave a focused button alone (Space and Enter are the button's).
 */
export function useCanvasKeys(laid: Layout | null, root: TreeNode | null, selected: string | null): void {
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const target = e.target instanceof Element ? e.target : null;
      if (target?.closest("input, textarea, select, [contenteditable], .monaco-editor")) return;
      if (e.altKey && e.key === "ArrowLeft") return void dispatch({ type: "focusBack" });
      if (e.altKey || e.ctrlKey || e.metaKey || target?.closest("button, a")) return;
      const intent = laid && root ? canvasKey(e.key, laid, root, selected) : null;
      if (!intent) return;
      e.preventDefault();
      dispatch(intent);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [laid, root, selected]);
}
