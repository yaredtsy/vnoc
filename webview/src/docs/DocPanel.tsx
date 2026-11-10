// The doc panel: a node's docs, rendered and read-only (07-ui/02-design.md §3). Editing happens in a normal editor tab ("Edit"); this panel is
// its live preview. The panel never writes, and it never holds the truth: the host lists the docs and sends their text.

import { type ReactNode, useEffect, useMemo, useState } from "react";

import { IconFileText, IconPencil, IconPlus, IconPointer, IconX } from "@tabler/icons-react";

import { Button } from "@/components/ui/button.tsx";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select.tsx";
import { onDocText, request } from "../host.ts";
import type { TreeNode } from "../types.ts";
import { renderMarkdown } from "./markdown.ts";

interface DocInfo {
  file: string;
  title: string;
  path: string;
}

const NEW_DOC = "+new";
const MIN_WIDTH = 280;
const MAX_WIDTH = 800;

interface DocPanelProps {
  node: TreeNode | null;
  revision: number;
  onAddDoc: (id: string) => void;
  onClose: () => void;
  /** take the whole width (the docs tab beside a code editor) instead of a resizable column next to the canvas */
  fill?: boolean;
  /** extra header buttons, before the close button */
  extra?: ReactNode;
}

export function DocPanel({ node, revision, onAddDoc, onClose, fill = false, extra }: DocPanelProps) {
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

  // drag the left edge to resize; pointer capture keeps the drag alive over the canvas
  const [width, setWidth] = useState(380);
  const onResizeMove = (e: React.PointerEvent<HTMLDivElement>) => {
    if (e.buttons !== 1) return;
    setWidth(Math.min(MAX_WIDTH, Math.max(MIN_WIDTH, window.innerWidth - e.clientX)));
  };

  return (
    <aside className={`relative flex flex-col bg-background text-[13px] ${fill ? "min-w-0 flex-1" : "flex-none border-l border-border"}`} style={fill ? undefined : { width }}>
      {!fill && (
        <div
          role="separator"
          aria-orientation="vertical"
          aria-label="Resize the doc panel"
          className="absolute inset-y-0 -left-1 z-10 w-2 cursor-col-resize touch-none transition-colors hover:bg-ring/40 active:bg-ring/60"
          onPointerDown={(e) => e.currentTarget.setPointerCapture(e.pointerId)}
          onPointerMove={onResizeMove}
        />
      )}
      <header className="flex flex-none items-center gap-2 border-b border-border px-3 py-2">
        <div className="min-w-0 flex-1">
          <div className="truncate font-semibold">{node?.name ?? "Docs"}</div>
          {node && (
            <div className="truncate font-mono text-[11px] text-muted-foreground" title={node.path}>
              {node.path}
              {node.range ? `:${node.range.start.line + 1}` : ""}
            </div>
          )}
        </div>
        {docs.length > 0 && (
          <Select value={doc?.path ?? ""} items={docs.map((d) => ({ value: d.path, label: d.title }))} onValueChange={(value) => value && pick(value)}>
            <SelectTrigger size="sm" className="max-w-[45%] min-w-0 font-medium" aria-label="Doc">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              {docs.map((d) => (
                <SelectItem key={d.path} value={d.path}>
                  {d.title}
                </SelectItem>
              ))}
              <SelectItem value={NEW_DOC}>
                <IconPlus /> New doc…
              </SelectItem>
            </SelectContent>
          </Select>
        )}
        {path && (
          <Button variant="ghost" size="icon-sm" title="Edit this doc" aria-label="Edit this doc" onClick={() => void request("openDoc", { path }).catch(() => {})}>
            <IconPencil />
          </Button>
        )}
        {extra}
        <Button variant="ghost" size="icon-sm" aria-label="Close the doc panel" onClick={onClose}>
          <IconX />
        </Button>
      </header>
      <div className="flex-1 overflow-auto px-4 py-4" onClick={onBodyClick}>
        {!node && (
          <div className="mt-10 flex flex-col items-center gap-2 text-center text-muted-foreground">
            <IconPointer className="size-8 opacity-60" />
            <p>Select a node to see its docs.</p>
          </div>
        )}
        {node && docs.length === 0 && (
          <div className="mt-10 flex flex-col items-center gap-3 text-center text-muted-foreground">
            <IconFileText className="size-8 opacity-60" />
            <p>
              No docs on <code className="rounded bg-muted px-1.5 py-0.5 font-mono text-foreground">{node.name}</code> yet.
            </p>
            {node.id !== "/" && (
              <Button variant="outline" size="sm" onClick={() => onAddDoc(node.id)}>
                <IconPlus /> Write the first doc
              </Button>
            )}
          </div>
        )}
        {doc && <div className="markdown" dangerouslySetInnerHTML={{ __html: html }} />}
      </div>
    </aside>
  );
}
