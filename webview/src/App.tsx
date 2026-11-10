import { useCallback, useEffect, useMemo, useState } from "react";

import { IconArrowLeft, IconChevronRight, IconInfoCircle, IconRefresh } from "@tabler/icons-react";

import { Button } from "@/components/ui/button.tsx";
import { Toggle } from "@/components/ui/toggle.tsx";
import type { StructureResult } from "@vnoc/shared/messages.ts";
import { findNode } from "@vnoc/shared/tree.ts";
import { Canvas } from "./canvas/Canvas.tsx";
import { layout } from "./canvas/layout.ts";
import { DocPanel } from "./docs/DocPanel.tsx";
import { on, post, request, vscode } from "./host.ts";
import { dispatch, useSession } from "./state/session.ts";

type Load = { status: "loading" } | { status: "error"; message: string } | { status: "ready"; data: StructureResult };

export function App() {
  const [load, setLoad] = useState<Load>({ status: "loading" });
  const [reloads, setReloads] = useState(0);
  const saved = vscode.getState<{ docsOpen?: boolean }>();
  // LOCAL view state: which cards show their code, and the doc panel. Everything the sidebar also shows (focus, selection, open cards) is the session.
  const [codeOpen, setCodeOpen] = useState<ReadonlySet<string>>(new Set());
  const [docsOpen, setDocsOpen] = useState(saved?.docsOpen ?? false);

  // the session, read-only: focus, the selected card, the open cards, a reveal request
  const focus = useSession((s) => s.contexts[s.active].focus);
  const selected = useSession((s) => s.contexts[s.active].selected);
  const openList = useSession((s) => s.contexts[s.active].expanded);
  const revealRequest = useSession((s) => s.reveal);
  const expanded = useMemo<ReadonlySet<string>>(() => new Set(openList), [openList]);

  // The newest revision shown. Two asks can cross; the older answer must not replace the newer one.
  const [shown] = useState({ revision: -1 });

  /** `quiet`: the core's index changed while the user is looking: swap the data without the loading state, and keep the view where it is. */
  const refresh = useCallback(
    (quiet = false) => {
      if (!quiet) setLoad({ status: "loading" });
      request("structure").then(
        (data) => {
          if (data.revision < shown.revision) return;
          shown.revision = data.revision;
          setLoad({ status: "ready", data });
          if (!quiet) setReloads((n) => n + 1);
        },
        (e: Error) => setLoad({ status: "error", message: e.message }),
      );
    },
    [shown],
  );
  useEffect(() => {
    refresh();
    return on("indexChanged", () => refresh(true));
  }, [refresh]);

  const toggleCode = useCallback((id: string) => {
    setCodeOpen((open) => {
      const next = new Set(open);
      if (!next.delete(id)) next.add(id);
      return next;
    });
  }, []);
  const select = useCallback((id: string) => dispatch({ type: "select", id }), []);
  const toggle = useCallback((id: string) => dispatch({ type: "toggle", id }), []);
  const focusOn = useCallback((id: string) => dispatch({ type: "focus", id }), []); // focusing selects, so an open doc panel shows that node
  // the host asks for the title, attaches, and opens the doc; if it fails it says so itself. When a doc was added, the panel shows it.
  const showDocs = useCallback((key: string) => {
    select(key);
    setDocsOpen(true);
  }, [select]);
  const addDoc = useCallback((key: string) => void request("addDoc", { key }).then((r) => r.added && showDocs(key), () => {}), [showDocs]);

  // survives closing and reopening the panel
  useEffect(() => {
    vscode.setState({ docsOpen });
  }, [docsOpen]);

  // Alt+← goes back one focus, like the ← button
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.altKey && e.key === "ArrowLeft") dispatch({ type: "focusBack" });
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  const root = load.status === "ready" ? load.data.root : null;
  // the doc panel shows the selected card, else the focused node
  const crumbs = useMemo(() => (root ? focus.map((id) => ({ id, node: findNode(root, id) })) : []), [root, focus]);
  const top = crumbs.length > 0 ? crumbs[crumbs.length - 1].node ?? null : null;
  const docNode = root ? (findNode(root, selected ?? "") ?? top) : null;
  const centerOn = useMemo(() => (revealRequest ? { id: revealRequest.id, n: revealRequest.n } : null), [revealRequest]);

  // zoomed in: the focused node is the root of what is drawn (and the root of a drawing is always open)
  const laid = useMemo(() => (root ? layout(top ?? root, expanded, codeOpen) : null), [root, expanded, top, codeOpen]);

  // Tell the host what was drawn: the extension's test suite uses it to prove the whole chain ran (bundle, request, core, driver, layout).
  useEffect(() => {
    if (laid) post({ type: "rendered", visible: laid.nodes.length, hidden: laid.hidden });
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
        <Button className="ml-auto" size="sm" onClick={() => refresh()}>
          <IconRefresh /> Refresh
        </Button>
        <Toggle variant="outline" size="sm" pressed={docsOpen} aria-label="Docs of the selected node" title="Docs of the selected node" onPressedChange={setDocsOpen}>
          <IconInfoCircle />
        </Toggle>
      </header>
      {crumbs.length > 0 && (
        <nav className="crumbs" aria-label="Focus">
          <Button variant="ghost" size="icon-xs" title="Back one step (Alt+←)" aria-label="Back one focus" onClick={() => dispatch({ type: "focusBack" })}>
            <IconArrowLeft />
          </Button>
          <Button variant="ghost" size="xs" onClick={() => dispatch({ type: "focusJump", depth: 0 })}>
            All code
          </Button>
          {crumbs.map((c, i) => (
            <span key={c.id}>
              <IconChevronRight className="mx-1 inline size-3 opacity-50" />
              <Button variant="ghost" size="xs" className="disabled:opacity-100 disabled:font-semibold" disabled={i === crumbs.length - 1} onClick={() => dispatch({ type: "focusJump", depth: i + 1 })}>
                {c.node?.name ?? "?"}
              </Button>
            </span>
          ))}
        </nav>
      )}
      <div className="main">
        {laid && <Canvas key={`${reloads}:${top?.id ?? ""}`} laid={laid} selected={selected} onSelect={select} onToggle={toggle} onToggleCode={toggleCode} onAddDoc={addDoc} onShowDocs={showDocs} onFocus={focusOn} centerOn={centerOn} onCentered={(n) => dispatch({ type: "revealed", n })} />}
        {docsOpen && <DocPanel node={docNode} revision={load.status === "ready" ? load.data.revision : 0} onAddDoc={addDoc} onClose={() => setDocsOpen(false)} />}
      </div>
    </div>
  );
}
