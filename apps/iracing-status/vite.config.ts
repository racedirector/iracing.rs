import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// @ts-expect-error process is a nodejs global
const host = process.env.TAURI_DEV_HOST;
// @ts-expect-error process is a nodejs global
const projectRoot = process.cwd().replace(/\\/g, "/");
const reactNativeWebEntry = `${projectRoot}/node_modules/react-native-web/dist/index.js`;

// https://vite.dev/config/
export default defineConfig(async () => ({
  plugins: [react()],
  resolve: {
    alias: [{ find: /^react-native$/, replacement: reactNativeWebEntry }],
    extensions: [
      ".web.tsx",
      ".web.ts",
      ".web.jsx",
      ".web.js",
      ".mjs",
      ".js",
      ".ts",
      ".jsx",
      ".tsx",
      ".json",
    ],
  },
  optimizeDeps: {
    exclude: [
      "react-native",
      "react-native-pager-view",
      "react-native-safe-area-context",
      "react-native-screens",
    ],
    esbuildOptions: {
      resolveExtensions: [
        ".web.tsx",
        ".web.ts",
        ".web.jsx",
        ".web.js",
        ".mjs",
        ".js",
        ".ts",
        ".jsx",
        ".tsx",
        ".json",
      ],
    },
  },

  // Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
  //
  // 1. prevent Vite from obscuring rust errors
  clearScreen: false,
  // 2. tauri expects a fixed port, fail if that port is not available
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
      // 3. tell Vite to ignore watching `src-tauri`
      ignored: ["**/src-tauri/**"],
    },
  },
}));
