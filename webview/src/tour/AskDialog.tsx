import { useEffect, useRef } from "react";

import { Codicon } from "@/components/Codicon.tsx";
import { findNode } from "@vnoc/shared/tree.ts";
import { on, post, request } from "../host.ts";
import { useData } from "../state/data.ts";
import { ASK_AGAIN_MS, ASK_DEBOUNCE_MS, DETAIL_CHOICES, MAX_DEPTH, depthOf, paramsOf, saysAskAgain, shortName, sizeAt } from "./ask.ts";
import { changeAskForm, closeAskDialog, copyAskPrompt, openAskDialog, useAskDialog } from "./askDialog.ts";

const MAC = typeof navigator !== "undefined" && /Mac|iPhone|iPad/.test(navigator.platform);

/**
 * **Ask for a Tour** (docs/26-tours/05-on-screen.md §5, TR23b): the dialog of the canvas page that builds the sentence to paste to an agent, drawn as **Commit and Pin…** is. **About** (the node, a chip that can be
 * removed), **Question**, **How far** (*Let the agent decide*, or a number of levels with the core's size of it), **Detail**, the prompt as the core gave it, **Copy Prompt** (`⌘Enter`) and **Cancel** (`Esc`).
 * It draws what it is given: the sentence, the words and the sizes are the core's, asked again a moment after the last change. The host opens it (`＋`, a node's menu, a row's **Ask for a New Tour**).
 */
