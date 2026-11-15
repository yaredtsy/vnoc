import { Codicon } from "@/components/Codicon.tsx";
import { META_H, type Placed } from "../canvas/layout.ts";
import { toggleSites } from "../state/view.ts";
import { callMeta } from "./callMeta.ts";
import { goToSite } from "./goToSite.ts";
import { NAME_X, metaChars } from "./rows.ts";

const fileName = (path: string) => path.slice(path.lastIndexOf("/") + 1);

/**
 * A call card's meta line (04 §2): `↳` and where it is called, each site its own link, then on the right the callee's file name, or its mark, or *not in
 * the index*, whole or not at all (`callMeta`). The line's font is monospace, so what fits is a count of characters.
 */
export function CallMeta({ node }: { node: Placed }) {
  const call = node.call!;
  const { look } = call;
  const right = look.unknown ? "not in the index" : look.mark ? look.mark.text : node.path ? fileName(node.path) : null;
  const meta = callMeta(call.sites, call.callerPath, right, metaChars());
  return (
    <div className="flex flex-none items-center gap-2 pr-3 font-mono text-[11px] text-muted-foreground" style={{ height: META_H, paddingLeft: NAME_X }}>
      {meta.leftLength > 0 && (
        <span className="flex min-w-0 items-center gap-1 whitespace-nowrap">
          <Codicon name="call-outgoing" className="text-[11px]" />
          {meta.sites.map((s, i) => (
            <span key={i} className="flex items-center gap-1">
              {i > 0 && <span aria-hidden>·</span>}
              {s.site ? (
                <button type="button" className="nodrag nopan cursor-pointer text-[color:var(--link)] hover:underline" title={`Go to ${s.text} in ${s.site.path}`} onClick={() => goToSite(s.site!)}>
                  {s.text}
                </button>
              ) : (
                <span title="the driver did not say which file">{s.text}</span>
              )}
            </span>
          ))}
          {meta.more > 0 && (
            <>
              <span aria-hidden>·</span>
              <button type="button" className="nodrag nopan cursor-pointer text-[color:var(--link)] hover:underline" title="Show every site" aria-expanded={node.siteRows > 0} onClick={() => toggleSites(node.id)}>
                {meta.more} more ▾
              </button>
            </>
          )}
        </span>
      )}
      <span className="min-w-0 flex-1" />
      {meta.right && (
        <span className="flex-none" title={look.unknown ? "the project does not have this code (a library, an ignored file)" : look.mark?.title ?? node.path}>
          {meta.right}
        </span>
      )}
    </div>
  );
}
