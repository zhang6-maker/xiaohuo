import { defineConfig } from "vite";
import { copyFileSync, mkdirSync, existsSync, readdirSync } from "node:fs";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = dirname(fileURLToPath(import.meta.url));

// 把画图兜底数据 web/qd 拷进 dist/qd。
// 关键：vite build 会清空 dist，而 qd 不是 import 资产，不手动回填则重嵌后 exe 丢 Quick Draw 长尾兜底。
// 这里在 closeBundle 里做，保证 `npm run build` 与 `tauri build` 每次都带 qd，无需手工 cp。
function copyQd() {
  const src = join(__dirname, "web", "qd");
  const dst = join(__dirname, "dist", "qd");
  if (!existsSync(src)) return;
  mkdirSync(dst, { recursive: true });
  const files = readdirSync(src);
  for (const f of files) copyFileSync(join(src, f), join(dst, f));
  console.log("[vite] qd 数据已回填 dist/qd (" + files.length + " 文件)");
}

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
  plugins: [
    {
      name: "copy-qd",
      closeBundle() {
        copyQd();
      },
    },
  ],
});
