import { useCallback, useEffect, useRef, useState } from "react";

import { dedent } from "@vnoc/shared/indent.ts";
import { type Placed, codeColumns } from "../canvas/layout.ts";
import { post, request } from "../host.ts";
import { useDirtyFiles } from "../state/dirty.ts";
import { type CodeFacts, type DraftAction, baselineOf, codeMark, draftFor, editBlock, isWritable } from "../state/draftRules.ts";
import { fileStateChanged, setBaseline, useDrafts } from "../state/drafts.ts";
import { cardRef, copyDraft, openInEditor, reloadDraft, revealCard, saveDraft, saveFile, showDiff, typeIn } from "../state/edit.ts";
import { clearCodeFocus, setCodeSize, useView } from "../state/view.ts";
import { CodeEditor } from "./CodeEditor.tsx";
import { Elsewhere, SaveFirst, SaveNotice, StateLine, Waiting } from "./DraftLines.tsx";
import { editStateOf } from "./editState.ts";

type State = { text: string; prefix: string; dirty: boolean } | { error: string } | null;

/**
 * The node's own source, read by the host from VS Code's document (so unsaved edits show), shown in Monaco coloured by Shiki. **It takes typing whenever it is shown** (docs/17-edit-in-canvas.md
 * §4, §5, 9.7): what the host answered is the card's baseline (`state/drafts.ts`), and the first keystroke that makes the text differ from it starts the draft, locally; typing back to the original
 * drops it. Not while the file has unsaved changes in a tab (the card's range comes from the last save and can point at the wrong lines): then it is read-only, and the line above the code says
 * "Save `<file>` to edit here" with a Save button; and not while another card holds the node's draft. A card with a draft never reads its code again (§5): the draft keeps its own path, range,
 * version and base. Which line and whether it types are `codeMark` and `isWritable` (`draftRules.ts`); this draws them.
 */
export function NodeCode({ node }: { node: Placed }) {
  const [state, setState] = useState<State>(null);
  const dirty = useDirtyFiles((s) => s.paths.has(node.path));
  const dirtyNow = useRef(dirty);
  dirtyNow.current = dirty;
  const draft = useDrafts((s) => draftFor(s.drafts, node));
  const hasDraft = draft !== undefined;
  const holder = useDrafts((s) => {
    const block = editBlock(s.drafts, node);
    return block?.kind === "other" ? block.holder : null; // another card holds this node's draft
  });
  const settling = useDrafts((s) => s.settling.has(node.id));
  const notice = useDrafts((s) => s.notices[node.id]);
  const editState = editStateOf(true, draft); // NodeCode is drawn only while the code is shown
  const focusRequest = useView((v) => (v.focusCode?.id === node.id ? v.focusCode.n : 0));
  const onFocused = useCallback((n: number) => clearCodeFocus(node.id, n), [node.id]);
  useEffect(() => {
    if (hasDraft) return;
    let live = true;
    request("code", { path: node.path, range: node.range! }).then(
      (got) => {
        if (!live) return;
        setState({ text: got.text, prefix: got.prefix, dirty: got.dirty });
        // the baseline a draft starts from: not of a file with unsaved changes (the card is read-only then)
        setBaseline(node.id, got.dirty || node.key === null ? null : baselineOf({ id: node.id, key: node.key, path: node.path, range: node.range! }, got));
        post({ type: "codeShown", key: node.id, dirty: dirtyNow.current, text: dedent(got.text, got.prefix) });
      },
      (e: Error) => live && setState({ error: e.message }),
    );
    return () => {
      live = false;
    };
  }, [node.id, node.key, node.path, node.range, dirty, hasDraft]); // a new range (the code above moved, or the index changed) reads the code again, and so does a save or a revert of the file, and a Discard
  // the card is as wide and its code view as tall as the code it read (not as the draft types: the size would follow each keystroke; a draft that grows scrolls inside the view)
  const read = state && "text" in state ? dedent(state.text, state.prefix) : undefined;
  const first = node.line ?? 1;
  useEffect(() => {
    if (read !== undefined) setCodeSize(node.id, { columns: codeColumns(read, first), lines: read.split("\n").length });
  }, [node.id, read, first]);
  useEffect(() => () => setCodeSize(node.id, null), [node.id]);
  useEffect(() => () => setBaseline(node.id, null), [node.id]); // a card that hides its code (or leaves the page) has no baseline to start a draft from
  useEffect(() => {
    fileStateChanged(node.id, dirty); // a "written, not saved" line has nothing left to wait for once the file is saved or reverted
  }, [dirty, node.id]);
  const file = node.path.slice(node.path.lastIndexOf("/") + 1);
  const fileDirty = dirty || (state !== null && "text" in state && state.dirty);
  const facts: CodeFacts = { hasDraft, holder, state: draft?.state ?? null, dirty: fileDirty, settling, file, notice };
  const mark = codeMark(facts);
  useEffect(() => {
    post({ type: "editState", key: node.id, state: editState, elsewhere: holder !== null, mark: mark?.kind ?? null });
  }, [node.id, editState, holder, mark?.kind]);

  const act: Record<DraftAction, () => void> = {
    reload: () => node.key !== null && reloadDraft(node.key),
    showDiff: () => node.key !== null && showDiff(node.key),
    openInEditor: () => node.key !== null && openInEditor(node.key),
    copyDraft: () => node.key !== null && copyDraft(node.key),
  };
  const shown = draft ? draft.text : state && "text" in state ? dedent(state.text, state.prefix) : undefined;
  const key = node.key;
  return (
    <>
      {mark?.kind === "elsewhere" ? (
        <Elsewhere text={mark.text} tip={`${mark.text}: one draft per function. Click to show that card.`} onClick={() => revealCard(mark.holder)} />
      ) : mark?.kind === "state" ? (
        <StateLine text={mark.text} actions={mark.actions} act={act} />
      ) : mark?.kind === "saveFirst" ? (
        <SaveFirst text={mark.text} tip={`${file} has unsaved changes. The card's lines come from the last save, so the code is read-only until it is saved.`} onSave={() => void saveFile(cardRef(node))} />
      ) : mark?.kind === "saving" ? (
        <Waiting text={mark.text} />
      ) : mark?.kind === "notice" ? (
        <SaveNotice notice={mark.notice} />
      ) : null}
      {shown !== undefined ? (
        <CodeEditor
          text={shown}
          path={draft?.path ?? node.path}
          firstLine={draft ? draft.range.start.line + 1 : node.line ?? 1}
          editState={editState}
          writable={isWritable(facts)}
          focusRequest={focusRequest}
          onFocused={onFocused}
          onText={(text) => key !== null && typeIn(node.id, key, text)}
          onSave={() => key !== null && saveDraft(key)}
        />
      ) : state && "error" in state ? (
        <div className="node-code error">{state.error}</div>
      ) : (
        <div className="node-code muted">loading…</div>
      )}
    </>
  );
}
