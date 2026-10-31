# 小火 · 桌面火柴人（xiaohuo）

> 一只住在桌面最上层的透明火柴人。它平时**不挡事**——鼠标可以直接穿过它去点桌面；需要它干活时，它会"看"你的屏幕：圈一块区域，让它讲题、解释、画张简笔画，或者点评你的涂鸦。

基于 **Tauri v2 + Rust**，AI 能力调用 **智谱 GLM-4V** 多模态模型。
仓库自带 **预编译 exe（约 4MB）**，填入你自己的 API Key 就能直接启动——**不需要装 Rust / Node**。

---

## 它能干什么

| 能力 | 说明 |
| --- | --- |
| 🧍 桌面宠物 | 480×540 无边框透明窗口，常驻置顶。 |
| 🖱️ 鼠标穿透 / 悬停可交互 | 默认 `ignore_cursor_events`，**不阻挡任何桌面点击**；只有当鼠标落进火柴人的包围盒时，窗口才自动恢复可交互。顶部按钮栏已隐藏，功能收在**长按菜单**里。 |
| 👀 盯着你的鼠标 | Rust 端持续采样光标坐标并推送给前端，火柴人能跟着鼠标转、有反应。 |
| ⬜ 全屏框选 | 一键把窗口撑满屏幕并临时放开 resizable，拖框圈出你想问的那块区域，选完自动退出。 |
| 📷 截屏 | 抓取框选区域的画面，转成图片交给模型。 |
| 💬 看图提问 / 搜题 | 画面 + 你的问题一起发给 GLM-4V，返回文字解答（默认 `glm-4v-flash`，**免费额度够日常用**）。 |
| ✏️ 画简笔画 | 输入一段描述，让它生成可绘制的路径指令。 |
| 🖼️ 点评画作 | 把你的涂鸦发给模型，让它点评 / 批改。 |

对应的五个后端命令（`src-tauri/src/lib.rs`）：`toggle_pick`（全屏框选）、`capture_screen`（截屏）、`ask_glm`（看图问答）、`ask_draw`（画简笔画）、`critique_draw`（点评画作）。

### 关于隐私

- AI 请求由 **Rust 端直连智谱官方接口** `open.bigmodel.cn`，**不经过本项目作者的任何服务器**。
- API Key 只存在你本地的 `.env` 里，**不会被打包进 exe**，也不写入任何远程存储。
- 被上传的**只有你主动框选的那块画面**。不框选就没有任何上传行为。

---

## 环境要求

- **Windows 10/11 64 位**（程序使用 Win32 API，**仅 Windows 可编译运行**，暂不支持 macOS / Linux）
- **WebView2 运行时**：Win10/11 通常已自带；若窗口白屏/打不开，装 [WebView2 Runtime](https://developer.microsoft.com/microsoft-edge/webview2/)
- 一个**智谱 API Key**：在 https://bigmodel.cn/console/apikeys 免费领取

---

## 快速开始（推荐，无需编译）

1. 把仓库根目录的 `.env.example` **复制一份，改名为 `.env`**，放在 `xiaohuo.exe` **同目录**下。
2. 打开 `.env`，填进你的 key：

   ```
   ZHIPU_API_KEY=你的key
   ```

3. **双击 `xiaohuo.exe`**。桌面上出现透明火柴人即成功。

鼠标移到它身上会变得可交互；**长按**唤出功能菜单，里面才是框选 / 提问 / 画图这些入口。

### `.env` 配置说明

| 变量 | 必填 | 默认 | 说明 |
| --- | --- | --- | --- |
| `ZHIPU_API_KEY` | 是 | — | 智谱开放平台 API Key |
| `ZHIPU_VISION_MODEL` | 否 | `glm-4v-flash` | 看图理解 / 搜题用的多模态模型 |
| `ZHIPU_DRAW_MODEL` | 否 | `glm-4-flash` | 画图模型 |

> `.env` 仅供本地使用，已在 `.gitignore` 中排除。**请务必不要把你的真实 key 提交或发给别人。**

---

## 想自己编译（可选）

前提：安装 [Rust](https://rustup.rs)（stable MSVC）与 [Node.js](https://nodejs.org)。

**双击根目录的 `build.bat`**，它会自动完成：

1. 定位 `cargo`（不在 PATH 时自动尝试 `%USERPROFILE%\.cargo\bin`）；
2. 若尚无 `dist/`，先用 npm 构建前端；
3. `cargo build --release` 编译出 exe；
4. 把 exe 拷回仓库根目录；
5. **自动删除 `src-tauri/target/`**——那是编译产生的几百 MB 中间文件，**只保留 exe**，目录保持干净。

> `cargo build --release` 只产出 exe，不会打包 MSI / NSIS 安装器，所以删掉 `target/` 完全不影响运行。

如果你想亲手敲命令：

```bash
cd src-tauri
cargo build --release     # 产物：src-tauri/target/release/xiaohuo.exe
```

---

## 常见问题

**双击 exe 没反应 / 窗口看不见？**
窗口是透明的，内容由 WebView2 渲染。若完全空白，多半是缺 WebView2 运行时，装上再试。

**提示「未配置 ZHIPU_API_KEY」？**
`.env` 必须和 `xiaohuo.exe` **在同一个目录**里，且变量名是 `ZHIPU_API_KEY`（别带空格或引号）。改完要**重启程序**，`dotenv` 只在启动时读一次。

**能启动但 AI 不回答、一直失败？**
常见原因是**杀毒软件拦截了程序的出站联网**（未签名的自建 exe 尤其容易中招）。把 `xiaohuo.exe` 或整个目录加进杀软信任区 / 白名单即可。

**它挡住我点东西了？**
不会——默认鼠标穿透，只有鼠标落进火柴人包围盒时才关闭穿透。把鼠标移开它的身体就恢复了。

**想改宠物大小 / 位置？**
看 `src-tauri/tauri.conf.json` 里 `app.windows` 的 `width` / `height`，改完重新编译生效。

---

## 技术栈

- **后端**：Rust + [Tauri v2](https://tauri.app)（含 `windows` crate 做 Win32 窗口控制、`reqwest` 调 API、`dotenvy` 读配置）
- **前端**：原生 HTML/CSS/Canvas + [Vite](https://vitejs.dev)（火柴人用 Canvas 画）
- **模型**：智谱 `glm-4v-flash`（视觉）/ `glm-4-flash`（文本）

---

## 目录结构

```
xiaohuo-opensource/
├── xiaohuo.exe        # 预编译 exe（约 4MB），填 key 直接用
├── .env.example       # 复制为 .env 并填 key
├── README.md          # 本文件
├── LICENSE            # MIT
├── build.bat          # 一键编译 + 自动清理 target
├── package.json / vite.config.js / index.html   # 前端（Vite）配置与入口
├── web/               # 前端源码（Canvas 火柴人 + UI）
├── dist/              # 已构建的前端（cargo 直接读取，免 npm 即可编译）
└── src-tauri/         # Rust 后端（Cargo.toml / tauri.conf.json / src / icons / capabilities）
```

> `dist/` 是故意保留在仓库里的：`cargo build --release` 直接读取 `../dist`，别人 clone 下来不必先跑 npm。改完前端请重新 `npm run build`。

---

## 开源许可

**MIT**（见 `LICENSE`）——可自由学习、修改、再分发，包括商用，只需保留版权声明。

## 免责

这是一个个人玩具项目，按"原样"提供，不作任何担保。使用第三方大模型服务产生的费用由你自己承担，请遵守智谱开放平台的服务条款。
