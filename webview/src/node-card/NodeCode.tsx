import { useEffect, useRef, useState } from "react";

import { dedent, readingPrefix } from "@vnoc/shared/indent.ts";
import { META_H, type Placed } from "../canvas/layout.ts";
import { post, request } from "../host.ts";
import { useDirtyFiles } from "../state/dirty.ts";
import { type DraftAction, ELSEWHERE_TEXT, draftFor, editBlock, showElsewhere, showRefusal, stateLine } from "../state/draftRules.ts";
import { fileStateChanged, setElsewhere, setRefused, typeInto, useDrafts } from "../state/drafts.ts";
import { cardRef, copyDraft, openInEditor, reloadDraft, saveAndEdit, saveDraft, showDiff } from "../state/edit.ts";
import { CodeEditor } from "./CodeEditor.tsx";
import { Elsewhere, Refusal, SaveNotice, StateLine } from "./DraftLines.tsx";
import { editStateOf } from "./editState.ts";
import { NAME_X } from "./rows.ts";

type State = { text: string } | { error: string } | null;

/**
 * The node's own source, read by the host from VS Code's document (so unsaved edits show), shown in Monaco coloured by Shiki. Read-only, until Edit starts a draft
 * (docs/17-edit-in-canvas.md): then Monaco is writable and shows the draft, which lives in the drafts store, not here.
 * The card's range comes from the last save, so while the file has unsaved changes the lines shown may be off: a line above the code says so (not in the
 * header, whose buttons never move). A card with a draft never reads its code again (§5): the draft keeps its own path, range, version and base.
 */
export function NodeCode({ node }: { node: Placed }) {
  const [state, setState] = useState<State>(null);
  const dirty = useDirtyFiles((s) => s.paths.has(node.path));
  const dirtyNow = useRef(dirty);
  dirtyNow.current = dirty;
  const draft = useDrafts((s) => draftFor(s.drafts, node));
  const hasDraft = draft !== undefined;
  const refused = useDrafts((s) => s.refused.has(node.id));
  const notice = useDrafts((s) => s.notices[node.id]);
  const blocked = useDrafts((s) => editBlock(s.drafts, node)?.kind === "other"); // another card holds this node's draft
  const elsewhere = useDrafts((s) => showElsewhere(s.elsewhere.has(node.id), s.drafts, node));
  const editState = editStateOf(true, draft); // NodeCode is drawn only while the code is shown
  useEffect(() => {
    if (hasDraft) return;
    let live = true;
    request("code", { path: node.path, range: node.range! }).then(
      (text) => {
        if (!live) return;
        setState({ text });
        post({ type: "codeShown", key: node.id, dirty: dirtyNow.current, text: dedent(text, readingPrefix(node.range?.start.character ?? 0)) });
      },
      (e: Error) => live && setState({ error: e.message }),
    );
    return () => {
      live = false;
    };
  }, [node.id, node.path, node.range, dirty, hasDraft]); // a new range (the code above moved, or the index changed) reads the code again, and so does a save or a revert of the file, and a Discard
  useEffect(() => {
    if (!dirty) setRefused(node.id, false); // the file was saved or reverted: the refusal is over
    fileStateChanged(node.id, dirty); // and a "written, not saved" line has nothing left to wait for
  }, [dirty, node.id]);
  useEffect(() => {
    if (!blocked) setElsewhere(node.id, false); // the other card's draft is saved or discarded: the refusal is over
  }, [blocked, node.id]);
  useEffect(() => {
    post({ type: "editState", key: node.id, state: editState, elsewhere });
  }, [node.id, editState, elsewhere]);

  const line = draft ? stateLine(draft.state, node.path.slice(node.path.lastIndexOf("/") + 1)) : null;
  const act: Record<DraftAction, () => void> = {
    reload: () => void reloadDraft(cardRef(node)),
    showDiff: () => node.key !== null && showDiff(node.key),
    openInEditor: () => node.key !== null && openInEditor(node.key),
    copyDraft: () => node.key !== null && copyDraft(node.key),
  };
  const shown = draft ? draft.text : state && "text" in state ? dedent(state.text, readingPrefix(node.range?.start.character ?? 0)) : undefined;
  const key = node.key;
  return (
    <>
      {showRefusal(refused, dirty) ? (
        <Refusal path={node.path} tip={(file) => `${file} has unsaved changes. Edit needs the saved file: the card's lines come from the last save.`} onSaveAndEdit={() => void saveAndEdit(cardRef(node))} />
      ) : elsewhere ? (
        <Elsewhere text={ELSEWHERE_TEXT} tip={`${ELSEWHERE_TEXT}: one draft per function`} />
      ) : line ? (
        <StateLine text={line.text} actions={line.actions} act={act} />
      ) : notice ? (
        <SaveNotice notice={notice} />
      ) : (
        dirty && <Unsaved path={node.path} />
      )}
      {shown !== undefined ? (
        <CodeEditor
          text={shown}
          path={draft?.path ?? node.path}
          firstLine={draft ? draft.range.start.line + 1 : node.line ?? 1}
          editState={editState}
          onText={(text) => key !== null && typeInto(key, text)}
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

/** One line above the code, at the meta line's size: a filled dot and "unsaved in editor". */
function Unsaved({ path }: { path: string }) {
  const file = path.slice(path.lastIndexOf("/") + 1);
  return (
    <div
      className="flex flex-none items-center gap-1.5 pr-3 font-mono text-[11px]"
      style={{ height: META_H, paddingLeft: NAME_X, color: "var(--vscode-descriptionForeground)" }}
      title={`${file} has unsaved changes. The lines shown may be off until it is saved.`}
    >
      <span className="h-1.5 w-1.5 flex-none rounded-full" style={{ background: "var(--vscode-editorWarning-foreground)" }} />
      unsaved in editor
    </div>
  );
}

