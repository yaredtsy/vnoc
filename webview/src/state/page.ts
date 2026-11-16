// Which page this is: the bundle is the canvas, or the docs tab beside an editor (`data-mode` on the root; the STRUCTURE view is its own bundle and holds no drafts). The host keeps one list of
// drafts per page (docs/17-edit-in-canvas.md §8, 9.6), and one doc is edited on one page only.

import type { Page } from "@vnoc/shared/messages.ts";

export const PAGE: Page = document.getElementById("root")?.dataset.mode === "docs" ? "docs" : "canvas";
