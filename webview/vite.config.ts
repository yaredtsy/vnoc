import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

export default defineConfig({
  plugins: [react()],
  build: {
    outDir: "../extension/media/webview",
    emptyOutDir: true,
    rollupOptions: {
      input: "src/main.tsx",
      output: { entryFileNames: "assets/index.js", assetFileNames: "assets/index[extname]" },
    },
  },
});
