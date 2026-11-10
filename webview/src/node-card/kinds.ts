// What differs between a folder, a file, a class and a function. One table, so a new kind is one row and not an edit in every component.

import type { Kind } from "@vnoc/shared/messages.ts";

export interface KindStyle {
  icon: string;
  /** shown in the body: "Function top" */
  label: string;
  /** how the name is written in the header */
  title: (name: string) => string;
}

export const KINDS: Record<Kind, KindStyle> = {
  folder: { icon: "▸", label: "Folder", title: (n) => `${n}/` },
  file: { icon: "▤", label: "File", title: (n) => n },
  class: { icon: "◆", label: "Class", title: (n) => n },
  function: { icon: "ƒ", label: "Function", title: (n) => `${n}()` },
};
