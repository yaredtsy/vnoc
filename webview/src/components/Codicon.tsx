import { cn } from "@/lib/utils.ts";

/** One of VS Code's own icons (the font is loaded by main.tsx). `name` is the codicon's name: `symbol-function`, `target`, … */
export function Codicon({ name, className }: { name: string; className?: string }) {
  return <i aria-hidden className={cn("codicon", `codicon-${name}`, "text-[14px] leading-none", className)} />;
}
