/** VS Code's thin progress line: 2 px, indeterminate, in the progress-bar colour. Put it in a `relative` parent; it sits on its bottom edge. */
export function ProgressLine({ label }: { label: string }) {
  return (
    <div role="progressbar" aria-label={label} className="pointer-events-none absolute inset-x-0 bottom-0 h-0.5 overflow-hidden">
      <div className="h-full w-1/4 bg-progress motion-safe:animate-[progress-slide_1.6s_linear_infinite]" />
    </div>
  );
}
