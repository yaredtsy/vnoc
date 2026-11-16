import { useEffect, useRef, useState } from "react";
import type * as Monaco from "monaco-editor/editor/editor.api.js";

import { META_H, type Placed } from "../canvas/layout.ts";
import { post, request } from "../host.ts";
import { useDirtyFiles } from "../state/dirty.ts";
import { currentTheme, languageOf, loadMonaco } from "./monaco.ts";
import { NAME_X } from "./rows.ts";

type State = { text: string } | { error: string } | null;

/** Lines after the first still carry the definition's own indentation (the range starts at `def`, not at the line start): take it off. */
function dedent(text: string, indent: number): string {
  const [first, ...rest] = text.split("\n");
  return [first, ...rest.map((l) => l.slice(Math.min(indent, l.length - l.trimStart().length)))].join("\n");
}

/**
 * The node's own source, read by the host from VS Code's document (so unsaved edits show), shown in Monaco coloured by Shiki. Read-only for now.
 * The card's range comes from the last save, so while the file has unsaved changes the lines shown may be off: a line above the code says so (not in the
 * header, whose buttons never move).
 */
export function NodeCode({ node }: { node: Placed }) {
  const [state, setState] = useState<State>(null);
  const dirty = useDirtyFiles((s) => s.paths.has(node.path));
  const dirtyNow = useRef(dirty);
  dirtyNow.current = dirty;
  useEffect(() => {
    let live = true;
    request("code", { path: node.path, range: node.range! }).then(
      (text) => {
        if (!live) return;
        setState({ text });
        post({ type: "codeShown", key: node.id, dirty: dirtyNow.current, text: dedent(text, node.range?.start.character ?? 0) });
      },
      (e: Error) => live && setState({ error: e.message }),
    );
    return () => {
      live = false;
    };
  }, [node.id, node.path, node.range, dirty]); // a new range (the code above moved, or the index changed) reads the code again, and so does a save or a revert of the file

  return (
    <>
      {dirty && <Unsaved path={node.path} />}
      {!state ? <div className="node-code muted">loading…</div> : "error" in state ? <div className="node-code error">{state.error}</div> : <CodeEditor text={dedent(state.text, node.range?.start.character ?? 0)} path={node.path} firstLine={node.line ?? 1} />}
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

function CodeEditor({ text, path, firstLine }: { text: string; path: string; firstLine: number }) {
  const host = useRef<HTMLDivElement>(null);
  const editor = useRef<Monaco.editor.IStandaloneCodeEditor | null>(null);
  // the first value for the editor when it is created; later changes go through `setValue` / options below
  const initial = useRef({ text, path, firstLine });

  useEffect(() => {
    let disposed = false;
    loadMonaco().then((monaco) => {
      if (disposed || !host.current) return;
      const i = initial.current;
      editor.current = monaco.editor.create(host.current, {
        value: i.text,
        language: languageOf(i.path),
        readOnly: true,
        theme: currentTheme(), // Shiki builds its colour map when an editor is created with a theme; without it everything is one colour
        automaticLayout: true, // the card is resized by the canvas; Monaco follows its container
        minimap: { enabled: false },
        scrollBeyondLastLine: false,
        lineNumbers: (n) => String(n + i.firstLine - 1), // lines of the file, not of the slice
        lineNumbersMinChars: 4,
        folding: false,
        glyphMargin: false,
        renderLineHighlight: "none",
        domReadOnly: true,
        contextmenu: false,
        fontSize: 11,
        fontFamily: getComputedStyle(document.body).getPropertyValue("--editor-font") || "monospace",
        overviewRulerLanes: 0,
        hideCursorInOverviewRuler: true,
        scrollbar: { alwaysConsumeMouseWheel: false, verticalScrollbarSize: 8, horizontalScrollbarSize: 8 },
      });
    });
    return () => {
      disposed = true;
      editor.current?.dispose();
      editor.current = null;
    };
  }, []);

  useEffect(() => {
    const e = editor.current;
    if (!e) {
      initial.current = { text, path, firstLine };
      return;
    }
    if (e.getValue() !== text) e.setValue(text);
    const model = e.getModel();
    if (model) loadMonaco().then((m) => !model.isDisposed() && m.editor.setModelLanguage(model, languageOf(path)));
    e.updateOptions({ lineNumbers: (n) => String(n + firstLine - 1) });
  }, [text, path, firstLine]);

  // `nodrag nopan nowheel`: selecting text must not drag the card, and the wheel scrolls the code, not the canvas
  return <div ref={host} className="node-code monaco-host nodrag nopan nowheel" />;
}
