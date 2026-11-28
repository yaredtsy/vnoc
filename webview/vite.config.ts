import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
import { fileURLToPath } from "node:url";

// Two bundles into one folder (docs/16-ui-redesign/06-sidebar.md §10): the canvas and the docs tab (`vite build`), and the STRUCTURE view of the side bar
// (`vite build --mode sidebar`: no React Flow, no Monaco, no Shiki). The sidebar is built on its own, with every import inlined, so its one script needs
// no sibling chunk: a webview's CSP lets a page run only the script that carries its nonce. `pnpm build` empties the folder, then runs both.
export default defineConfig(({ mode }) => {
  const sidebar = mode === "sidebar";
  return {
    // relative URLs: a webview's root is not the extension's folder, so `/assets/x.ttf` in the CSS would never load (the codicon font did not)
    base: "./",
    plugins: [react(), tailwindcss()],
    resolve: { alias: { "@": fileURLToPath(new URL("./src", import.meta.url)) } },
    build: {
      outDir: "../extension/media/webview",
      emptyOutDir: false,
      // no `<link rel=modulepreload>`: a webview's CSP lets a script run only with the page's nonce, and the preload tags Vite adds for a lazy chunk's imports (Mermaid's diagrams)
      // carry none, so they are blocked. A plain `import()` inherits the nonce of the script that made it, which is how the Shiki chunks load.
      modulePreload: false,
      rollupOptions: {
        input: sidebar ? "src/sidebar/main.tsx" : "src/main.tsx",
        output: sidebar
          ? { entryFileNames: "assets/sidebar.js", assetFileNames: "assets/sidebar[extname]", inlineDynamicImports: true }
          : { entryFileNames: "assets/index.js", assetFileNames: "assets/index[extname]" },
      },
    },
  };
});
