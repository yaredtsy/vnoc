import { BarButton } from "@/components/BarButton.tsx";
import type { ContextId } from "@vnoc/shared/session.ts";
import { dispatch } from "@/state/session.ts";
import type { Crumb } from "./useCrumbs.ts";

/** One level up: the focus path loses its last step. The tooltip names where it goes. */
export function BackButton({ crumbs, context }: { crumbs: Crumb[]; context?: ContextId }) {
  const target = crumbs.length > 1 ? crumbs[crumbs.length - 2].node?.name : null;
  return (
    <BarButton
      icon="arrow-left"
      label={crumbs.length === 0 ? "Back" : `Back to ${target ?? "all code"}`}
      keys="Alt+←"
      disabled={crumbs.length === 0}
      onClick={() => dispatch({ type: "focusBack" }, context)}
    />
  );
}
