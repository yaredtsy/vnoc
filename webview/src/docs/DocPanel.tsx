// The doc panel: a node's docs, rendered (07-ui/02-design.md §3), and editable in place (docs/17-edit-in-canvas.md §8). The canvas's panel and the docs tab are this one component. Edit starts a
// draft of the whole `.md` (the drafts store, `state/drafts.ts`) and shows it in Monaco in place of the rendered Markdown; Save writes it through the host. The panel never writes a file itself,
// and it never holds the truth: the host lists the docs and sends their text. Which buttons and which line are `docEdit.ts`.

import { type ReactNode, useEffect, useMemo, useState } from "react";

import { IconFileText, IconPlus, IconPointer, IconX } from "@tabler/icons-react";

import { Codicon } from "@/components/Codicon.tsx";
import { IconButton } from "@/components/IconButton.tsx";
import { Button } from "@/components/ui/button.tsx";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select.tsx";
import { on, post, request } from "../host.ts";
import { platformNow, saveTip } from "../node-card/cardKeys.ts";
import { CodeEditor } from "../node-card/CodeEditor.tsx";
import { Elsewhere, Refusal, SaveNotice, StateLine } from "../node-card/DraftLines.tsx";
import { type DraftAction, isChanged } from "../state/draftRules.ts";
import { editStateOf } from "../node-card/editState.ts";
import { typeInto, useDrafts } from "../state/drafts.ts";
import { beginDocEdit, copyDraft, discardDraft, openInEditor, reloadDocDraft, saveDraft, showDiff } from "../state/edit.ts";
import { PAGE } from "../state/page.ts";
import { setDocWidth, useView } from "../state/view.ts";
import { type DocSubject, calledByText } from "@vnoc/shared/docSubject.ts";
import type { DocInfo, TreeNode } from "@vnoc/shared/messages.ts";
import { type DocButtonId, docButtons, docLine } from "./docEdit.ts";
import { renderMarkdown } from "./markdown.ts";

const NEW_DOC = "+new";
const MIN_WIDTH = 280;
const MAX_WIDTH = 800;

interface DocPanelProps {
  node: TreeNode | null;
  /** for a selected call on the canvas: who calls it, shown on top of its docs */
  calledBy?: DocSubject["calledBy"];
  revision: number;
  onAddDoc: (id: string) => void;
  onClose: () => void;
  /** take the whole width (the docs tab beside a code editor) instead of a resizable column next to the canvas */
  fill?: boolean;
  /** extra header buttons, before the close button */
  extra?: ReactNode;
  /** what to say when there is no node (the canvas: nothing is selected; the docs tab: the cursor is not in a function or class) */
  emptyText?: string;
}

