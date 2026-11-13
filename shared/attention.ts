// The rows of NEEDS ATTENTION's "Not read" group (docs/11-missing-relink-remove.md §6, docs/16-ui-redesign/02-top-bar.md §3 #4): the files the driver
// could not read, and the "too many files" row. They come from the structure the extension holds, not from the core's attention list.
// Pure: no `vscode`, tested with `node --test`.

import type { StructureResult } from "./messages";

export type NotReadRow =
  | { kind: "file"; path: string; /** the row's label */ label: string; /** the row's description: why, in a few words */ reason: string; /** the full error */ tooltip: string }
  | { kind: "truncated"; label: string; tooltip: string };

const REASON_MAX = 60;

/** The error of a failed file in a few words: "not UTF-8" for the usual one, else its first line without the "cannot read x:" lead-in. */
export function shortReason(error: string): string {
  if (/valid UTF-8|invalid utf-?8/i.test(error)) return "not UTF-8";
  const first = error.split("\n")[0].replace(/^cannot read [^:]*:\s*/, "").trim();
  if (first === "") return "could not be read";
  return first.length > REASON_MAX ? `${first.slice(0, REASON_MAX - 1)}…` : first;
}

/** One row per failed file, in the order the core gave them, then a "too many files" row when the structure was cut. */
export function notReadRows(s: Pick<StructureResult, "failed" | "truncated"> & { stats?: Pick<StructureResult["stats"], "files"> }): NotReadRow[] {
  const rows: NotReadRow[] = s.failed.map((f) => ({ kind: "file", path: f.path, label: f.path, reason: shortReason(f.error), tooltip: f.error }));
  if (s.truncated) {
    const read = s.stats ? `${s.stats.files} files were read; ` : "";
    rows.push({ kind: "truncated", label: "too many files: the structure is cut", tooltip: `${read}V-NOC stopped there, so the files after it are not on the canvas or in the sidebar.` });
  }
  return rows;
}
