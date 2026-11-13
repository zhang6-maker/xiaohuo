# 小火 · 桌面火柴人（xiaohuo）

> 一只住在桌面最上层的透明火柴人。**双击就能玩，不用注册、不用配 API Key** —— 它会满屏溜达、跟着鼠标转、被戳会吓一跳（不再缩角落消失）、会唱歌看书、能换装换发型；平时鼠标能直接穿透它去点桌面，不挡你干活。
>
> 想让它更聪明，再**可选**地填一个自己的智谱 API Key，解锁聊天、圈屏幕讲题、看屏理解和查词条。**不配 Key 也能玩大半功能。**

基于 **Tauri v2 + Rust**（AI 能力可选接入智谱 GLM-4V）。
[GitHub Releases](https://github.com/zhang6-maker/xiaohuo/releases) 提供**预编译便携包（约 19MB）**：解压 → 双击 → 完事；
另有 **MSI 安装包** `xiaohuo_0.0.9_x64_en-US.msi`（14.34 MiB），双击安装、自动建开始菜单/桌面快捷方式，也可在系统「应用」里卸载。
两者**不需要装 Rust / Node，也不需要账号**。
仓库、exe、便携包**均不含任何真实 API Key**（详见[关于隐私](#关于隐私)）。

---

## 它能干什么

| 能力 | 说明 |
| --- | --- |
| 🧍 桌面宠物 | 无边框透明窗口**铺满整屏**、常驻置顶；火柴人在全屏范围内自由漫游。想换回小窗：右键宠物 →「🖥️ 全屏 / 窗口」。 |
| 🖱️ 鼠标穿透 / 悬停可交互 | 默认 `ignore_cursor_events`，**不阻挡任何桌面点击**；只有当鼠标落进火柴人的包围盒时，窗口才自动恢复可交互。顶部按钮栏已隐藏，功能收在**长按菜单**里。 |
| 👀 盯着你的鼠标 | Rust 端持续采样光标坐标并推送给前端，火柴人能跟着鼠标转、有反应。 |
| ⬜ 全屏框选 | 一键把窗口撑满屏幕并临时放开 resizable，拖框圈出你想问的那块区域，选完自动退出。 |
| 📷 截屏 | 抓取框选区域的画面，转成图片交给模型。 |
| 💬 看图提问 / 搜题 | 画面 + 你的问题一起发给 GLM-4V，返回文字解答（默认 `glm-4v-flash`，**免费额度够日常用**）。 |
| ✏️ 画简笔画 | 输入一段描述，让它生成可绘制的路径指令。 |
| 🖼️ 点评画作 | 把你的涂鸦发给模型，让它点评 / 批改。 |
| ✍️ 屏幕写字 | 聊天框输入「在屏幕上写我爱你」，它会**按真实笔顺一笔一划把字写出**（内置 9574 字离线笔顺库，是真书写不是描边轮廓），**火柴人右手会精准握笔、笔尖跟着手逐笔移动，姿态像真在写字**；线条细、像手写；字色可自选（右键「✍️ 字色」选色并记住，或说「用红色写…」），不设置时跟随身体色；一批字固定一个色，写字过程不会被烟花/画画改掉；多条留言向下堆叠互不重叠；**默认停留约 1 分钟**自动消失，可加「停留X分钟/秒」指定时长（最长 2 小时）。中文走真笔顺，数字与常用标点也内置手写笔画、同样逐笔书写且手跟笔尖；英文字母/表情等仍回退为同色文本。**超过约 12 字（如整篇《出师表》）会温柔拒绝，不再静默截断写残缺片段**。 |

对应的五个后端命令（`src-tauri/src/lib.rs`）：`toggle_pick`（全屏框选）、`capture_screen`（截屏）、`ask_glm`（看图问答）、`ask_draw`（画简笔画）、`critique_draw`（点评画作）。

### 不配 Key 能玩什么（重点）

| 类型 | 功能 | 说明 |
| --- | --- | --- |
| **纯本地，连网都不用** | 全屏漫游、点击穿透、鼠标跟随、戳了会吓一跳（不再缩角落消失）、换色 / 换装 / 发型 / 围巾、节日皮肤、唱歌（8 首原创短歌轮换）、看书、📝 记忆 + 📖 记录本、屏幕写字 | Canvas 手绘 + `localStorage` + 本地文件，零依赖；屏幕写字也是纯前端零依赖 |
| **零 Key（外网可用时更好）** | 画常见简笔画（本地模板 / Iconify 图标库 / 内置 17 份手绘兜底稿）、查词条 | **44 词本地词典命中即答**（零 Key 零联网）；词典未收录的词才联网检索（维基 + DuckDuckGo）再让模型解释，这一步没配 Key 且外网不通时可能拿不到答案（不会崩，会给人话提示） |
| **需要自己的智谱 Key** | 自由聊天、圈屏幕讲题、看屏理解、生僻词让 AI 现画 | Key 在 https://bigmodel.cn/console/apikeys 免费领取，只存本机 `.env` |

> 画画对常见词（猫 / 狗 / 牛 / 羊 / 烟花 / 火箭…）走本地模板，零配置可用；只有没匹配到任何模板、图标、手绘稿的**生僻词**才去调 AI，此时若没配 Key 会提示「未配置 ZHIPU_API_KEY」（**不会崩**，只是画不出来）。
>
> 在聊天框问**「什么是猴子」「鸡是什么」「解释一下房子」**，或直接打「鸡 / 猴子 / 房子」这类名词，都会自动给解释：内置 44 词本地词典**优先命中直接答**，不经外网、不需要 API Key。词典**没收录**的词（如「量子计算」）才联网查 + 让模型用自己的知识答 —— 这一步需要 Key，且依赖外网检索；没配 Key 或外网不通时可能拿不到答案（不会崩，会给人话提示）。
>
> 🎵 唱歌内置 **8 首原创短歌**，每次开唱随机换一首且不会连唱同一首；聊天框说「**换一首 / 再来一首 / 点歌**」可立刻换，节日自动唱对应节日小调。歌词是头顶飘的文字气泡，**目前没有声音**。

### 关于隐私

- AI 请求由 **Rust 端直连智谱官方接口** `open.bigmodel.cn`，**不经过本项目作者的任何服务器**。
- API Key 只存在你本地的 `.env` 里，**不会被打包进 exe**，也不写入任何远程存储。
- 被上传的**只有你主动框选的那块画面**。不框选就没有任何上传行为。
- 「📝 记忆」功能把问题 / 回答 / 截图**只存在 exe 同级的 `memories/` 目录**，纯本地、不上传。换机器或删除程序前请自行备份该目录；**发布包里绝不会带上它**。

---

## 环境要求

- **Windows 10/11 64 位**（程序使用 Win32 API，**仅 Windows 可编译运行**，暂不支持 macOS / Linux）
- **WebView2 运行时**：Win10/11 通常已自带；若窗口白屏/打不开，装 [WebView2 Runtime](https://developer.microsoft.com/microsoft-edge/webview2/)
- **智谱 API Key（可选）**：只有想用 AI 聊天 / 看屏 / 搜题 / 生僻词画画时才需要，在 https://bigmodel.cn/console/apikeys 免费领取。**不填也能玩上面那一大堆本地功能**

---

## 快速开始（推荐，无需编译）

### 只想先看它跑起来（零配置，10 秒）

1. 到 [GitHub Releases](https://github.com/zhang6-maker/xiaohuo/releases) 下载 **`xiaohuo-portable.zip`**（或单独下 `xiaohuo.exe`），解压到任意文件夹。
2. **双击 `xiaohuo.exe`**。桌面上出现透明火柴人即成功 —— **不用注册、不用填任何东西**。
   - 首次运行可能弹出 Windows SmartScreen「已保护你的电脑」→ 点「**详细信息 → 仍要运行**」（个人项目未买代码签名证书，属正常，不是病毒告警）。
   - 若窗口空白 / 打不开 → 安装 [WebView2 Runtime](https://developer.microsoft.com/microsoft-edge/webview2/)（Win10/11 通常已自带）。

鼠标移到它身上会变得可交互；**长按**唤出功能菜单。想退出：右键 →「⏻ 退出」。

### 想解锁 AI 功能（可选，约 1 分钟）

1. 把包里的 `.env.example` **改名为 `.env`**，用记事本打开，填进你自己的 key：

   ```
   ZHIPU_API_KEY=你的key
   ```

   - `.env` 必须和 `xiaohuo.exe` **在同一个目录**。
   - key 在 https://bigmodel.cn/console/apikeys **免费领取**。本项目**不附带任何 key**，也不会把 key 内置进 exe。
   - 其余 `ZHIPU_CHAT_MODEL` / `ZHIPU_VISION_MODEL` / `ZHIPU_DRAW_MODEL` 可选，留空即用免费默认模型；若把它们设成付费模型（如 `glm-4.5-air` / `glm-4.6v`），小火会优先扣你的资源包，资源包耗尽时自动切回免费模型，并在桌面右下角实时显示当前用的哪个模型。
2. **重新双击 `xiaohuo.exe`** —— 配置只在启动时读一次，改完必须重启。

### `.env` 配置说明

| 变量 | 必填 | 默认 | 说明 |
| --- | --- | --- | --- |
| `ZHIPU_API_KEY` | **仅 AI 功能需要** | — | 智谱开放平台 API Key；不填也能玩本地功能 |
| `ZHIPU_CHAT_MODEL` | 否 | `glm-4-flash` | 聊天回复模型 |
| `ZHIPU_VISION_MODEL` | 否 | `glm-4v-flash` | 看图理解 / 搜题用的多模态模型 |
| `ZHIPU_DRAW_MODEL` | 否 | `glm-4-flash` | 画图模型 |

> 想用资源包（付费模型）：把上面三个模型项填成你的付费模型（如 `glm-4.5-air` / `glm-4.6v`），小火优先扣资源包；资源包耗尽时本会话自动降级回免费 `glm-4-flash` / `glm-4v-flash` 并重试，无需手动改配置。每次调用都会在桌面右下角显示实际使用的模型与来源。

> `.env` 仅供本地使用，已在 `.gitignore` 中排除。**请务必不要把你的真实 key 提交或发给别人。**

---

## 想自己编译（可选）

前提：安装 [Rust](https://rustup.rs)（stable MSVC）与 [Node.js](https://nodejs.org)。

仓库里**没有 `build.bat`**（早期版本有过，为避免二进制/脚本污染 git 历史已移除）。请按两步手动构建：

```bash
npm install
npm run build                # 生成 dist/（vite 会自动把 qd 兜底数据回填进 dist/qd）
cd src-tauri
cargo build --release        # 产物：src-tauri/target/release/xiaohuo.exe
```

> Windows 上 `cargo` 若不在 PATH，用 `%USERPROFILE%\.cargo\bin\cargo.exe`。
> `cargo build --release` 只产出**裸 exe**（不生成 MSI / NSIS 安装器）；前端资产靠 `custom-protocol` 已压进 exe，**运行时不需要 dist/**。
> **前端改过必须先 `npm run build` 再 `cargo build --release`**，否则嵌进的是旧前端。
> 验证是否嵌对：搜 exe 字节里 `/assets/index-<hash>.js` 的 hash 是否与本次构建一致。

---

## 常见问题

**双击 exe 没反应 / 窗口看不见？**
窗口是透明的，内容由 WebView2 渲染。若完全空白，多半是缺 WebView2 运行时，装上再试。

**弹出「Windows 已保护你的电脑」？**
未签名的个人项目会触发 SmartScreen。点「**详细信息 → 仍要运行**」即可，这是签名提示而非病毒告警。

**AI 一直返回 429？**
- 免费模型有调用频率/额度限制，稍等一会儿再试，或去控制台升级套餐。
- 若你配了付费模型却仍报 429，多半是**资源包耗尽**——小火会自动切回免费模型并在右下角提示，不用手动改配置。
- 两种情况都**不代表 key 泄露**。

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
xiaohuo/
├── .env.example       # 复制为 .env 并填 key（与 exe 同目录）
├── README.md          # 本文件
├── LICENSE            # MIT
├── package.json / vite.config.js / index.html   # 前端（Vite）配置与入口
├── web/               # 前端源码（Canvas 火柴人 + UI）
├── src/               # 前端抽出的模块（fetchWithTimeout / templates）
└── src-tauri/         # Rust 后端（Cargo.toml / tauri.conf.json / src / icons / capabilities）
```

> **预编译 exe 不在仓库里**：为避免二进制污染 git 历史，exe 作为 [GitHub Releases](https://github.com/zhang6-maker/xiaohuo/releases) 附件发布，去那里下载即可。从源码构建时，`cargo build --release` 会读取仓库根目录的 `../dist`（前端构建产物，需先 `npm run build` 生成）——改完前端请先 `npm run build` 再 `cargo build --release`，否则会嵌进旧前端。

---

## 开源许可

**MIT**（见 `LICENSE`）——可自由学习、修改、再分发，包括商用，只需保留版权声明。

## 免责

这是一个个人玩具项目，按"原样"提供，不作任何担保。使用第三方大模型服务产生的费用由你自己承担，请遵守智谱开放平台的服务条款。
