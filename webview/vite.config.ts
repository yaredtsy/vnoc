import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
import { fileURLToPath } from "node:url";

export default defineConfig({
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
