import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import { onFocus, onIndexChanged, request, vscode } from "./host.ts";
import { Canvas } from "./canvas/Canvas.tsx";
import { DocPanel } from "./docs/DocPanel.tsx";
import { type FocusStack, jump, prune, push } from "./canvas/focusStack.ts";
import { defaultCollapsed, layout } from "./canvas/layout.ts";
import { findNode } from "./tree.ts";
import type { StructureResult, TreeNode } from "./types.ts";

type Load = { status: "loading" } | { status: "error"; message: string } | { status: "ready"; data: StructureResult };

export function App() {
  const [load, setLoad] = useState<Load>({ status: "loading" });
  const [reloads, setReloads] = useState(0);
  const saved = vscode.getState<{ collapsed: string[]; focus?: string[]; docsOpen?: boolean }>();
  const [collapsed, setCollapsed] = useState<Set<string> | null>(saved ? new Set(saved.collapsed) : null);
  const [codeOpen, setCodeOpen] = useState<ReadonlySet<string>>(new Set());
  const [focus, setFocus] = useState<FocusStack>(saved?.focus ?? []);
  // the doc panel: open or closed (saved), and the card the user clicked last
  const [docsOpen, setDocsOpen] = useState(saved?.docsOpen ?? false);
  const [selected, setSelected] = useState<string | null>(null);

  // The newest revision shown. Two asks can cross; the older answer must not replace the newer one.
  const shown = useRef(-1);

  /** `quiet`: the core's index changed while the user is looking: swap the data without the loading state, and keep the view where it is. */
  const refresh = useCallback((quiet = false) => {
    if (!quiet) setLoad({ status: "loading" });
    request<StructureResult>("structure").then(
      (data) => {
        if (data.revision < shown.current) return;
        shown.current = data.revision;
        setCollapsed((c) => c ?? new Set(defaultCollapsed(data.root)));
        setLoad({ status: "ready", data });
        if (!quiet) setReloads((n) => n + 1);
      },
      (e: Error) => setLoad({ status: "error", message: e.message }),
    );
  }, []);
  useEffect(() => {
    refresh();
    return onIndexChanged(() => refresh(true));
  }, [refresh]);

  const toggle = useCallback((id: string) => {
    setCollapsed((c) => {
      const next = new Set(c ?? []);
      if (!next.delete(id)) next.add(id);
      return next;
    });
  }, []);
  const toggleCode = useCallback((id: string) => {
    setCodeOpen((open) => {
      const next = new Set(open);
      if (!next.delete(id)) next.add(id);
      return next;
    });
  }, []);
  // the host asks for the title, attaches, and opens the doc; if it fails it says so itself. When a doc was added, the panel shows it.
  const showDocs = useCallback((key: string) => {
    setSelected(key);
    setDocsOpen(true);
  }, []);
  const addDoc = useCallback(
    (key: string) => {
      const docs = (rootRef.current && findNode(rootRef.current, key)?.docs) || 0;
      request<{ added: boolean }>("addDoc", { key, docs }).then((r) => r.added && showDocs(key), () => {});
    },
    [showDocs],
  );
  const focusOn = useCallback((id: string) => setFocus((f) => push(f, id)), []);

  // survives closing and reopening the panel
  useEffect(() => {
    if (collapsed) vscode.setState({ collapsed: [...collapsed], focus, docsOpen });
  }, [collapsed, focus, docsOpen]);

  // A Focus click in the sidebar: while the canvas is open the host sends it; a canvas opened by that click asks for it once it is up.
  useEffect(() => {
    request<string | null>("pendingFocus").then((id) => id && focusOn(id), () => {});
    return onFocus(focusOn);
  }, [focusOn]);

  // After a refresh the node we were zoomed into may be gone: drop the stack down to one that still exists.
  const root = load.status === "ready" ? load.data.root : null;
  useEffect(() => {
    if (root) setFocus((f) => prune(f, (id) => findNode(root, id) !== null));
  }, [root]);
  const rootRef = useRef<TreeNode | null>(null);
  rootRef.current = root;
  // the doc panel shows the selected card, else the focused node
  const docNode = root ? (findNode(root, selected ?? "") ?? (focus.length > 0 ? findNode(root, focus[focus.length - 1]) : null)) : null;
  const crumbs = useMemo(() => (root ? focus.map((id) => ({ id, node: findNode(root, id) })) : []), [root, focus]);
  const top = crumbs.length > 0 ? crumbs[crumbs.length - 1].node : null;

  const laid = useMemo(() => {
    if (!root || !collapsed) return null;
    // zoomed in: the focused node is the root of what is drawn, and it is open (otherwise there would be nothing to see)
    return top ? layout(top, new Set([...collapsed].filter((id) => id !== top.id)), codeOpen) : layout(root, collapsed, codeOpen);
  }, [root, collapsed, top, codeOpen]);

  // Tell the host what was drawn: the extension's test suite uses it to prove the whole chain ran (bundle, request, core, driver, layout).
  useEffect(() => {
    if (laid) vscode.postMessage({ type: "rendered", visible: laid.nodes.length, hidden: laid.hidden });
  }, [laid]);

  return (
    <div className="app">
      <header className="bar">
        <strong>V-NOC</strong>
        {load.status === "loading" && <span>reading the code…</span>}
        {load.status === "error" && <span className="error">{load.message}</span>}
        {load.status === "ready" && (
          <span>
            {load.data.root.name}: {load.data.stats.folders} folders · {load.data.stats.files} files · {load.data.stats.classes} classes · {load.data.stats.functions} functions
            {laid && laid.hidden > 0 ? ` · ${laid.hidden} hidden (click a node to expand)` : ""}
            {load.data.truncated ? " · TRUNCATED: too many files" : ""}
            {load.data.failed.length > 0 ? ` · ${load.data.failed.length} files could not be read` : ""}
          </span>
        )}
        <button onClick={() => refresh()}>Refresh</button>
        <button className={`info${docsOpen ? " on" : ""}`} aria-pressed={docsOpen} title="Docs of the selected node" onClick={() => setDocsOpen((open) => !open)}>
          ⓘ
        </button>
      </header>
      {crumbs.length > 0 && (
        <nav className="crumbs" aria-label="Focus">
          <button onClick={() => setFocus((f) => jump(f, 0))}>All code</button>
          {crumbs.map((c, i) => (
            <span key={c.id}>
              <span className="sep">›</span>
              <button disabled={i === crumbs.length - 1} onClick={() => setFocus((f) => jump(f, i + 1))}>
                {c.node?.name ?? "?"}
              </button>
            </span>
          ))}
        </nav>
      )}
      <div className="main">
        {laid && <Canvas key={`${reloads}:${top?.id ?? ""}`} laid={laid} selected={selected} onSelect={setSelected} onToggle={toggle} onToggleCode={toggleCode} onAddDoc={addDoc} onShowDocs={showDocs} onFocus={focusOn} />}
        {docsOpen && <DocPanel node={docNode} revision={load.status === "ready" ? load.data.revision : 0} onAddDoc={addDoc} onClose={() => setDocsOpen(false)} />}
      </div>
    </div>
  );
}
