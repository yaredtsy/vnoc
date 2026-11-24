import { useEffect, useRef } from "react";
import type * as Monaco from "monaco-editor/editor/editor.api.js";

import { CODE_LINE_H } from "../canvas/layout.ts";
import { CODE_FIELD_CLASS } from "./codeField.ts";
import { platformNow, staysInCode, swallows } from "./cardKeys.ts";
import type { EditState } from "./editState.ts";
import { currentTheme, languageOf, loadMonaco } from "./monaco.ts";

// The code editor of a card (docs/17-edit-in-canvas.md §4), and of the doc panel (§8): Monaco, read-only until a draft is `editing`, with the key rules of 9.0, 9.2c and 9.2d. The
// text it shows and what typing does are the caller's (`text`, `onText`, `onSave`): the draft lives in the drafts store, not here.

export interface EditorProps {
  text: string;
  path: string;
  firstLine: number;
  editState: EditState;
  /** whether Monaco takes typing: the card decides (`isWritable`): a draft that is `editing`, or the code with no draft, a saved file and no other card holding the node's draft (9.7) */
  writable: boolean;
  /** the user typed: the text Monaco holds now */
  onText: (text: string) => void;
  /** ⌘S, or ⌘⌥S, in the editor */
  onSave: () => void;
  /** the Monaco language id, when the path does not say (a doc is Markdown whatever the path is) */
  language?: string;
  /** "Edit in Card" and the doc's pencil ask the editor to take the keyboard: a number that changes with each ask, 0 for none. `onFocused` says it did */
  focusRequest?: number;
  onFocused?: (request: number) => void;
}

export function CodeEditor({ text, path, firstLine, editState, writable, onText, onSave, language, focusRequest = 0, onFocused }: EditorProps) {
  const host = useRef<HTMLDivElement>(null);
  const editor = useRef<Monaco.editor.IStandaloneCodeEditor | null>(null);
  const editable = writable;
  // the first value for the editor when it is created; later changes go through `setValue` / options below
  const initial = useRef({ text, path, firstLine, editable, language });
  // what the editor's handlers read when they run: the latest props, without making the editor again
  const live = useRef({ editable, editState, onText, onSave, focusRequest, onFocused });
  live.current = { editable, editState, onText, onSave, focusRequest, onFocused };

  useEffect(() => {
    let disposed = false;
    loadMonaco().then((monaco) => {
      if (disposed || !host.current) return;
      const i = initial.current;
      editor.current = monaco.editor.create(host.current, {
        value: i.text,
        language: i.language ?? languageOf(i.path),
        // Monaco's own hidden textarea, not its EditContext (a div): VS Code's webview takes ⌘X and ⌘V, and sends them back to the page to run on the focused
        // element, where a textarea cuts and pastes and the EditContext div did nothing (⌘C only reads the selection, so it worked)
        editContext: false,
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
        lineHeight: CODE_LINE_H, // the layout makes a card's code view as tall as its lines (`codeViewHeight`)
        padding: { top: 4, bottom: 4 },
        fontFamily: getComputedStyle(document.body).getPropertyValue("--editor-font") || "monospace",
        overviewRulerLanes: 0,
        hideCursorInOverviewRuler: true,
        scrollbar: { alwaysConsumeMouseWheel: false, verticalScrollbarSize: 8, horizontalScrollbarSize: 8 },
      });
      const e = editor.current;
      // ⌘S and ⌘⌥S (Ctrl+S elsewhere) save the draft, and only the draft; `swallows` below keeps VS Code from also running its own Save
      e.addCommand(monaco.KeyMod.CtrlCmd | monaco.KeyCode.KeyS, () => live.current.onSave());
      e.addCommand(monaco.KeyMod.CtrlCmd | monaco.KeyMod.Alt | monaco.KeyCode.KeyS, () => live.current.onSave());
      // typing is the draft; `setValue` (a read, a discard) is not typing, and the caller's rule (`typeIn`) ignores a text that is the baseline's own
      e.onDidChangeModelContent(() => live.current.editable && live.current.onText(e.getValue()));
      // an ask for the keyboard that came before the editor was made
      if (live.current.focusRequest) {
        e.focus();
        live.current.onFocused?.(live.current.focusRequest);
      }
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
    if (!focusRequest || !editor.current) return;
    editor.current.focus();
    onFocused?.(focusRequest);
  }, [focusRequest, onFocused]);

  useEffect(() => {
    const e = editor.current;
    if (!e) {
      initial.current = { text, path, firstLine, editable, language };
      return;
    }
    if (e.getValue() !== text) e.setValue(text);
    const model = e.getModel();
    if (model) loadMonaco().then((m) => !model.isDisposed() && m.editor.setModelLanguage(model, language ?? languageOf(path)));
    e.updateOptions({ lineNumbers: (n) => String(n + firstLine - 1) });
  }, [text, path, firstLine, language]);

  return <div ref={host} className={CODE_FIELD_CLASS} />;
}
