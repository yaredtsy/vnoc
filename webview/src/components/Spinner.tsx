import { useSyncExternalStore } from "react";

import { Codicon } from "@/components/Codicon.tsx";
import { cn } from "@/lib/utils.ts";
import { prefersReducedMotion, spinnerIcon } from "@/lib/motion.ts";

const subscribe = (notify: () => void) => {
  const query = matchMedia("(prefers-reduced-motion: reduce)");
  query.addEventListener("change", notify);
  return () => query.removeEventListener("change", notify);
};

/** A spinner: it turns, or with *Reduce motion* it is a dot that stands still (09 §6). The only place a codicon spins. */
export function Spinner({ className }: { className?: string }) {
  const reduced = useSyncExternalStore(subscribe, prefersReducedMotion);
  const { name, spin } = spinnerIcon(reduced);
  return <Codicon name={name} className={cn(spin && "codicon-modifier-spin", className)} />;
}
