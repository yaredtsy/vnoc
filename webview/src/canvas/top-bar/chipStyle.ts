// The look of a chip on the bar (docs/16-ui-redesign/02-top-bar.md): the context chips and the unsaved-drafts chip share it.

export const CHIP = "flex h-[22px] flex-none items-center gap-1 rounded-[5px] border px-1.5 text-[length:var(--font-size)]";
export const filled = "border-transparent bg-chip-active text-chip-active-foreground";
export const outlined = "border-border text-muted-foreground hover:bg-toolbar-hover hover:text-foreground";
