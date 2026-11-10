// The doc panel: a node's docs, rendered and read-only (07-ui/02-design.md §3). Editing happens in a normal editor tab ("Edit"); this panel is
// its live preview. The panel never writes, and it never holds the truth: the host lists the docs and sends their text.

import { useEffect, useMemo, useState } from "react";

import { onDocText, request } from "../host.ts";
import type { TreeNode } from "../types.ts";
import { renderMarkdown } from "./markdown.ts";

interface DocInfo {
  file: string;
  title: string;
  path: string;
}

const NEW_DOC = "+new";

export function DocPanel({ node, revision, onAddDoc, onClose }: { node: TreeNode | null; revision: number; onAddDoc: (id: string) => void; onClose: () => void }) {
  const nodeId = node?.node_id ?? null;
  const [docs, setDocs] = useState<DocInfo[]>([]);
  const [chosen, setChosen] = useState<string | null>(null);
  const [text, setText] = useState<string | null>(null);

  // the docs of this node: again whenever the index changes (a doc was added, removed or renamed)
  useEffect(() => {
    if (!nodeId) return setDocs([]);
    let current = true;
    request<DocInfo[]>("docs", { id: nodeId }).then((list) => current && setDocs(list), () => current && setDocs([]));
    return () => void (current = false);
  }, [nodeId, revision]);

  // keep the chosen doc while it exists, else the first
  const doc = docs.find((d) => d.path === chosen) ?? docs[0] ?? null;
  const path = doc?.path ?? null;

  useEffect(() => {
    if (!path) return setText(null);
    let current = true;
    request<string>("docText", { path }).then((t) => current && setText(t), () => current && setText(null));
    // typing in the editor tab, or a change on disk: the host sends the new text
    const stop = onDocText((changed, newText) => changed === path && setText(newText));
    return () => {
      current = false;
      stop();
    };
  }, [path]);

  const html = useMemo(() => (text === null ? "" : renderMarkdown(text)), [text]);

  const pick = (value: string) => (value === NEW_DOC && node ? onAddDoc(node.id) : setChosen(value));
  const onBodyClick = (e: React.MouseEvent) => {
    const link = (e.target as HTMLElement).closest("a[data-href]");
    if (!link || !path) return;
    e.preventDefault();
    void request("openLink", { href: link.getAttribute("data-href"), from: path }).catch(() => {});
  };

  return (
    <aside className="doc-panel">
      <header>
        {docs.length > 0 ? (
          <select value={doc?.path ?? ""} onChange={(e) => pick(e.target.value)} aria-label="Doc">
            {docs.map((d) => (
              <option key={d.path} value={d.path}>
                {d.title}
              </option>
            ))}
            <option value={NEW_DOC}>＋ New doc…</option>
          </select>
        ) : (
          <span className="title">Docs</span>
        )}
        {path && <button onClick={() => void request("openDoc", { path }).catch(() => {})}>✎ Edit</button>}
        <button className="close" aria-label="Close the doc panel" onClick={onClose}>
          ✕
        </button>
      </header>
      {node && (
        <div className="doc-node">
          {node.name} · {node.path}
          {node.range ? `:${node.range.start.line + 1}` : ""}
        </div>
      )}
      <div className="doc-body" onClick={onBodyClick}>
        {!node && <p className="empty">Select a node to see its docs.</p>}
        {node && docs.length === 0 && (
          <div className="empty">
            <p>No docs on <code>{node.name}</code> yet.</p>
            {(node.kind === "function" || node.kind === "class") && <button onClick={() => onAddDoc(node.id)}>Write the first doc</button>}
          </div>
        )}
        {doc && <div className="markdown" dangerouslySetInnerHTML={{ __html: html }} />}
      </div>
    </aside>
  );
}
