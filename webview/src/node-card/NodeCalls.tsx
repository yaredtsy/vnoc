import { useEffect, useMemo, useState } from "react";

import { Button } from "@/components/ui/button.tsx";
import { cn } from "@/lib/utils.ts";
import { type CallRow, callRows, isStale, markBadge, siteLabel } from "../canvas/calls.ts";
import type { Placed } from "../canvas/layout.ts";
import { post, request } from "../host.ts";
import { useCalls } from "../state/calls.ts";
import { useData } from "../state/data.ts";
import { dispatch } from "../state/session.ts";
import { KINDS } from "./kinds.ts";

/**
 * The "Calls" section of the card a call tree was made for: what the entry calls, as indented rows (docs/14 §7). A row whose node the project knows
 * selects it when clicked; one the project does not know (a library) is grey and does nothing. A row's call sites open the parent's file at that line.
 * When the index has moved on since the tree was made it says so and offers Refresh; it never runs again by itself.
 */
export function NodeCalls({ node }: { node: Placed }) {
  const tree = useCalls((s) => s.trees[node.id]);
  const revision = useData((s) => s.structure?.revision ?? 0);
  const rows = useMemo(() => (tree ? callRows(tree.root) : []), [tree]);
  const stale = tree ? isStale(tree, revision) : false;
  const [sitesOf, setSitesOf] = useState<string | null>(null); // the row whose call sites are listed
  const [refreshing, setRefreshing] = useState(false);

  useEffect(() => setRefreshing(false), [tree]);
  useEffect(() => setSitesOf(null), [tree]);
  // tell the host what was drawn: the extension's test suite uses it
  useEffect(() => {
    if (tree) post({ type: "callsDrawn", key: node.id, rows: rows.length, stale });
  }, [tree, rows.length, stale, node.id]);

  if (!tree) return null;
  const refresh = () => {
    setRefreshing(true);
    request("showCallTree", { key: node.id }).catch(() => {}).finally(() => setRefreshing(false));
  };
  return (
    <div className="node-calls nodrag nopan nowheel" aria-label={`Calls of ${node.name}`}>
      <div className="calls-head">
        <span className="calls-title">Calls</span>
        {tree.root.marks.map((m) => (
          <span key={m} className="call-mark" title={markBadge(m).title}>
            {markBadge(m).text}
          </span>
        ))}
        {stale && (
          <>
            <span className="calls-stale">the code changed</span>
            <Button variant="ghost" size="xs" className="h-[18px] px-1.5" disabled={refreshing} onClick={refresh}>
              {refreshing ? "Refreshing…" : "Refresh"}
            </Button>
          </>
        )}
      </div>
      <ul className="calls-rows">
        {rows.length === 0 && <li className="call-empty">calls nothing in this project</li>}
        {rows.map((row) => (
          <Row key={row.id} row={row} open={sitesOf === row.id} onSites={() => setSitesOf(sitesOf === row.id ? null : row.id)} />
        ))}
      </ul>
    </div>
  );
}

function Row({ row, open, onSites }: { row: CallRow; open: boolean; onSites: () => void }) {
  const { node } = row;
  const known = node.key !== null;
  const sites = row.parentPath === null ? [] : node.call_sites;
  return (
    <li className={cn("call-row", !known && "unknown")} style={{ paddingLeft: 6 + row.depth * 12 }}>
      <div className="call-line">
        <span className="call-kind">{KINDS[node.kind].icon}</span>
        {known ? (
          <button className="call-name" title={`Select ${node.qualname}`} onClick={() => dispatch({ type: "select", id: node.key! })}>
            {row.label}
          </button>
        ) : (
          <span className="call-name" title={`${node.qualname}: not in this project's structure`}>
            {row.label}
          </span>
        )}
        {row.marks.map((m) => (
          <span key={m.text} className="call-mark" title={m.title}>
            {m.text}
          </span>
        ))}
        {sites.length > 0 && (
          <button className="call-sites-toggle" aria-expanded={open} title="Where it is called" onClick={onSites}>
            {open ? "▾" : "▸"} {sites.length === 1 ? "1 site" : `${sites.length} sites`}
          </button>
        )}
      </div>
      {open && (
        <div className="call-sites">
          {sites.map((site) => (
            <button key={`${site.start.line}:${site.start.character}`} onClick={() => void request("openAt", { path: row.parentPath!, range: site }).catch(() => {})}>
              {siteLabel(site)}
            </button>
          ))}
        </div>
      )}
    </li>
  );
}
