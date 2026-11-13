import { IconChevronRight } from "@tabler/icons-react";

import { dispatch } from "@/state/session.ts";
import { setHover } from "@/state/view.ts";
import { CrumbButton } from "./CrumbButton.tsx";
import { FoldedCrumbs, type Step } from "./FoldedCrumbs.tsx";
import type { Crumb } from "./useCrumbs.ts";

/** Below this width the path folds from the left (docs/16-ui-redesign/02-top-bar.md §5). */
export const NARROW = 520;
const KEPT = 2;

/**
 * Where the canvas is: the project, then one crumb per focus step. The last one is bold and does nothing; any other jumps up to it, and the
 * pointer over it points at its card. When narrow, the earlier steps fold into `… ›` and the last two stay.
 */
export function Crumbs({ project, stats, crumbs, narrow }: { project: string; stats: string; crumbs: Crumb[]; narrow: boolean }) {
  const steps: (Step & { title?: string; hover?: string })[] = [
    { id: "project", label: project, depth: 0, title: stats },
    ...crumbs.map((c, i) => ({ id: c.id, label: c.node?.name ?? "?", depth: i + 1, hover: c.id })),
  ];
  const folded = narrow ? steps.slice(0, Math.max(0, steps.length - KEPT)) : [];
  const shown = steps.slice(folded.length);
  const jump = (depth: number) => dispatch({ type: "focusJump", depth });
  return (
    <nav aria-label="Focus" className="flex min-w-0 flex-1 items-center overflow-hidden text-xs">
      {folded.length > 0 && <FoldedCrumbs steps={folded} onJump={jump} />}
      {shown.map((s, i) => (
        <span key={s.id} className="flex items-center">
          {i > 0 && <IconChevronRight className="size-3 flex-none opacity-50" />}
          <CrumbButton label={s.label} title={s.title} current={s.depth === crumbs.length} onHover={s.hover ? (on) => setHover(on ? s.hover! : null) : undefined} onClick={() => jump(s.depth)} />
        </span>
      ))}
    </nav>
  );
}
