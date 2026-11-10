// The node card: header, body, footer (plan_vs/02-technology/03-frontend-webview.md, "The node card"). Each part is its own component,
// so the later controls (link icon, doc count, code toggle) go into the part they belong to and this file stays a layout.

import { Handle, type Node, type NodeProps, Position } from "@xyflow/react";

import type { Placed } from "../canvas/layout.ts";
import { NodeCode } from "./NodeCode.tsx";
import { NodeBody } from "./NodeBody.tsx";
import { NodeFooter } from "./NodeFooter.tsx";
import { NodeHeader } from "./NodeHeader.tsx";
import "./node-card.css";

export function NodeCard({ data }: NodeProps<Node<Placed>>) {
  return (
    <div
      className={`node-card ${data.kind}${data.syntaxError ? " broken" : ""}${data.codeOpen ? " code-open" : ""}`}
      // VS Code's own right-click menu for a webview: the entries are in package.json (`webview/context`), shown when these keys match
      data-vscode-context={JSON.stringify({ webviewSection: "node", nodeKey: data.id, kind: data.kind, preventDefaultContextMenuItems: true })}
    >
      {/* the edges attach here; nobody connects nodes by hand, so the dots are invisible (node-card.css) */}
      <Handle type="target" position={Position.Left} isConnectable={false} />
      <NodeHeader node={data} />
      {data.codeOpen ? <NodeCode node={data} /> : <NodeBody node={data} />}
      <NodeFooter node={data} />
      <Handle type="source" position={Position.Right} isConnectable={false} />
    </div>
  );
}
