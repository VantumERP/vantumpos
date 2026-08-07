import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";

// @ts-expect-error process is a nodejs global
const host = process.env.TAURI_DEV_HOST;

// https://vite.dev/config/
export default defineConfig(async () => ({
  plugins: [react(), tailwindcss()],
  resolve: {
    alias: {
      "@": new URL("./src", import.meta.url).pathname,
    },
  },
  test: {
    environment: "jsdom",
    setupFiles: ["./src/test/setup.ts"],
    css: true,
    // Agent worktrees live INSIDE the repo (`.claude/worktrees/<name>`), so the
    // default glob picks up every stale copy of every test file and reports
    // their failures as ours. The gate must only ever run this checkout.
    exclude: [
      "**/node_modules/**",
      "**/dist/**",
      "**/.claude/worktrees/**",
      "**/.codex/worktrees/**",
    ],
    // Vitest defaults to 5000 ms, and the module tests that walk a whole state
    // machine — PopisModule alone drives six statuses and clicks every action on
    // each — sit close enough to it that a busy machine turns a passing suite
    // red. That happened three times on 07.08.2026 while a cargo build ran
    // beside the suite, reporting 4, then 13, then 5 failures; each time the
    // same commit gave 539/539 when the suite had the CPU to itself. A false red
    // is worse than a slow gate: it costs a re-run, and it teaches whoever reads
    // it to distrust the gate.
    //
    // 30 s is a ceiling for a hung test, not a budget to spend — the whole suite
    // runs in about 30 s wall, so a test that genuinely needs seconds is a test
    // waiting on something it should be asserting on instead.
    testTimeout: 30_000,
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
