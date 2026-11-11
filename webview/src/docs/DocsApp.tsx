// The docs tab beside a code editor (07-ui/02-design.md §3, "Docs beside the code"): the doc panel alone. Unpinned, it shows whichever changed last:
// the node the text cursor is in (`session.cursor`, widened to the innermost node around it that has docs), or the card selected on the canvas
// (`session.selected`, exactly that node). The pin holds it on one node. A tab opened by "Show Docs" on a row or a card starts pinned to that
// node. The pin is this tab's own state: nothing else needs to know.

import { useCallback, useEffect, useMemo, useState } from "react";

import { IconPin, IconPinFilled } from "@tabler/icons-react";

import { Toggle } from "@/components/ui/toggle.tsx";
import type { StructureResult } from "@vnoc/shared/messages.ts";
import { MISSING_PIN, docTarget, findNode, missingAsNode, pathTo } from "@vnoc/shared/tree.ts";
import { on, post, request } from "../host.ts";
import { useSession } from "../state/session.ts";
import { pin, useView } from "../state/view.ts";
import { DocPanel } from "./DocPanel.tsx";

export function DocsApp() {
  const [structure, setStructure] = useState<StructureResult | null>(null);
  const cursor = useSession((s) => s.cursor);
  const selected = useSession((s) => s.contexts[s.active].selected);

  // Which of the two moved last. Kept by comparing during the render (React's way to derive state from changing values, no effect needed).
  const [seen, setSeen] = useState({ cursor, selected, source: selected && !cursor ? ("selection" as const) : ("cursor" as const) });
  if (seen.cursor !== cursor || seen.selected !== selected) {
    setSeen({ cursor, selected, source: seen.selected !== selected ? "selection" : "cursor" });
  }

  // the node it is held on (a tab opened by "Show Docs" on a row or a card starts pinned; saved with the tab by state/view.ts)
  const pinned = useView((v) => v.pinned);

  // The newest revision shown: two asks can cross, and the older answer must not replace the newer one.
  const refresh = useCallback(() => {
    request("structure").then((data) => setStructure((current) => (current && data.revision < current.revision ? current : data)), () => {});
  }, []);
  useEffect(() => {
    refresh();
    const stops = [on("indexChanged", refresh), on("showNode", ({ id }) => pin(id))];
    return () => stops.forEach((stop) => stop());
  }, [refresh]);

  // the pin; else what moved last. The cursor node is widened to what has docs (a folder is never the answer there: the tab follows the code).
  const followed = useMemo(() => {
    if (!structure) return null;
    if (seen.source === "selection" && seen.selected) return findNode(structure.root, seen.selected) ?? null;
    if (!seen.cursor) return null;
    return docTarget((pathTo(structure.root, seen.cursor) ?? []).filter((n) => n.kind !== "folder")) ?? null;
  }, [structure, seen]);
  // a Missing node is not in the tree: its pin carries the permanent ID, and the structure lists it
  const pinnedNode = !structure || !pinned ? null : pinned.startsWith(MISSING_PIN) ? ((m) => (m ? missingAsNode(m) : null))(structure.missing.find((m) => MISSING_PIN + m.id === pinned)) : findNode(structure.root, pinned);
  const node = pinnedNode ?? followed;
  const addDoc = (key: string) => void request("addDoc", { key }).catch(() => {});

  return (
    <div className="flex h-screen">
      <DocPanel
        fill
        emptyText="Put the cursor in a function or class, or click a node on the canvas, to see its docs here."
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
            title={pinned ? "Pinned: click to follow the cursor and the canvas again" : "Stop following the cursor and the canvas"}
            onPressedChange={(pressed) => pin(pressed && node ? node.id : null)}
          >
            {pinned ? <IconPinFilled /> : <IconPin />}
          </Toggle>
        }
      />
    </div>
  );
}
