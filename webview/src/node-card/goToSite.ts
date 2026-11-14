import type { OpenableSite } from "@vnoc/shared/calls.ts";
import { request } from "../host.ts";

/** Opens the file a call site names at that line (beside the canvas when it is in front: the host decides). */
export const goToSite = (site: OpenableSite) => void request("openAt", { path: site.path, range: { start: site.start, end: site.end } }).catch(() => {});

/** Go to Call Site (Ctrl/Cmd+Enter): the one site goes there; several open the host's list, which previews each as it is moved through. */
export function goToCallSite(sites: readonly OpenableSite[]): void {
  if (sites.length === 1) return goToSite(sites[0]);
  void request("pickCallSite", { sites: sites.map((s) => ({ path: s.path, range: { start: s.start, end: s.end } })) }).catch(() => {});
}
