import tailwindcss from "@tailwindcss/vite";
import vue from "@vitejs/plugin-vue";
import { resolve } from "node:path";
import { defineConfig } from "vite";

// Preview-only: renders the popup in a plain browser with the Tauri runtime
// stubbed out, so the design can be reviewed without building the app.
const root = process.cwd();

export default defineConfig({
  root: resolve(root, "preview"),
  plugins: [vue(), tailwindcss()],
  resolve: {
    alias: [{ find: /^@tauri-apps\/.*/, replacement: resolve(root, "preview/mock.ts") }],
  },
  server: { port: 1430, strictPort: true, fs: { allow: [root] } },
});
