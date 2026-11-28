// Go to Code for a stop (design 03 §2.1, 04 §1.3, §7): the real file in an editor tab, the cursor on the step's first line (else the stop's first). The stage's `↗`, the
// map card's `↗` and the route's hover `↗` and double-click all come here. A folder is revealed in the Explorer. The node is the structure's, found by the stop's `key`.

import { findNode } from "@vnoc/shared/tree.ts";
import type { TreeNode } from "@vnoc/shared/messages.ts";
import type { StepView, TourStop } from "@vnoc/shared/tours.ts";
import { request } from "../host.ts";

export function goToStop(root: TreeNode | null, stop: TourStop, step?: StepView | null): void {
  const node = root && stop.key ? findNode(root, stop.key) : undefined;
  if (!node) return;
  if (node.kind === "folder") return void request("goToCode", { key: node.id }).catch(() => {});
  const line = (step?.lines?.[0] ?? (node.range ? node.range.start.line + 1 : 1)) - 1;
  void request("openAt", { path: node.path, range: { start: { line, character: 0 }, end: { line, character: 0 } } }).catch(() => {});
}
