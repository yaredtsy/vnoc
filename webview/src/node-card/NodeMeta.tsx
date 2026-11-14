import type { Placed } from "../canvas/layout.ts";
import { META_H } from "../canvas/layout.ts";
import { metaOf } from "./metaOf.ts";
import { NAME_X } from "./rows.ts";

/**
 * Where it is, relative to the card it hangs from (`metaOf`); the full path is the tooltip. A root's path is cut at the START when it does not fit, so the
 * file name stays. `<bdi>` keeps the text in its own order inside the right-to-left box.
 */
export function NodeMeta({ node }: { node: Placed }) {
  const meta = metaOf(node);
  return (
    <div className="flex flex-none items-center pr-3 font-mono text-[11px] text-muted-foreground" style={{ height: META_H, paddingLeft: NAME_X }}>
      <span className={`min-w-0 flex-1 truncate text-left [direction:rtl] ${meta.error ? "text-destructive" : ""}`} title={meta.tip}>
        <bdi>{meta.text}</bdi>
      </span>
    </div>
  );
}
