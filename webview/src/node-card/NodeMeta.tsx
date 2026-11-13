import type { Placed } from "../canvas/layout.ts";
import { whereOf } from "./whereOf.ts";

/**
 * Where it lives (`whereOf`). When it does not fit it is cut at the START, so the file name and the line stay (the folders are in the tooltip).
 * `<bdi>` keeps the text in its own order inside the right-to-left box.
 */
export function NodeMeta({ node }: { node: Placed }) {
  const where = whereOf(node);
  return (
    <div className="flex h-[22px] flex-none items-center gap-2 px-3 font-mono text-[11px] text-muted-foreground">
      <span className="min-w-0 flex-1 truncate text-left [direction:rtl]" title={where}>
        <bdi>{where}</bdi>
      </span>
      {node.syntaxError && <span className="flex-none text-destructive">syntax error</span>}
    </div>
  );
}
