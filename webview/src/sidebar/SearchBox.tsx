import type { RefObject } from "react";

import { Codicon } from "@/components/Codicon.tsx";

/** The box at the top of the view (06 §5). The focus stays in it while the results are shown: they are its `aria-activedescendant`. Draws; `useSearch` decides. */
export function SearchBox({ value, onChange, onKeyDown, inputRef, disabled, expanded, activeId, onClear }: {
  value: string;
  onChange: (value: string) => void;
  onKeyDown: (e: React.KeyboardEvent) => void;
  inputRef: RefObject<HTMLInputElement | null>;
  disabled: boolean;
  /** the results are shown */
  expanded: boolean;
  /** the id of the active result's element */
  activeId: string | undefined;
  onClear: () => void;
}) {
  return (
    <div className="flex-none p-1.5">
      <div className="relative">
        <Codicon name="search" className="pointer-events-none absolute top-1/2 left-1.5 -translate-y-1/2 text-[14px] text-[color:var(--input-placeholder)]" />
        <input
          ref={inputRef}
          type="text"
          role="combobox"
          aria-label="Search nodes"
          aria-expanded={expanded}
          aria-controls="search-results"
          aria-activedescendant={activeId}
          aria-autocomplete="list"
          spellCheck={false}
          autoComplete="off"
          disabled={disabled}
          placeholder="Search nodes…"
          value={value}
          onChange={(e) => onChange(e.target.value)}
          onKeyDown={onKeyDown}
          className="h-6 w-full rounded-[3px] border border-[color:var(--input-border)] bg-[color:var(--input-bg)] pr-6 pl-6 text-[length:var(--font-size)] text-[color:var(--input-fg)] outline-none placeholder:text-[color:var(--input-placeholder)] focus:border-[color:var(--ring)] disabled:opacity-60"
        />
        {value !== "" && (
          <button type="button" tabIndex={-1} aria-label="Clear search" title="Clear (Esc)" className="absolute top-1/2 right-1 flex size-4 -translate-y-1/2 items-center justify-center rounded-sm text-[color:var(--input-placeholder)] hover:text-[color:var(--input-fg)]" onClick={onClear}>
            <Codicon name="close" className="text-[14px]" />
          </button>
        )}
      </div>
    </div>
  );
}
