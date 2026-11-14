import { BarButton } from "@/components/BarButton.tsx";
import { dispatch } from "@/state/session.ts";
import type { Crumb } from "./useCrumbs.ts";

/** One level up: the focus path loses its last step. The tooltip names where it goes. */
export function BackButton({ crumbs }: { crumbs: Crumb[] }) {
  const target = crumbs.length > 1 ? crumbs[crumbs.length - 2].node?.name : null;
  return (
    <BarButton
      icon="arrow-left"
      label={crumbs.length === 0 ? "Back" : `Back to ${target ?? "all code"}`}
      keys="Alt+←"
      disabled={crumbs.length === 0}
      onClick={() => dispatch({ type: "focusBack" })}
    />
  );
}
