import { useEffect, useMemo } from "react";

import { IconArrowLeft, IconChevronRight, IconInfoCircle, IconRefresh } from "@tabler/icons-react";

import { Button } from "@/components/ui/button.tsx";
import { Toggle } from "@/components/ui/toggle.tsx";
import { findNode } from "@vnoc/shared/tree.ts";
import { Canvas } from "./canvas/Canvas.tsx";
import { layout } from "./canvas/layout.ts";
import { DocPanel } from "./docs/DocPanel.tsx";
import { post, request } from "./host.ts";
import { refresh, useData } from "./state/data.ts";
import { dispatch, useSession } from "./state/session.ts";
import { setDocsOpen, useView } from "./state/view.ts";

// The canvas page: the bar, the crumbs (the focus), the cards, the doc panel. It reads the stores and draws; nothing here keeps state of its own.

// right-click on the empty canvas: its own menu (Fit View, Collapse All, ...); a card inside it says "node" and wins
const PANE_CONTEXT = JSON.stringify({ webviewSection: "pane", preventDefaultContextMenuItems: true });

export function App() {
  const { status, message, structure, reloads } = useData();
  const docsOpen = useView((v) => v.docsOpen);
  const codeOpen = useView((v) => v.codeOpen);

  // the session, read-only: focus, the selected card, the open cards
  const focus = useSession((s) => s.contexts[s.active].focus);
  const selected = useSession((s) => s.contexts[s.active].selected);
  const openList = useSession((s) => s.contexts[s.active].expanded);
  const expanded = useMemo<ReadonlySet<string>>(() => new Set(openList), [openList]);

  const root = structure?.root ?? null;
  const crumbs = useMemo(() => (root ? focus.map((id) => ({ id, node: findNode(root, id) })) : []), [root, focus]);
  const top = crumbs.length > 0 ? crumbs[crumbs.length - 1].node ?? null : null;
  // zoomed in: the focused node is the root of what is drawn (and the root of a drawing is always open)
  const laid = useMemo(() => (root ? layout(top ?? root, expanded, codeOpen) : null), [root, expanded, top, codeOpen]);
  // the doc panel shows the selected card, else the focused node
  const docNode = root ? (findNode(root, selected ?? "") ?? top) : null;

  // Alt+← goes back one focus, like the ← button
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.altKey && e.key === "ArrowLeft") dispatch({ type: "focusBack" });
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  // Tell the host what was drawn: the extension's test suite uses it to prove the whole chain ran (bundle, request, core, driver, layout).
  useEffect(() => {
    if (laid) post({ type: "rendered", visible: laid.nodes.length, hidden: laid.hidden });
  }, [laid]);

  return (
    <div className="app">
      <header className="bar">
        <strong>V-NOC</strong>
        {status === "loading" && <span>reading the code…</span>}
        {status === "error" && <span className="error">{message}</span>}
        {status === "ready" && structure && (
          <span>
            {structure.root.name}: {structure.stats.folders} folders · {structure.stats.files} files · {structure.stats.classes} classes · {structure.stats.functions} functions
            {laid && laid.hidden > 0 ? ` · ${laid.hidden} hidden (click a node to expand)` : ""}
            {structure.truncated ? " · TRUNCATED: too many files" : ""}
            {structure.failed.length > 0 ? ` · ${structure.failed.length} files could not be read` : ""}
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
      <div className="main" data-vscode-context={PANE_CONTEXT}>
        {laid && <Canvas key={`${reloads}:${top?.id ?? ""}`} laid={laid} />}
        {docsOpen && (
          <DocPanel
            node={docNode}
            revision={structure?.revision ?? 0}
            onAddDoc={(key) => void request("addDoc", { key }).catch(() => {})}
            onClose={() => setDocsOpen(false)}
          />
        )}
      </div>
    </div>
  );
}
