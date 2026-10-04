import path from "node:path";
import { fileURLToPath } from "node:url";

import { paraglideVitePlugin } from "@inlang/paraglide-js";
import tailwindcss from "@tailwindcss/vite";
import { tanstackRouter } from "@tanstack/router-plugin/vite";
import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";

const __dirname = path.dirname(fileURLToPath(import.meta.url));

export default defineConfig({
  plugins: [
    tanstackRouter({ target: "react", autoCodeSplitting: true }),
    paraglideVitePlugin({ project: "./project.inlang" }),
    // The React Compiler, through oxc rather than Babel.
    react({ compiler: true }),
    tailwindcss(),
  ],

  // The Tauri CLI prints the Rust build's errors to the same terminal.
  clearScreen: false,

  // `tauri.conf.json` names this port. 5173 is LTK Manager's, so both run at once.
  server: {
    port: 5174,
    strictPort: true,
    watch: {
      ignored: ["**/target/**"],
    },
  },

  resolve: {
    alias: {
      "@": path.resolve(__dirname, "./src"),
    },
  },

  envPrefix: ["VITE_", "TAURI_"],

  build: {
    // The webview is Chromium on Windows and WebKit on macOS and Linux.
    target: process.env.TAURI_ENV_PLATFORM === "windows" ? "chrome105" : "es2020",
    sourcemap: Boolean(process.env.TAURI_ENV_DEBUG),
    minify: !process.env.TAURI_ENV_DEBUG,
  },
});
