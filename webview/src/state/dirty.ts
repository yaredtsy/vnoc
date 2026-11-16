// The files with unsaved changes (docs/17-edit-in-canvas.md step 9.1), as the host knows them: the cards that show code mark it. Not saved: the host knows it
// at every start. Asked once when the page starts, then the host sends the whole set whenever it changes.

import { create } from "zustand";

import { on, request } from "../host.ts";

/** `paths`: relative to the opened folder, as `Placed.path`. A card reads `(s) => s.paths.has(path)`, so it draws again only when its own file flips. */
export const useDirtyFiles = create<{ paths: ReadonlySet<string> }>(() => ({ paths: new Set() }));

const accept = (paths: string[]) => useDirtyFiles.setState({ paths: new Set(paths) });

request("dirtyFiles").then(({ paths }) => accept(paths), () => {});
on("dirtyFiles", ({ paths }) => accept(paths));
