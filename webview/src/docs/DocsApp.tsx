// The docs tab beside a code editor (07-ui/02-design.md §3, "Docs beside the code"): the doc panel alone. It shows the node the text cursor is in
// (`session.cursor`, widened to the innermost node around it that has docs), until the pin holds it on one node. A tab opened by "Show Docs" on a
// row or a card starts pinned to that node. The pin is this tab's own state: nothing else needs to know.

import { useCallback, useEffect, useMemo, useState } from "react";

import { IconPin, IconPinFilled } from "@tabler/icons-react";

import { Toggle } from "@/components/ui/toggle.tsx";
import type { StructureResult } from "@vnoc/shared/messages.ts";
import { docTarget, findNode, pathTo } from "@vnoc/shared/tree.ts";
import { on, post, request, vscode } from "../host.ts";
import { useSession } from "../state/session.ts";
import { DocPanel } from "./DocPanel.tsx";

export function DocsApp() {
  const [structure, setStructure] = useState<StructureResult | null>(null);
  const cursor = useSession((s) => s.cursor);
  // the node it was opened for (the page carries it: a message sent while the page loads could be lost), else the one saved with the tab
  const [pinned, setPinned] = useState<string | null>(() => document.getElementById("root")?.dataset.node ?? vscode.getState<{ pinned?: string }>()?.pinned ?? null);

  // The newest revision shown: two asks can cross, and the older answer must not replace the newer one.
  const refresh = useCallback(() => {
    request("structure").then((data) => setStructure((current) => (current && data.revision < current.revision ? current : data)), () => {});
  }, []);
  useEffect(() => {
    refresh();
    const stops = [on("indexChanged", refresh), on("showNode", ({ id }) => setPinned(id))];
    return () => stops.forEach((stop) => stop());
  }, [refresh]);
  useEffect(() => {
    vscode.setState({ pinned: pinned ?? undefined }); // in braces: `setState` returns the state, and React would take that for a cleanup function
  }, [pinned]);

  // the pin, else the cursor node widened to what has docs (a folder is never the answer: the tab follows the code)
  const followed = useMemo(() => {
    if (!structure || !cursor) return null;
    return docTarget((pathTo(structure.root, cursor) ?? []).filter((n) => n.kind !== "folder")) ?? null;
  }, [structure, cursor]);
  const node = (structure && pinned ? findNode(structure.root, pinned) : null) ?? followed;
  const addDoc = (key: string) => void request("addDoc", { key }).catch(() => {});

  return (
    <div className="flex h-screen">
      <DocPanel
        fill
        emptyText="Put the cursor in a function or class to see its docs here."
        node={node}
        revision={structure?.revision ?? 0}
        onAddDoc={addDoc}
        onClose={() => post({ type: "close" })}
        extra={
          <Toggle
            size="sm"
            pressed={pinned !== null}
            disabled={!node}
            aria-label="Keep showing this node"
            title={pinned ? "Pinned: click to follow the cursor again" : "Stop following the cursor"}
            onPressedChange={(pressed) => setPinned(pressed && node ? node.id : null)}
          >
            {pinned ? <IconPinFilled /> : <IconPin />}
          </Toggle>
        }
      />
    </div>
  );
}
