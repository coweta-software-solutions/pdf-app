import { svelte } from "@sveltejs/vite-plugin-svelte";
import { defineConfig } from "vite-plus";

export default defineConfig({
  plugins: [svelte()],
  server: {
    port: 5173,
    proxy: {
      "/health": "http://127.0.0.1:3000",
      "/convert": "http://127.0.0.1:3000",
      "/merge": "http://127.0.0.1:3000",
      "/split": "http://127.0.0.1:3000",
      "/jobs": "http://127.0.0.1:3000",
    },
  },
  build: {
    outDir: "dist",
    emptyOutDir: true,
  },
});
