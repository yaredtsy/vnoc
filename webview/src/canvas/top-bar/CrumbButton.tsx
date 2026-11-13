import { Button } from "@/components/ui/button.tsx";

/** One step of the path. The current one is bold and does nothing. */
export function CrumbButton({ label, title, current, onClick, onHover }: { label: string; title?: string; current: boolean; onClick: () => void; onHover?: (on: boolean) => void }) {
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
