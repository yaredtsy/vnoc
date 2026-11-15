// Reduce motion (docs/16-ui-redesign/09-states.md §2, §6): with `prefers-reduced-motion`, nothing pulses, a transition is instant, and a spinner is a static
// dot. CSS does most of it (`style.css`); what a script or a glyph decides is here, pure, so the rule is a test.

/** Whether the user asked for less motion (the OS's setting, as the browser reports it). */
export const prefersReducedMotion = (): boolean => typeof matchMedia === "function" && matchMedia("(prefers-reduced-motion: reduce)").matches;

/** The codicon a spinner draws: the turning one, or with reduced motion a dot that stands still. */
export const spinnerIcon = (reduced: boolean): { name: string; spin: boolean } => (reduced ? { name: "circle-filled", spin: false } : { name: "loading", spin: true });

/** How long a script-driven move (a fit of the view) lasts: none with reduced motion. */
export const motionMs = (ms: number, reduced: boolean): number => (reduced ? 0 : ms);
