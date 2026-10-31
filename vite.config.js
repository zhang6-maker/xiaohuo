import { defineConfig } from "vite";

// Tauri 配套 Vite 配置
export default defineConfig({
  // 不要让 Vite 清屏刷掉 Rust 编译/运行输出
  clearScreen: false,
  server: {
    port: 5173,
    strictPort: true,
    host: false,
  },
  build: {
    target: "es2021",
    sourcemap: false,
  },
  // 关键：排除 @tauri-apps/api 预打包（pre-bundle）。
  // 否则 Vite 把它打成单 chunk 后，动态子路径导入 import('@tauri-apps/api/event')
  // 会解析失败；排除后由 Vite 按真实文件解析，配合 <script type="module"> 即可正常加载。
  optimizeDeps: {
    exclude: ["@tauri-apps/api"],
  },
});
