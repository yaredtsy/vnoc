import { Button } from "@/components/ui/button.tsx";

/**
 * One step of the path, at the editor breadcrumbs' size: 13 px, dim; the current one is foreground and bold and does nothing. Not the kit's `xs` preset
 * (12 px): the sizes are overridden on the default one.
 */
export function CrumbButton({ label, title, current, onClick, onHover }: { label: string; title?: string; current: boolean; onClick: () => void; onHover?: (on: boolean) => void }) {
  return (
    <Button
      variant="ghost"
      className="h-[22px] max-w-40 flex-none rounded-[5px] px-1.5 text-[length:var(--font-size)] font-normal text-muted-foreground hover:bg-toolbar-hover hover:text-foreground disabled:opacity-100 data-[current=true]:font-semibold data-[current=true]:text-foreground"
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
