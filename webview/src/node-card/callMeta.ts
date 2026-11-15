// A call card's meta line (docs/16-ui-redesign/04-node-cards.md §2, §2.1): where it is called on the left, where the callee lives on the right. Pure, so the
// width rule is tested with character counts: the line is monospace, so its width is the number of characters times one width.

import { type OpenableSite, siteText } from "@vnoc/shared/calls.ts";
import type { CallSite } from "@vnoc/shared/messages.ts";

/** How many sites are written out before "N more ▾". */
export const SITES_SHOWN = 2;

export interface SiteLink {
  /** `line 16` (the caller's file), `base.py:12` (another file), `line 16 ?` (the driver said no file) */
  text: string;
  /** the place to go; null for a site with no file: it is not a link */
  site: OpenableSite | null;
}

export interface CallMeta {
  /** the first sites, each its own link */
  sites: SiteLink[];
  /** how many more there are (a button that opens the list), 0 when none */
  more: number;
  /** the callee's file name, or its mark, or "not in the index": whole, or null when it does not fit beside the sites */
  right: string | null;
  /** the characters the left side takes, `↳ ` included (what `right` had to fit beside) */
  leftLength: number;
}

const GAP = 2; // the room between the left text and the right one

/**
 * `right`: what goes on the right if there is room. The sites win the room: it is shown whole or not at all, never cut (`payme…` says nothing and
 * looks like a site). `chars`: how many characters the line holds (`metaChars()` of `rows.ts`).
 */
export function callMeta(sites: readonly CallSite[], parentPath: string | null, right: string | null, chars: number): CallMeta {
  const links: SiteLink[] = sites.slice(0, SITES_SHOWN).map((s) => (s.path === null ? { text: `line ${s.start.line + 1} ?`, site: null } : { text: siteText(s as OpenableSite, parentPath), site: s as OpenableSite }));
  const more = Math.max(0, sites.length - SITES_SHOWN);
  const parts = [...links.map((l) => l.text), ...(more > 0 ? [`${more} more ▾`] : [])];
  const leftLength = sites.length === 0 ? 0 : 2 + parts.join(" · ").length; // "↳ " and the parts
  const fits = right !== null && leftLength + (leftLength > 0 ? GAP : 0) + right.length <= chars;
  return { sites: links, more, right: fits ? right : null, leftLength };
}
