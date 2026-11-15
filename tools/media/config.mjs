// Pinned versions, sizes and paths. Everything else in this tool reads them from here.
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

export const here = dirname(fileURLToPath(import.meta.url));
export const repoRoot = join(here, "../..");

/** The code shown in the pictures: httpx (BSD-3-Clause), at a tag. `demo/PIN` repeats tag and commit for a person to read. */
export const httpx = {
  url: "https://github.com/encode/httpx",
  tag: "0.28.1",
  commit: "26d48e0634e6ee9cdc0533996db289ce4b430177",
};

/** The VS Code the pictures are taken with: the one the window suite uses (extension/.vscode-test). */
export const vscodeVersion = "1.146.0";

/** The window, in logical pixels. On a Retina Mac the screenshots are 2x of this and the GIFs 1x. */
export const window = { width: 1280, height: 800 };

export const paths = {
  /** everything made while running: the clone, the profile, the frames. Outside this repository, so the clone is not inside its git repo. */
  work: join(tmpdir(), "vnoc-media"),
  /** the extension for this Mac, as `node scripts/package.mjs` writes it */
  vsix: join(repoRoot, `vnoc-${process.platform}-${process.arch}.vsix`),
  /** what is kept: `tools/media/out/` is git-ignored */
  out: join(here, "out"),
  /** what is committed */
  screenshots: join(repoRoot, "extension/media/screenshots"),
};
