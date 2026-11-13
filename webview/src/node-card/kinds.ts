// What differs between a folder, a file, a class and a function. One table, so a new kind is one row and not an edit in every component.
// The icons are codicons, the ones VS Code's Outline uses for the same kinds (docs/16-ui-redesign/04-node-cards.md §5).

import type { Kind } from "@vnoc/shared/messages.ts";

export interface KindStyle {
  /** the codicon's name */
  icon: string;
  /** how the name is written in the header */
  title: (name: string) => string;
  /** the colour of the stripe and the icon: a token from tokens.css */
  color: string;
}

export const KINDS: Record<Kind, KindStyle> = {
  folder: { icon: "folder", title: (n) => `${n}/`, color: "var(--kind-folder)" },
  file: { icon: "file-code", title: (n) => n, color: "var(--kind-file)" },
  class: { icon: "symbol-class", title: (n) => n, color: "var(--kind-class)" },
  function: { icon: "symbol-function", title: (n) => n, color: "var(--kind-function)" },
};
