import { Codicon } from "@/components/Codicon.tsx";
import { IconButton } from "@/components/IconButton.tsx";
import { cn } from "@/lib/utils.ts";
import type { Placed } from "../canvas/layout.ts";
import { discardDraft, saveDraft } from "../state/edit.ts";
import { type ButtonId, cardButtons } from "./cardButtons.ts";
import { platformNow, saveTip } from "./cardKeys.ts";
import { type CardEdit, NOT_EDITING } from "./editState.ts";
import { useNodeActions } from "./useNodeActions.ts";

const BUTTONS: Record<ButtonId, { icon: string; label: (codeOpen: boolean) => string }> = {
  focus: { icon: "target", label: () => "Focus" },
  explore: { icon: "compass", label: () => "Explore" },
  goToCode: { icon: "link-external", label: () => "Go to Code" },
  code: { icon: "code", label: (open) => (open ? "Hide Code in Card" : "Code in Card") },
  save: { icon: "save", label: () => saveTip(platformNow()) },
  discard: { icon: "discard", label: () => "Discard" },
  addDoc: { icon: "add", label: () => "Add Doc…" },
};

/**
 * The card's buttons, always shown (docs/16-ui-redesign/04-node-cards.md §3): which ones is `cardButtons`. Dim at rest, full colour on the selected
 * card. `nodrag nopan`: React Flow must not start a drag or a pan from a click on one. On a sidebar row (07 §3) the same buttons, without `</>`, and out of
 * the tab order: the row's keys do the same things. `before` runs first: a row's buttons make its panel's context the active one (06 §4), as every other
 * gesture in the panel does.
 */
export function ActionStrip({ node, contextId, focusRoot, selected, surface = "canvas", before, edit = NOT_EDITING }: { node: Pick<Placed, "id" | "kind" | "range" | "role" | "key" | "codeOpen" | "path">; contextId: string; focusRoot: boolean; selected: boolean; surface?: "canvas" | "sidebar"; before?: () => void; edit?: CardEdit }) {
  const actions = useNodeActions(node.id, contextId, node.key);
  const key = node.key;
  const run: Record<ButtonId, () => void> = {
    focus: actions.focus,
    explore: actions.explore,
    goToCode: actions.goToCode,
    code: actions.toggleCode,
    save: () => void (key !== null && saveDraft(key)),
    discard: () => key !== null && discardDraft(key),
    addDoc: actions.addDoc,
  };
  return (
    <div className={cn("flex flex-none items-center", selected ? "text-[color:var(--card-selected-fg)]" : "text-muted-foreground")}>
      {cardButtons(node, focusRoot, surface, edit).map(({ id, disabled }) => {
        const { icon, label } = BUTTONS[id];
        const pressed = id === "code" ? node.codeOpen : undefined;
        return (
          <IconButton key={id} className="nodrag nopan size-[22px] hover:text-foreground" label={label(node.codeOpen)} disabled={disabled} aria-pressed={pressed} variant={pressed ? "secondary" : "ghost"} tabIndex={surface === "sidebar" ? -1 : undefined} onClick={() => (before?.(), run[id]())}>
            <Codicon name={icon} />
          </IconButton>
        );
      })}
    </div>
  );
}
