import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
import { fileURLToPath } from "node:url";

export default defineConfig({
  // relative URLs: a webview's root is not the extension's folder, so `/assets/x.ttf` in the CSS would never load (the codicon font did not)
  base: "./",
  plugins: [react(), tailwindcss()],
  resolve: { alias: { "@": fileURLToPath(new URL("./src", import.meta.url)) } },
  build: {
    outDir: "../extension/media/webview",
    emptyOutDir: true,
    rollupOptions: {
      input: "src/main.tsx",
      output: { entryFileNames: "assets/index.js", assetFileNames: "assets/index[extname]" },
    },
  },
});
