import { useEffect, useRef } from "react";

import { Codicon } from "@/components/Codicon.tsx";
import { type AskForm, DETAIL_CHOICES, REACH_CHOICES } from "./ask.ts";

/**
 * The prompt builder (docs/26-tours/05-on-screen.md §5, TR23a), at the top of the TOURS section, above the list: **About** (the node, a chip that can be removed), **Question**, **How far**, **Detail**,
 * the prompt as the core gave it, and **Copy Prompt**. Esc or **Cancel** closes it. It draws what it is given: the sentence is the core's, asked again a moment after the last change.
 */
export function AskFormView({ form, chip, prompt, words, onChange, onCopy, onCancel }: { form: AskForm; /** the chip's words, or null when there is no node */ chip: { label: string; title: string } | null; /** the core's sentence for the parts now */ prompt: string; /** the core's words after **Copy Prompt**, or null */ words: string | null; onChange: (part: Partial<AskForm>) => void; onCopy: () => void; onCancel: () => void }) {
  const question = useRef<HTMLTextAreaElement>(null);
  // it opens with the cursor in the question
  useEffect(() => question.current?.focus(), []);
  return (
    <div
      role="group"
      aria-label="Ask for a Tour"
      data-part="ask-form"
      className="flex max-h-[85%] flex-none flex-col gap-1.5 overflow-y-auto border-b border-b-border px-2 py-1.5 text-[12px]"
      onKeyDown={(e) => {
        if (e.key !== "Escape") return;
        e.preventDefault();
        e.stopPropagation();
        onCancel();
      }}
    >
      <div className="flex items-center gap-1.5">
        <span className="w-[52px] flex-none text-muted-foreground">About</span>
        {chip ? (
          <span data-part="ask-about" title={chip.title} className="flex min-w-0 items-center gap-1 rounded-full bg-[color:var(--card-badge-bg)] py-px pr-1 pl-2 text-[11px] text-[color:var(--card-badge-fg)]">
            <span className="min-w-0 truncate">{chip.label}</span>
            <button type="button" data-part="ask-about-remove" aria-label="Remove the node" title="Remove" className="flex size-3.5 flex-none items-center justify-center rounded-full hover:bg-toolbar-hover" onClick={() => onChange({ about: null, tour: null })}>
              <Codicon name="close" className="text-[11px]" />
            </button>
          </span>
        ) : (
          <span data-part="ask-about" className="min-w-0 truncate text-[11px] text-[color:var(--input-placeholder)]">
            anything: say it in the question
          </span>
        )}
      </div>
      <label className="flex items-start gap-1.5">
        <span className="w-[52px] flex-none pt-0.5 text-muted-foreground">Question</span>
        <textarea
          ref={question}
          data-part="ask-question"
          rows={2}
          value={form.question}
          placeholder="What do you want to understand?"
          className="min-w-0 flex-1 resize-none rounded-[3px] border border-[color:var(--input-border)] bg-[color:var(--input-bg)] px-1.5 py-0.5 text-[length:var(--font-size)] text-[color:var(--input-fg)] outline-none placeholder:text-[color:var(--input-placeholder)] focus:border-[color:var(--ring)]"
          onChange={(e) => onChange({ question: e.target.value })}
        />
      </label>
      {[
        { part: "reach", label: "How far", value: form.reach, choices: REACH_CHOICES },
        { part: "detail", label: "Detail", value: form.detail, choices: DETAIL_CHOICES },
      ].map((field) => (
        <label key={field.part} className="flex items-center gap-1.5">
          <span className="w-[52px] flex-none text-muted-foreground">{field.label}</span>
          <select
            data-part={`ask-${field.part}`}
            value={field.value}
            className="h-6 min-w-0 flex-1 rounded-[3px] border border-[color:var(--input-border)] bg-[color:var(--input-bg)] px-1 text-[length:var(--font-size)] text-[color:var(--input-fg)] outline-none focus:border-[color:var(--ring)]"
            onChange={(e) => onChange(field.part === "reach" ? { reach: e.target.value as AskForm["reach"] } : { detail: e.target.value as AskForm["detail"] })}
          >
            {field.choices.map((c) => (
              <option key={c.value} value={c.value}>
                {c.label}
              </option>
            ))}
          </select>
        </label>
      ))}
      <p data-part="ask-prompt" aria-label="The prompt" className="rounded-[3px] border border-border bg-[color:var(--input-bg)] px-1.5 py-1 text-[11px] break-words whitespace-pre-wrap text-foreground select-text">
        {prompt || "…"}
      </p>
      <div className="sticky bottom-0 -mx-2 -mb-1.5 flex items-center gap-1.5 bg-[color:var(--background)] px-2 py-1">
        <button type="button" data-part="ask-copy" className="rounded-sm border border-[color:var(--ring)] bg-[color:var(--list-hover-bg)] px-2 py-0.5 text-[11px] text-foreground hover:bg-toolbar-hover" onClick={onCopy}>
          Copy Prompt
        </button>
        <button type="button" data-part="ask-cancel" className="rounded-sm border border-border px-2 py-0.5 text-[11px] text-muted-foreground hover:text-foreground" onClick={onCancel}>
          Cancel
        </button>
        {words !== null && (
          <span data-part="ask-words" role="status" className="min-w-0 truncate text-[11px] text-muted-foreground">
            {words}
          </span>
        )}
      </div>
    </div>
  );
}
