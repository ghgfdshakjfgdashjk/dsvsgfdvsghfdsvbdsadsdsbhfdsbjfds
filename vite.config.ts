import { defineConfig } from "vite";
import { resolve, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));

const host = process.env.TAURI_DEV_HOST;

export default defineConfig(async () => ({

  clearScreen: false,

  build: {
    rollupOptions: {
      // the overlay is its own window, so it is its own page
      input: {
        main: resolve(here, "index.html"),
        overlay: resolve(here, "overlay.html"),
      },
    },
  },

  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1421,
        }
      : undefined,
    watch: {

      ignored: ["**/src-tauri/**"],
    },
  },
}));
