import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import { IconArrowLeft, IconChevronRight, IconInfoCircle, IconRefresh } from "@tabler/icons-react";

import { Button } from "@/components/ui/button.tsx";
import { Toggle } from "@/components/ui/toggle.tsx";
import { on, post, request, vscode } from "./host.ts";
import { Canvas } from "./canvas/Canvas.tsx";
import { DocPanel } from "./docs/DocPanel.tsx";
import { type FocusStack, jump, pop, prune, push } from "@vnoc/shared/focusStack.ts";
import { layout } from "./canvas/layout.ts";
import { reveal } from "./canvas/reveal.ts";
import type { StructureResult } from "@vnoc/shared/messages.ts";
import { findNode } from "@vnoc/shared/tree.ts";

type Load = { status: "loading" } | { status: "error"; message: string } | { status: "ready"; data: StructureResult };

export function App() {
  const [load, setLoad] = useState<Load>({ status: "loading" });
  const [reloads, setReloads] = useState(0);
  const saved = vscode.getState<{ expanded?: string[]; docsOpen?: boolean }>();
  // what the user opened. At first nothing is: the root is open and its children are closed.
  const [expanded, setExpanded] = useState<ReadonlySet<string>>(new Set(saved?.expanded));
  const [codeOpen, setCodeOpen] = useState<ReadonlySet<string>>(new Set());
  // The focus stack belongs to the host: the sidebar shares it. This is a copy that starts empty, is filled from the host, and tells it about every change.
  const [focus, setFocus] = useState<FocusStack>([]);
  const focusLoaded = useRef(false);
  // the doc panel: open or closed (saved), and the card the user clicked last
  const [docsOpen, setDocsOpen] = useState(saved?.docsOpen ?? false);
  const [selected, setSelected] = useState<string | null>(null);
  // "Show on canvas": the node to reveal once the tree is there, and the node to bring to the middle of the view
  const [revealId, setRevealId] = useState<string | null>(null);
  const [centerOn, setCenterOn] = useState<{ id: string; n: number } | null>(null);

  // The newest revision shown. Two asks can cross; the older answer must not replace the newer one.
  const shown = useRef(-1);

  /** `quiet`: the core's index changed while the user is looking: swap the data without the loading state, and keep the view where it is. */
  const refresh = useCallback((quiet = false) => {
    if (!quiet) setLoad({ status: "loading" });
    request("structure").then(
      (data) => {
        if (data.revision < shown.current) return;
        shown.current = data.revision;
        setLoad({ status: "ready", data });
        if (!quiet) setReloads((n) => n + 1);
      },
      (e: Error) => setLoad({ status: "error", message: e.message }),
    );
  }, []);
  useEffect(() => {
    refresh();
    return on("indexChanged", () => refresh(true));
  }, [refresh]);

  const toggle = useCallback((id: string) => {
    setExpanded((open) => {
      const next = new Set(open);
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
      request("addDoc", { key }).then((r) => r.added && showDocs(key), () => {});
    },
    [showDocs],
  );
  // focusing a node also selects it, so an open doc panel shows that node's docs
  const focusOn = useCallback((id: string) => {
    setFocus((f) => push(f, id));
    setSelected(id);
  }, []);

  // survives closing and reopening the panel (the focus is the host's: see above)
  useEffect(() => {
    vscode.setState({ expanded: [...expanded], docsOpen });
  }, [expanded, docsOpen]);

  // the focus: ask the host for it once, then follow it, and report every change of ours (the sidebar follows). An echo changes nothing on the host.
  useEffect(() => {
    request("focusStack").then(
      (stack) => {
        setFocus(stack);
        focusLoaded.current = true;
      },
      () => (focusLoaded.current = true),
    );
    return on("focusStack", ({ stack }) => setFocus(stack));
  }, []);
  useEffect(() => {
    if (focusLoaded.current) post({ type: "focusChanged", stack: [...focus] });
  }, [focus]);

  // Show on canvas from a menu: while the canvas is open the host sends it; a canvas opened by that command asks for it once it is up.
  useEffect(() => {
    request("pending").then((p) => p && setRevealId(p.id), () => {});
    return on("reveal", ({ id }) => setRevealId(id));
  }, []);

  // Alt+← goes back one focus, like the ← button
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.altKey && e.key === "ArrowLeft") setFocus((f) => (f.length > 0 ? pop(f) : f));
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  // After a refresh the node we were zoomed into may be gone: drop the stack down to one that still exists.
  const root = load.status === "ready" ? load.data.root : null;
  useEffect(() => {
    if (root) setFocus((f) => prune(f, (id) => findNode(root, id) !== undefined));
  }, [root]);
  // Show on canvas: back to the nearest focus that contains the node, open its ancestors, select it, bring it to the middle
  useEffect(() => {
    if (!root || !revealId) return;
    const shown = reveal(root, focus, revealId);
    if (shown) {
      setFocus(shown.focus);
      setExpanded((open) => new Set([...open, ...shown.open]));
      setSelected(revealId);
      setCenterOn((c) => ({ id: revealId, n: (c?.n ?? 0) + 1 }));
    }
    setRevealId(null);
  }, [root, revealId, focus]);
  // the doc panel shows the selected card, else the focused node
  const docNode = root ? (findNode(root, selected ?? "") ?? (focus.length > 0 ? findNode(root, focus[focus.length - 1]) : null) ?? null) : null;
  const crumbs = useMemo(() => (root ? focus.map((id) => ({ id, node: findNode(root, id) })) : []), [root, focus]);
  const top = crumbs.length > 0 ? crumbs[crumbs.length - 1].node : null;


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
          <Button variant="ghost" size="icon-xs" title="Back one step (Alt+←)" aria-label="Back one focus" onClick={() => setFocus(pop)}>
            <IconArrowLeft />
          </Button>
          <Button variant="ghost" size="xs" onClick={() => setFocus((f) => jump(f, 0))}>
            All code
          </Button>
          {crumbs.map((c, i) => (
            <span key={c.id}>
              <IconChevronRight className="mx-1 inline size-3 opacity-50" />
              <Button variant="ghost" size="xs" className="disabled:opacity-100 disabled:font-semibold" disabled={i === crumbs.length - 1} onClick={() => setFocus((f) => jump(f, i + 1))}>
                {c.node?.name ?? "?"}
              </Button>
            </span>
          ))}
        </nav>
      )}
      <div className="main">
        {laid && <Canvas key={`${reloads}:${top?.id ?? ""}`} laid={laid} selected={selected} onSelect={setSelected} onToggle={toggle} onToggleCode={toggleCode} onAddDoc={addDoc} onShowDocs={showDocs} onFocus={focusOn} centerOn={centerOn} />}
        {docsOpen && <DocPanel node={docNode} revision={load.status === "ready" ? load.data.revision : 0} onAddDoc={addDoc} onClose={() => setDocsOpen(false)} />}
      </div>
    </div>
  );
}
