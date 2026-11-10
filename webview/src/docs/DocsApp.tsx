// The docs tab beside a code editor (07-ui/02-design.md §3, "Docs beside the code"): the doc panel alone. The host decides which node it shows
// (the one at the cursor in the editor) and sends it as `showNode`; the pin stops the host from following the cursor.

import { useCallback, useEffect, useState } from "react";

import { IconPin, IconPinFilled } from "@tabler/icons-react";

import { Toggle } from "@/components/ui/toggle.tsx";
import { onIndexChanged, onShowNode, request, vscode } from "../host.ts";
import { findNode } from "../tree.ts";
import type { StructureResult } from "../types.ts";
import { DocPanel } from "./DocPanel.tsx";

export function DocsApp() {
  const [structure, setStructure] = useState<StructureResult | null>(null);
  const [key, setKey] = useState<string | null>(null);
  const [pinned, setPinned] = useState(false);

  // The newest revision shown: two asks can cross, and the older answer must not replace the newer one.
  const refresh = useCallback(() => {
    request<StructureResult>("structure").then((data) => setStructure((current) => (current && data.revision < current.revision ? current : data)), () => {});
  }, []);
  useEffect(() => {
    refresh();
    // a tab opened by "Show docs" asks for its node; after that the host pushes changes
    request<string | null>("docsTarget").then(setKey, () => {});
    const stops = [onIndexChanged(refresh), onShowNode(setKey)];
    return () => stops.forEach((stop) => stop());
  }, [refresh]);

  const togglePin = (pressed: boolean) => {
    setPinned(pressed);
    vscode.postMessage({ type: "pin", pinned: pressed });
  };
  const node = structure && key ? findNode(structure.root, key) : null;
  const addDoc = (id: string) => void request("addDoc", { key: id, docs: node?.docs ?? 0 }).catch(() => {});

  return (
    <div className="flex h-screen">
      <DocPanel
        fill
        node={node}
        revision={structure?.revision ?? 0}
        onAddDoc={addDoc}
        onClose={() => vscode.postMessage({ type: "close" })}
        extra={
          <Toggle size="sm" pressed={pinned} aria-label="Keep showing this node" title={pinned ? "Following is off: click to follow the cursor again" : "Stop following the cursor"} onPressedChange={togglePin}>
            {pinned ? <IconPinFilled /> : <IconPin />}
          </Toggle>
        }
      />
    </div>
  );
}
