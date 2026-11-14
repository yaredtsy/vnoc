import { Codicon } from "@/components/Codicon.tsx";
import type { ContextId } from "@vnoc/shared/session.ts";
import { dispatch } from "@/state/session.ts";
import { setHover } from "@/state/view.ts";
import { CrumbButton } from "./CrumbButton.tsx";
import { FoldedCrumbs } from "./FoldedCrumbs.tsx";
import { type Step, foldCrumbs } from "./foldCrumbs.ts";
import type { Crumb } from "./useCrumbs.ts";

/**
 * Where the canvas is: the project, then one crumb per focus step. The last one is bold and does nothing; any other jumps up to it, and the
 * pointer over it points at its card. When narrow (`foldCrumbs`), the earlier steps fold into `… ›`. `context`: the one it changes (the active one when not
 * given: the canvas's; the sidebar's panel gives its own).
 */
export function Crumbs({ project, stats, crumbs, width, context }: { project: string; stats: string; crumbs: Crumb[]; width: number; context?: ContextId }) {
  const steps: (Step & { title?: string; hover?: string })[] = [
    { id: "project", label: project, depth: 0, title: stats },
    ...crumbs.map((c, i) => ({ id: c.id, label: c.node?.name ?? "?", depth: i + 1, hover: c.id })),
  ];
  const { folded, shown } = foldCrumbs(steps, width);
  const jump = (depth: number) => dispatch({ type: "focusJump", depth }, context);
  return (
    <nav aria-label="Focus" className="flex min-w-0 flex-1 items-center overflow-hidden">
      {folded.length > 0 && <FoldedCrumbs steps={folded} onJump={jump} />}
      {shown.map((s, i) => (
        <span key={s.id} className="flex items-center">
          {i > 0 && <Codicon name="chevron-right" className="flex-none text-[16px] text-muted-foreground" />}
          <CrumbButton label={s.label} title={s.title} current={s.depth === crumbs.length} onHover={s.hover ? (on) => setHover(on ? s.hover! : null) : undefined} onClick={() => jump(s.depth)} />
        </span>
      ))}
    </nav>
  );
}