export function DocPanel({ node, calledBy = null, revision, onAddDoc, onClose, fill = false, extra, emptyText = "Select a node to see its docs." }: DocPanelProps) {
  const nodeId = node?.node_id ?? null;
  // Each answer keeps what it was asked for, so another node's docs (or another doc's text) are never shown while the new ones load.
  const [listed, setListed] = useState<{ nodeId: string; docs: DocInfo[] } | null>(null);
  const [chosen, setChosen] = useState<string | null>(null);
  const [read, setRead] = useState<{ path: string; text: string | null } | null>(null);

  // the docs of this node: again whenever the index changes (a doc was added, removed or renamed)
  useEffect(() => {
    if (!nodeId) return;
    let current = true;
    request("docs", { nodeId }).then((docs) => current && setListed({ nodeId, docs }), () => current && setListed({ nodeId, docs: [] }));
    return () => void (current = false);
  }, [nodeId, revision]);
  const loading = nodeId !== null && listed?.nodeId !== nodeId;
  // A node with no docs has no permanent ID (`nodeId` is null): nothing is asked for it, so the last answer belongs to another node and is not shown.
  const docs = nodeId !== null && listed?.nodeId === nodeId ? listed.docs : [];

  // a doc with a draft is the one shown (the draft is not lost to a switch of doc, or of node); else the chosen doc while it exists, else the first
  const drafts = useDrafts((s) => s.drafts);
  const doc = docs.find((d) => drafts[d.path]?.kind === "doc") ?? docs.find((d) => d.path === chosen) ?? docs[0] ?? null;
  const path = doc?.path ?? null;
  const text = read && read.path === path ? read.text : null;
  const draft = path ? drafts[path] : undefined;
  const editState = editStateOf(path !== null, draft);
  const refused = useDrafts((s) => (path ? s.refused.has(path) : false));
  const busy = useDrafts((s) => (path ? s.elsewhere.has(path) : false));
  const notice = useDrafts((s) => (path ? s.notices[path] : undefined));
  const ref = path && node ? { path, node: node.id } : null;
  const hasDraft = draft !== undefined;

  useEffect(() => {
    if (!path) return;
    let current = true;
    request("docText", { path }).then((t) => current && setRead({ path, text: t }), () => current && setRead({ path, text: null }));
    // typing in the editor tab, or a change on disk: the host sends the new text
    const stop = on("docText", (changed) => changed.path === path && setRead({ path, text: changed.text }));
    return () => {
      current = false;
      stop();
    };
  }, [path, hasDraft]); // and again when a draft starts or ends: a save, a Discard, shows the doc as the host has it now (§8: no index to wait for)

  // what the panel says, for the extension's test suite (as a card does): the doc's state, and whether Edit was refused because the other page has it
  useEffect(() => {
    if (path) post({ type: "editState", key: path, state: editState, elsewhere: busy });
  }, [path, editState, busy]);

  // the test suite cannot click or type in a webview: Edit, typing and Save of the doc it names (the doc's path is the key)
  useEffect(
    () =>
      on("viewCommand", ({ name, key, text }) => {
        if (!path || key !== path) return;
        if (name === "docEdit" && ref) void beginDocEdit(ref);
        else if (name === "docSetDraft" && text !== undefined) typeInto(path, text);
        else if (name === "docSave") void saveDraft(path);
      }),
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [path, ref?.node],
  );

  const html = useMemo(() => (text === null ? "" : renderMarkdown(text)), [text]);

  const file = path ? path.slice(path.lastIndexOf("/") + 1) : "";
  const run: Record<DocButtonId, () => void> = {
    edit: () => ref && void beginDocEdit(ref),
    openInEditor: () => path && void request("openDoc", { path }).catch(() => {}),
    save: () => path && void saveDraft(path),
    discard: () => path && discardDraft(path),
  };
  const BUTTONS: Record<DocButtonId, { icon: string; label: string }> = {
    edit: { icon: "edit", label: "Edit" },
    openInEditor: { icon: "go-to-file", label: "Open in Editor" },
    save: { icon: "save", label: saveTip(platformNow()) },
    discard: { icon: "discard", label: "Discard" },
  };
  const act: Record<DraftAction, () => void> = {
    reload: () => ref && void reloadDocDraft(ref),
    showDiff: () => path && showDiff(path),
    openInEditor: () => path && openInEditor(path),
    copyDraft: () => path && copyDraft(path),
  };
  const line = docLine({ refused, busy, asking: PAGE, state: draft?.state ?? null, file, notice });
  const lineEl =
    line?.kind === "refusal" ? (
      <Refusal pad={16} path={path ?? ""} tip={(f) => `${f} has unsaved changes in an editor tab. Edit needs the saved file.`} onSaveAndEdit={() => ref && void beginDocEdit(ref, true)} />
    ) : line?.kind === "busy" ? (
      <Elsewhere pad={16} text={line.text} tip={`${line.text}: one draft per doc`} />
    ) : line?.kind === "state" ? (
      <StateLine pad={16} text={line.text} actions={line.actions} act={act} />
    ) : line?.kind === "notice" ? (
      <SaveNotice pad={16} notice={line.notice} />
    ) : null;

  const pick = (value: string) => (value === NEW_DOC && node ? onAddDoc(node.id) : setChosen(value));
  const onBodyClick = (e: React.MouseEvent) => {
    const link = (e.target as HTMLElement).closest("a[data-href]");
    if (!link || !path) return;
    e.preventDefault();
    void request("openLink", { href: link.getAttribute("data-href") ?? "", from: path }).catch(() => {});
  };

  // drag the left edge to resize; pointer capture keeps the drag alive over the canvas
  const width = useView((v) => v.docWidth);
  const onResizeMove = (e: React.PointerEvent<HTMLDivElement>) => {
    if (e.buttons !== 1) return;
    setDocWidth(Math.min(MAX_WIDTH, Math.max(MIN_WIDTH, window.innerWidth - e.clientX)));
  };

  return (
    <aside className={`relative flex flex-col bg-background text-[13px] ${fill ? "min-w-0 flex-1" : "flex-none border-l border-border"}`} style={fill ? undefined : { width }}>
      {!fill && (
        <div
          role="separator"
          aria-orientation="vertical"
          aria-label="Resize the doc panel"
          className="absolute inset-y-0 -left-1 z-10 w-2 cursor-col-resize touch-none motion-safe:transition-colors hover:bg-ring/40 active:bg-ring/60"
          onPointerDown={(e) => e.currentTarget.setPointerCapture(e.pointerId)}
          onPointerMove={onResizeMove}
        />
      )}
      <header className="flex flex-none items-center gap-2 border-b border-border px-3 py-2">
        <div className="min-w-0 flex-1">
          <div className="truncate font-semibold">{node?.name ?? "Docs"}</div>
          {node && calledBy && <div className="truncate text-[11px] text-muted-foreground">{calledByText(calledBy)}</div>}
          {node && (
            <div className="truncate font-mono text-[11px] text-muted-foreground" title={node.path}>
              {node.path}
              {node.range ? `:${node.range.start.line + 1}` : ""}
            </div>
          )}
        </div>
        {docs.length > 0 && (
          <Select value={doc?.path ?? ""} items={docs.map((d) => ({ value: d.path, label: d.title }))} disabled={hasDraft} onValueChange={(value) => value && pick(value)}>
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
        {docButtons(editState, draft ? isChanged(draft) : false).map(({ id, disabled }) => (
          <IconButton key={id} size="icon-sm" label={BUTTONS[id].label} disabled={disabled} onClick={run[id]}>
            <Codicon name={BUTTONS[id].icon} />
          </IconButton>
        ))}
        {extra}
        <Button variant="ghost" size="icon-sm" aria-label="Close the doc panel" onClick={onClose}>
          <IconX />
        </Button>
      </header>
      <div className="flex min-h-0 flex-1 flex-col">
        {lineEl}
        {draft ? (
          <CodeEditor text={draft.text} path={draft.path} language="markdown" firstLine={1} editState={editState} onText={(t) => typeInto(draft.key, t)} onSave={() => void saveDraft(draft.key)} />
        ) : (
          <div className="min-h-0 flex-1 overflow-auto px-4 py-4" onClick={onBodyClick}>
            {!node && (
              <div className="mt-10 flex flex-col items-center gap-2 text-center text-muted-foreground">
                <IconPointer className="size-8 opacity-60" />
                <p>{emptyText}</p>
              </div>
            )}
            {node && !loading && docs.length === 0 && (
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
        )}
      </div>
    </aside>
  );
}
