import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// Tauri 期望固定端口，dev server 端口不能漂移，否则 Rust 侧加载不到
export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: {
    port: 5183,
    strictPort: true,
    watch: {
      // src-tauri 由 Rust 侧 watch，前端不用重复监听
      ignored: ["**/src-tauri/**"],
    },
  },
  build: {
    // WKWebView 至少支持 ES2022；Vite 8 默认用 oxc 压缩，不再依赖 esbuild
    target: "es2022",
    sourcemap: false,
    chunkSizeWarningLimit: 900,
  },
});
