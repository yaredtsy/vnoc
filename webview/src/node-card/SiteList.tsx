import { useEffect, useState } from "react";

import { siteLabel } from "@vnoc/shared/calls.ts";
import type { OpenableSite } from "@vnoc/shared/calls.ts";
import { SITE_LIST_PAD, SITE_ROW_H, type Placed } from "../canvas/layout.ts";
import { request } from "../host.ts";
import { NAME_X } from "./rows.ts";
import { goToSite } from "./goToSite.ts";

/**
 * Every site of a call card, one line each: `line 40  stock = reserve_stock(item, qty)`, the line's text read by the host (04 §2.1), so you see which
 * call is which before you go there. Its height is `siteListHeight`, counted by the layout.
 */
export function SiteList({ node }: { node: Placed }) {
  const sites = node.call!.sites;
  const [texts, setTexts] = useState<Record<number, string>>({});
  useEffect(() => {
    let live = true;
    sites.forEach((site, i) => {
      if (site.path === null) return;
      const range = { start: { line: site.start.line, character: 0 }, end: { line: site.start.line + 1, character: 0 } };
      request("code", { path: site.path, range }).then((got) => live && setTexts((t) => ({ ...t, [i]: got.text.trim() })), () => {});
    });
    return () => {
      live = false;
    };
  }, [sites]);
  return (
    <ul className="nodrag nopan nowheel m-0 flex-none list-none overflow-hidden border-t border-[color:var(--card-border)] p-0 font-mono text-[11px]" style={{ paddingLeft: NAME_X, paddingTop: SITE_LIST_PAD / 2, paddingBottom: SITE_LIST_PAD / 2 }}>
      {sites.map((site, i) => (
        <li key={i} className="flex items-center gap-2 whitespace-nowrap" style={{ height: SITE_ROW_H }}>
          {site.path === null ? (
            <span className="text-muted-foreground" title="the driver did not say which file">{siteLabel(site)} ?</span>
          ) : (
            <button type="button" className="cursor-pointer text-[color:var(--link)] hover:underline" onClick={() => goToSite(site as OpenableSite)}>
              {siteLabel(site)}
            </button>
          )}
          <span className="min-w-0 truncate text-muted-foreground">{texts[i] ?? ""}</span>
        </li>
      ))}
    </ul>
  );
}
