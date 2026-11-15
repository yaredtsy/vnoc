import type { StructureResult } from "@vnoc/shared/messages.ts";

/** The project's numbers: the first crumb's tooltip. */
export const statsLine = (s: StructureResult): string => `${s.stats.folders} folders · ${s.stats.files} files · ${s.stats.classes} classes · ${s.stats.functions} functions`;
