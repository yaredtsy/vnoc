import type { TreeNode } from "@vnoc/shared/messages.ts";
import type { TourStop } from "@vnoc/shared/tours.ts";
import { contentsText } from "../../node-card/metaOf.ts";
import { KINDS } from "../../node-card/kinds.ts";
import { Codicon } from "@/components/Codicon.tsx";

const count = (n: TreeNode) => {
  const c = { folders: 0, files: 0, classes: 0, functions: 0 };
  for (const child of n.children) c[({ folder: "folders", file: "files", class: "classes", function: "functions" } as const)[child.kind]]++;
  return c;
};

/**
 * A stop with no code to light (design 03 §3.1): the node's children as rows, with the card's own meta line. A child that is a stop of this tour has its number and a
 * click jumps to it (the stop whose `key` is the child's: a lookup in what the core sent); one that is not is dim. (The design folds more than eight dim rows into
 * `+ N not on this tour`: not built, the rows scroll.)
 */
export function Contents(p: { node: TreeNode | null; stops: readonly TourStop[]; onJump: (stop: string) => void }) {
  if (!p.node) return <div className="stage-code stage-empty">This is no longer in the code.</div>;
  return (
    <div className="stage-code stage-contents" data-part="contents">
      {p.node.children.map((child) => {
        const stop = p.stops.find((s) => s.key === child.id);
        const kind = KINDS[child.kind];
        return (
          <button key={child.id} type="button" className={`stage-row${stop ? "" : " stage-dim"}`} disabled={!stop} onClick={() => stop && p.onJump(stop.id)} data-child={child.id}>
            <span className="stage-gutter">{stop ? stop.number : ""}</span>
            <span style={{ color: kind.color }}>
              <Codicon name={kind.icon} className="text-[12px]" />
            </span>
            <span className="stage-rowname">{kind.title(child.name)}</span>
            <span className="stage-rowmeta">{child.kind === "folder" || child.kind === "file" ? contentsText(child.kind, count(child)) : ""}</span>
          </button>
        );
      })}
      {p.node.children.length === 0 && <div className="stage-empty">empty</div>}
    </div>
  );
}
