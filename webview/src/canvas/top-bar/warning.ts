// What the bar's ⚠ says. Pure, so it is tested without a browser (docs/16-ui-redesign/02-top-bar.md §3 #4, §4).

export type Level = "none" | "warning" | "error";

export interface WarningInput {
  /** what the last structure said, or null before the first one */
  structure: { truncated: boolean; failed: readonly unknown[] } | null;
  status: "loading" | "ready" | "error";
  /** why the last read failed, when `status` is "error" */
  message: string;
}

/**
 * An error while a structure is drawn is red and wins; with no structure the canvas says it in words and the bar says nothing. Otherwise the
 * icon shows only when the structure is cut or files could not be read.
 */
export function warningOf({ structure, status, message }: WarningInput): { level: Level; text: string } {
  if (!structure) return { level: "none", text: "" };
  if (status === "error") return { level: "error", text: `Could not read the code again: ${message}` };
  const reasons: string[] = [];
  if (structure.truncated) reasons.push("Too many files: the structure is cut");
  const n = structure.failed.length;
  if (n > 0) reasons.push(`${n} ${n === 1 ? "file" : "files"} could not be read`);
  return reasons.length > 0 ? { level: "warning", text: reasons.join(" · ") } : { level: "none", text: "" };
}
