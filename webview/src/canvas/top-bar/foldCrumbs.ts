// How the path folds on a narrow canvas (docs/16-ui-redesign/02-top-bar.md §5). Pure, so the rule is tested; Crumbs only draws it.

/** Below this width the path folds from the left. */
export const NARROW = 520;
/** How many steps stay drawn when it folds (the last two: the current one and the one before it). */
export const KEPT = 2;

export interface Step {
  id: string;
  label: string;
  /** how many focus steps to keep when jumping to it (0 = the project) */
  depth: number;
}

/**
 * `steps` is the project, then one per focus step. At or above NARROW everything is shown. Below it the earliest steps fold (the project first)
 * and the last KEPT stay, so the current crumb, which is the last, is never folded. A folded step jumps to its own `depth`.
 */
export function foldCrumbs<T extends Step>(steps: readonly T[], width: number): { folded: T[]; shown: T[] } {
  const cut = width < NARROW ? Math.max(0, steps.length - KEPT) : 0;
  return { folded: steps.slice(0, cut), shown: steps.slice(cut) };
}
