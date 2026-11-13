import { IconChevronRight } from "@tabler/icons-react";

import { Button } from "@/components/ui/button.tsx";
import { dispatch } from "@/state/session.ts";
import { setHover } from "@/state/view.ts";
import type { Crumb } from "./useCrumbs.ts";

/**
 * Where the canvas is: the project, then one crumb per focus step. The last one is bold and does nothing; any other jumps up to it, and the
 * pointer over it points at its card.
 */
export function Crumbs({ project, stats, crumbs }: { project: string; stats: string; crumbs: Crumb[] }) {
  return (
    <nav aria-label="Focus" className="flex min-w-0 flex-1 items-center overflow-hidden text-xs">
      <Crumb label={project} title={stats} current={crumbs.length === 0} onClick={() => dispatch({ type: "focusJump", depth: 0 })} />
      {crumbs.map((c, i) => (
        <span key={c.id} className="flex items-center">
          <IconChevronRight className="size-3 flex-none opacity-50" />
          <Crumb
            label={c.node?.name ?? "?"}
            current={i === crumbs.length - 1}
            onHover={(on) => setHover(on ? c.id : null)}
            onClick={() => dispatch({ type: "focusJump", depth: i + 1 })}
          />
        </span>
      ))}
    </nav>
  );
}

function Crumb({ label, title, current, onClick, onHover }: { label: string; title?: string; current: boolean; onClick: () => void; onHover?: (on: boolean) => void }) {
  return (
    <Button
      variant="ghost"
      size="xs"
      className="max-w-40 flex-none px-1.5 disabled:opacity-100 data-[current=true]:font-semibold"
      data-current={current}
      title={title}
      disabled={current}
      onMouseEnter={() => onHover?.(true)}
      onMouseLeave={() => onHover?.(false)}
      onClick={onClick}
    >
      <span className="truncate">{label}</span>
    </Button>
  );
}