export function AskDialog() {
  const dialog = useAskDialog((s) => s.dialog);
  const tree = useData((s) => s.structure?.root ?? null);
  const question = useRef<HTMLTextAreaElement>(null);
  // the host chose **Ask for a Tour…**: now, if the page is here, or at start if it was not
  useEffect(() => {
    const take = () => void request("tourAskTake").then((got) => got && void openAskDialog(got)).catch(() => {});
    take();
    return on("tourAskDialog", (from) => {
      void openAskDialog(from);
      take(); // it was taken: the host holds none now
    });
  }, []);
  const form = dialog?.form ?? null;
  const open = dialog !== null;
  // it opens with the cursor in the question
  useEffect(() => {
    if (open) question.current?.focus();
  }, [open]);
  // the core's sentence (and the size of each depth) for the parts now, asked again 300 ms after the last change, and asked again when the core says a size is still being worked out
  const asked = form === null ? null : JSON.stringify(paramsOf(form));
  const first = dialog !== null && dialog.prompt === null;
  useEffect(() => {
    const form = useAskDialog.getState().dialog?.form;
    if (!form) return;
    let wanted = true;
    let timer: ReturnType<typeof setTimeout> | undefined;
    const run = () =>
      void request("tourAskText", paramsOf(form)).then(
        (answer) => {
          if (wanted) useAskDialog.setState((s) => (s.dialog ? { dialog: { ...s.dialog, prompt: answer.text, sizes: answer.sizes ?? null } } : s));
        },
        (e) => {
          if (wanted && saysAskAgain(e)) timer = setTimeout(run, ASK_AGAIN_MS);
        },
      );
    timer = setTimeout(run, first ? 0 : ASK_DEBOUNCE_MS);
    return () => {
      wanted = false;
      clearTimeout(timer);
    };
  }, [asked]); // eslint-disable-line react-hooks/exhaustive-deps
  const label = form === null || form.about === null ? null : form.tour !== null ? shortName(form.about) : ((tree ? (findNode(tree, form.about)?.qualname ?? findNode(tree, form.about)?.name) : null) ?? shortName(form.about));
  const size = dialog === null ? null : sizeAt(dialog.sizes, dialog.form.depth);
  // what the suite reads of the dialog
  useEffect(() => {
    post({
      type: "askDialog",
      dialog: dialog === null ? null : { about: label, question: dialog.form.question, agentDepth: dialog.form.agentDepth, depth: dialog.form.depth, size, detail: dialog.form.detail, prompt: dialog.prompt ?? "" },
    });
  }, [open, label, dialog?.form.question, dialog?.form.agentDepth, dialog?.form.depth, size, dialog?.form.detail, dialog?.prompt]); // eslint-disable-line react-hooks/exhaustive-deps
  // the suite cannot click in a webview: the same controls, by the host's `viewCommand`
  useEffect(
    () =>
      on("viewCommand", ({ name, key, text }) => {
        if (name !== "askDialog" || !key) return;
        const d = useAskDialog.getState().dialog;
        if (key === "question") changeAskForm({ question: text ?? "" });
        else if (key === "agent") changeAskForm({ agentDepth: !(d?.form.agentDepth ?? true) });
        else if (key === "depth") changeAskForm({ depth: depthOf(text ?? "", d?.form.depth ?? 1) });
        else if (key === "detail") changeAskForm({ detail: (text ?? "agent") as (typeof DETAIL_CHOICES)[number]["value"] });
        else if (key === "about:remove") changeAskForm({ about: null, tour: null });
        else if (key === "copy") void copyAskPrompt();
        else if (key === "cancel") closeAskDialog();
      }),
    [],
  );
  if (!dialog || !form) return null;
  return (
    <div
      className="pin-backdrop"
      role="presentation"
      onKeyDown={(e) => {
        if (e.key === "Escape") {
          e.preventDefault();
          e.stopPropagation();
          closeAskDialog();
        } else if (e.key === "Enter" && (MAC ? e.metaKey : e.ctrlKey)) {
          e.preventDefault();
          void copyAskPrompt();
        }
      }}
    >
      <div role="dialog" aria-modal="true" aria-labelledby="ask-title" className="pin-dialog ask-dialog" data-part="ask-dialog">
        <h3 id="ask-title" className="pin-title">
          Ask for a Tour
        </h3>
        <div className="ask-row">
          <span className="ask-label">About</span>
          {label !== null && form.about !== null ? (
            <span data-part="ask-about" title={form.about} className="ask-chip">
              <span className="ask-chip-text">{label}</span>
              <button type="button" data-part="ask-about-remove" aria-label="Remove the node" title="Remove" className="ask-chip-x" onClick={() => changeAskForm({ about: null, tour: null })}>
                <Codicon name="close" className="text-[11px]" />
              </button>
            </span>
          ) : (
            <span data-part="ask-about" className="pin-dim ask-chip-none">
              anything: say it in the question
            </span>
          )}
        </div>
        <label className="pin-label">
          Question
          <textarea
            ref={question}
            data-part="ask-question"
            rows={3}
            value={form.question}
            placeholder="What do you want to understand?"
            className="pin-message"
            onChange={(e) => changeAskForm({ question: e.target.value })}
          />
        </label>
        <div className="ask-row ask-far" data-part="ask-far">
          <span className="ask-label">How far</span>
          <label className="pin-check">
            <input type="checkbox" data-part="ask-agent" checked={form.about === null || form.agentDepth} disabled={form.about === null} title={form.about === null ? "Name a node to say how far" : undefined} onChange={() => changeAskForm({ agentDepth: !form.agentDepth })} />
            Let the agent decide
          </label>
          {form.about !== null && !form.agentDepth && (
            <>
              <input
                type="number"
                data-part="ask-depth"
                aria-label="Levels deep"
                min={0}
                max={MAX_DEPTH}
                step={1}
                value={form.depth}
                className="ask-number"
                onChange={(e) => changeAskForm({ depth: depthOf(e.target.value, form.depth) })}
              />
              <span data-part="ask-size" role="status" className="pin-dim ask-size">
                {size ?? "…"}
              </span>
            </>
          )}
        </div>
        <div className="ask-row">
          <span className="ask-label">Detail</span>
          <div role="radiogroup" aria-label="Detail" className="ask-segments" data-part="ask-detail">
            {DETAIL_CHOICES.map((c) => (
              <button key={c.value} type="button" role="radio" aria-checked={form.detail === c.value} data-part={`ask-detail-${c.value}`} className={`ask-segment${form.detail === c.value ? " ask-segment-on" : ""}`} onClick={() => changeAskForm({ detail: c.value })}>
                {c.label}
              </button>
            ))}
          </div>
        </div>
        <p data-part="ask-prompt" aria-label="The prompt" className="ask-prompt">
          {dialog.prompt ?? "…"}
        </p>
        <div className="stage-buttons">
          <button type="button" data-part="ask-copy" className="stage-btn stage-primary" title={`Copy Prompt (${MAC ? "⌘" : "Ctrl+"}Enter)`} onClick={() => void copyAskPrompt()}>
            Copy Prompt
          </button>
          <button type="button" data-part="ask-cancel" className="stage-btn stage-secondary" title="Cancel (Esc)" onClick={closeAskDialog}>
            Cancel
          </button>
          <span className="pin-dim ask-hint">{MAC ? "⌘↵" : "Ctrl+↵"}</span>
        </div>
      </div>
    </div>
  );
}
