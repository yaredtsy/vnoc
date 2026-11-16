import { useEffect, useRef, useState } from "react";
import type * as Monaco from "monaco-editor/editor/editor.api.js";

import { Button } from "@/components/ui/button.tsx";
import { META_H, type Placed } from "../canvas/layout.ts";
import { post, request } from "../host.ts";
import { useDirtyFiles } from "../state/dirty.ts";
import { draftFor, showRefusal } from "../state/draftRules.ts";
import { setRefused, typeInto, useDrafts } from "../state/drafts.ts";
import { cardRef, saveAndEdit, saveDraft } from "../state/edit.ts";
import { type EditState, editStateOf } from "./editState.ts";
import { platformNow, staysInCode, swallows } from "./cardKeys.ts";
import { currentTheme, languageOf, loadMonaco } from "./monaco.ts";
import { CODE_FIELD_CLASS } from "./codeField.ts";
import { dedent, readingPrefix } from "./indent.ts";
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
  }, [dirty, node.id]);
  useEffect(() => {
    post({ type: "editState", key: node.id, state: editState });
  }, [node.id, editState]);

  const shown = draft ? draft.text : state && "text" in state ? dedent(state.text, readingPrefix(node.range?.start.character ?? 0)) : undefined;
  const key = node.key;
  return (
    <>
      {showRefusal(refused, dirty) ? <Refusal path={node.path} onSaveAndEdit={() => void saveAndEdit(cardRef(node))} /> : dirty && <Unsaved path={node.path} />}
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

/** E5, in the card's body: Edit was refused because the file has unsaved changes. The card's range comes from the last save. */
function Refusal({ path, onSaveAndEdit }: { path: string; onSaveAndEdit: () => void }) {
  const file = path.slice(path.lastIndexOf("/") + 1);
  return (
    <div className="flex flex-none items-center gap-1.5 pr-2 font-mono text-[11px]" style={{ height: META_H, paddingLeft: NAME_X, color: "var(--vscode-descriptionForeground)" }}>
      <span className="h-1.5 w-1.5 flex-none rounded-full" style={{ background: "var(--vscode-editorWarning-foreground)" }} />
      <span className="min-w-0 flex-1 truncate" title={`${file} has unsaved changes. Edit needs the saved file: the card's lines come from the last save.`}>
        Save {file} first
      </span>
      <Button variant="secondary" size="xs" className="nodrag nopan h-5 flex-none" onClick={onSaveAndEdit}>
        Save and Edit
      </Button>
    </div>
  );
}

interface EditorProps {
  text: string;
  path: string;
  firstLine: number;
  editState: EditState;
  /** the user typed: the draft's text */
  onText: (text: string) => void;
  /** ⌘S, or ⌘⌥S, in the editor */
  onSave: () => void;
}

function CodeEditor({ text, path, firstLine, editState, onText, onSave }: EditorProps) {
  const host = useRef<HTMLDivElement>(null);
  const editor = useRef<Monaco.editor.IStandaloneCodeEditor | null>(null);
  const editable = editState === "editing";
  // the first value for the editor when it is created; later changes go through `setValue` / options below
  const initial = useRef({ text, path, firstLine, editable });
  // what the editor's handlers read when they run: the latest props, without making the editor again
  const live = useRef({ editable, editState, onText, onSave });
  live.current = { editable, editState, onText, onSave };

  useEffect(() => {
    let disposed = false;
    loadMonaco().then((monaco) => {
      if (disposed || !host.current) return;
      const i = initial.current;
      editor.current = monaco.editor.create(host.current, {
        value: i.text,
        language: languageOf(i.path),
        readOnly: !i.editable,
        theme: currentTheme(), // Shiki builds its colour map when an editor is created with a theme; without it everything is one colour
        automaticLayout: true, // the card is resized by the canvas; Monaco follows its container
        minimap: { enabled: false },
        scrollBeyondLastLine: false,
        lineNumbers: (n) => String(n + i.firstLine - 1), // lines of the file, not of the slice
        lineNumbersMinChars: 4,
        folding: false,
        glyphMargin: false,
        renderLineHighlight: "none",
        domReadOnly: !i.editable,
        contextmenu: false,
        fontSize: 11,
        fontFamily: getComputedStyle(document.body).getPropertyValue("--editor-font") || "monospace",
        overviewRulerLanes: 0,
        hideCursorInOverviewRuler: true,
        scrollbar: { alwaysConsumeMouseWheel: false, verticalScrollbarSize: 8, horizontalScrollbarSize: 8 },
      });
      const e = editor.current;
      // ⌘S and ⌘⌥S (Ctrl+S elsewhere) save the draft, and only the draft; `swallows` below keeps VS Code from also running its own Save
      e.addCommand(monaco.KeyMod.CtrlCmd | monaco.KeyCode.KeyS, () => live.current.onSave());
      e.addCommand(monaco.KeyMod.CtrlCmd | monaco.KeyMod.Alt | monaco.KeyCode.KeyS, () => live.current.onSave());
      // typing is the draft; `setValue` (a read, a discard) is not typing
      e.onDidChangeModelContent(() => live.current.editable && live.current.onText(e.getValue()));
    });
    return () => {
      disposed = true;
      editor.current?.dispose();
      editor.current = null;
    };
  }, []);

  useEffect(() => {
    const box = host.current;
    if (!box) return;
    // The card's container, bubble phase: Monaco's own handler runs first (it is lower down), then this one keeps the key from VS Code (docs/17 §7, measured in 9.0)
    const onKey = (ev: KeyboardEvent) => {
      const key = { key: ev.key, code: ev.code, meta: ev.metaKey, ctrl: ev.ctrlKey, alt: ev.altKey, shift: ev.shiftKey };
      // VS Code's own Save keys (9.0), and the typing keys React Flow would take from a field because a modifier is held (9.2c)
      if (swallows(live.current.editState, key, platformNow()) || staysInCode(live.current.editState, key)) ev.stopPropagation();
    };
    box.addEventListener("keydown", onKey);
    return () => box.removeEventListener("keydown", onKey);
  }, []);

  useEffect(() => {
    editor.current?.updateOptions({ readOnly: !editable, domReadOnly: !editable });
  }, [editable]);

  useEffect(() => {
    const e = editor.current;
    if (!e) {
      initial.current = { text, path, firstLine, editable };
      return;
    }
    if (e.getValue() !== text) e.setValue(text);
    const model = e.getModel();
    if (model) loadMonaco().then((m) => !model.isDisposed() && m.editor.setModelLanguage(model, languageOf(path)));
    e.updateOptions({ lineNumbers: (n) => String(n + firstLine - 1) });
  }, [text, path, firstLine]);

  return <div ref={host} className={CODE_FIELD_CLASS} />;
}
