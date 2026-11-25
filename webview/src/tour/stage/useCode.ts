import { useEffect, useState } from "react";

import { request } from "../../host.ts";
import { useDirtyFiles } from "../../state/dirty.ts";

/** The shown code of a node: its lines, the file line the first one is, and the text as the host read it (the first line with the indent before the range). */
export type Shown = { first: number; lines: string[]; text: string } | { error: string } | null;

const WHOLE_FILE = { start: { line: 0, character: 0 }, end: { line: 1_000_000, character: 0 } };

/**
 * The code of the stop's node as it is now, from the host's `code` request (the one a card's `</>` uses: VS Code's document, so unsaved edits show). `path` is below
 * the opened folder and `range` the node's (a file has none: the whole file). It is read again when the range moves or the file is saved. Null while it is read, and
 * for a node the structure does not have.
 */
export function useCode(path: string | null, range: { start: { line: number; character: number }; end: { line: number; character: number } } | null, wanted: boolean): Shown {
  const [shown, setShown] = useState<Shown>(null);
  const dirty = useDirtyFiles((s) => (path === null ? false : s.paths.has(path)));
  const key = range ? `${range.start.line}:${range.start.character}-${range.end.line}:${range.end.character}` : "";
  useEffect(() => {
    if (!wanted || path === null) return setShown(null);
    let live = true;
    request("code", { path, range: range ?? WHOLE_FILE }).then(
      (got) => {
        if (!live) return;
        const text = got.prefix + got.text;
        setShown({ first: range ? range.start.line + 1 : 1, lines: text.split(/\r?\n/), text });
      },
      (e: Error) => live && setShown({ error: e.message }),
    );
    return () => {
      live = false;
    };
  }, [path, key, wanted, dirty]); // eslint-disable-line react-hooks/exhaustive-deps -- `range` is `key`
  return shown;
}
