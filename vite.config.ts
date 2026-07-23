import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";

const host = process.env.TAURI_DEV_HOST;

export default defineConfig({
  plugins: [svelte()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1421
        }
      : undefined,
    watch: {
      ignored: ["**/src-tauri/**"]
    }
  },
  build: {
    target: ["es2021", "chrome105", "safari13"],
    cssMinify: true,
    sourcemap: false,
    rollupOptions: {
      input: {
        main: "index.html",
        widget: "widget.html"
      }
    }
  }
});
