use std::ffi::c_void;
use std::sync::{Arc, Mutex};
use base64::Engine;
use tauri::Emitter;
use tauri::Listener;
use tauri::Manager;
use windows::Win32::Foundation::{POINT, RECT};
use windows::Win32::Graphics::Gdi::{
    BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject, GetDIBits, GetDC,
    ReleaseDC, SelectObject, DIB_RGB_COLORS, SRCCOPY, BITMAPINFO, BITMAPINFOHEADER, HBITMAP, HDC,
    HGDIOBJ,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetCursorPos, GetSystemMetrics, GetWindowRect, SM_CXSCREEN, SM_CYSCREEN,
};

/// 框选态标志：进入"看屏/搜题"全屏框选时置 true，期间窗口必须完全可交互（否则框选点不到）
struct AppState {
    picking: Arc<Mutex<bool>>,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(AppState {
            picking: Arc::new(Mutex::new(false)),
        })
        .setup(|app| {
            // 加载 .env（dev 时 cwd=D:\xiaohuo，故优先 src-tauri/.env）
            let _ = dotenvy::from_path("src-tauri/.env").or_else(|_| dotenvy::from_path(".env"));

            let app_handle = app.handle().clone();

            if let Some(w) = app_handle.get_webview_window("main") {
                // 真穿透：默认 ignore=true，透明窗不挡桌面点击
                let _ = w.set_ignore_cursor_events(true);

                // 放到屏幕左下角（避让右下角的寻慧桌面宠物），并留任务栏余量。
                // 用 LogicalPosition + mon.scale_factor() 取真实 dpr，Tauri 自动把逻辑坐标转物理像素，
                // 避免手算物理坐标（上一版把 物理宽/窗宽 当 dpr，高 DPI 下算出负坐标把窗口推出屏外 → 看不见）。
                if let Ok(Some(mon)) = w.current_monitor() {
                    let sf = mon.scale_factor();                  // 真实 dpr（如 1.5）
                    let logical_h = mon.size().height as f64 / sf; // 物理高 / dpr = 逻辑高
                    let x = 24.0;                                  // 距左边 24 逻辑像素
                    let y = (logical_h - 540.0 - 60.0).max(0.0);  // 底部留 60 逻辑像素（任务栏余量），夹到 0 防极端负
                    let _ = w.set_position(tauri::LogicalPosition::new(x, y));
                    // 兜底：setup 阶段 set_position 偶发时机偏早失效，延迟 400ms 再设一次（位置相同，无跳变）
                    let wb = w.clone();
                    std::thread::spawn(move || {
                        std::thread::sleep(std::time::Duration::from_millis(400));
                        if let Ok(Some(mon2)) = wb.current_monitor() {
                            let sf2 = mon2.scale_factor();
                            let lh2 = mon2.size().height as f64 / sf2;
                            let _ = wb.set_position(tauri::LogicalPosition::new(
                                24.0,
                                (lh2 - 540.0 - 60.0).max(0.0),
                            ));
                        }
                    });
                }
            }

            // 框选态标志，供轮询线程读取
            let picking = app.state::<AppState>().picking.clone();

            // 前端每帧把宠物包围盒(窗口内逻辑坐标 [x,y,w,h])经 event 发过来
            let rect = Arc::new(Mutex::new([250i32, 170i32, 300i32, 230i32]));
            let rect_listen = rect.clone();
            let _ = app_handle.listen("pet-rect", move |e| {
                if let Ok(v) = serde_json::from_str::<[i32; 4]>(e.payload()) {
                    if let Ok(mut g) = rect_listen.lock() {
                        *g = v;
                    }
                }
            });

            let handle_poll = app_handle.clone();
            let rect_poll = rect.clone();
            std::thread::spawn(move || {
                let mut last_ignore = true;
                let mut frame = 0u32;
                loop {
                    std::thread::sleep(std::time::Duration::from_millis(16));
                    frame = frame.wrapping_add(1);

                    // 鼠标屏幕坐标（设备像素）
                    let mut pt = POINT::default();
                    unsafe {
                        let _ = GetCursorPos(&mut pt);
                    }

                    // 每帧用 GetWindowRect 重新取窗口矩形（设备像素，与 GetCursorPos 同源）。
                    // 关键修复①：之前只在 setup 取一次 outer_position，窗口异步落位后偏移就错了，
                    //   导致 lx/ly 恒偏 → mouse.inside 恒 false → 头不跟、戳不到。
                    // 关键修复②：lx/ly 是设备像素，而前端发的 pet-rect 与栏阈值 95 是逻辑像素；
                    //   高 DPI(dpr≠1) 下两套单位混用会让悬停判定错位 → 统一乘以 scale_factor 转设备像素。
                    let mut lx = pt.x;
                    let mut ly = pt.y;
                    let mut sf = 1.0_f64;
                    if let Some(w) = handle_poll.get_webview_window("main") {
                        if let Ok(h) = w.hwnd() {
                            let mut r = RECT::default();
                            unsafe {
                                let _ = GetWindowRect(h, &mut r);
                            }
                            lx -= r.left;
                            ly -= r.top;
                        }
                        if let Ok(s) = w.scale_factor() {
                            sf = s;
                        }
                    }

                    // 鼠标坐标推给前端（设备像素相对窗口），前端除以 dpr 得逻辑像素
                    let _ = handle_poll.emit("mouse-move", [lx, ly]);

                    // 悬停判定（全部在设备像素下做，与 lx/ly 同源）：
                    // 在宠物包围盒 或 顶部按钮栏(窗口内 y<95 逻辑)内 → 关穿透(可交互)；否则穿透(不挡桌面)
                    let r = *rect_poll.lock().unwrap_or_else(|_| rect_poll.lock().unwrap());
                    let inside = lx >= 0
                        && lx <= (480.0 * sf) as i32
                        && ly >= 0
                        && ly <= (540.0 * sf) as i32;
                    let over_pet = inside
                        && lx >= (r[0] as f64 * sf) as i32
                        && lx <= ((r[0] + r[2]) as f64 * sf) as i32
                        && ly >= (r[1] as f64 * sf) as i32
                        && ly <= ((r[1] + r[3]) as f64 * sf) as i32;
                    // 顶部按钮栏已隐藏（功能收进长按菜单），不再因按钮栏区域关闭穿透，
                    // 否则窗口顶部 95px 会一直挡住桌面点击。
                    let over_bar = false;

                    // 框选态：强制完全可交互（否则全屏下框选点不到）
                    let picking_now = *picking.lock().unwrap_or_else(|_| picking.lock().unwrap());
                    let want_ignore = if picking_now {
                        false
                    } else {
                        !(over_pet || over_bar)
                    };
                    if want_ignore != last_ignore {
                        last_ignore = want_ignore;
                        if let Some(w) = handle_poll.get_webview_window("main") {
                            let _ = w.set_ignore_cursor_events(want_ignore);
                        }
                    }

                    // 诊断事件：每 ~10 帧发一次（约 6/s），把内部状态推给前端 HUD
                    if frame % 10 == 0 {
                        let _ = handle_poll.emit(
                            "dbg",
                            [
                                lx,
                                ly,
                                if inside { 1 } else { 0 },
                                if over_pet { 1 } else { 0 },
                                if want_ignore { 1 } else { 0 },
                                (sf * 100.0) as i32,
                            ],
                        );
                    }
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![capture_screen, ask_glm, ask_draw, critique_draw, toggle_pick, ask_reply])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// 截全屏 → 编码 PNG → 返回 data URL。框选前抓一次，保证框选看到的是"点击前"的真实屏幕。
#[tauri::command]
fn capture_screen() -> Result<String, String> {
    unsafe {
        let screen: HDC = GetDC(None); // None = 整个桌面
        let sw = GetSystemMetrics(SM_CXSCREEN);
        let sh = GetSystemMetrics(SM_CYSCREEN);
        eprintln!("[capture] 开始截屏: {}x{}", sw, sh);
        let mem: HDC = CreateCompatibleDC(Some(screen));
        let bmp: HBITMAP = CreateCompatibleBitmap(screen, sw, sh);
        let old: HGDIOBJ = SelectObject(mem, HGDIOBJ(bmp.0));
        let _ = BitBlt(mem, 0, 0, sw, sh, Some(screen), 0,0, SRCCOPY);

        let mut bmi: BITMAPINFO = std::mem::zeroed();
        bmi.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
        bmi.bmiHeader.biWidth = sw as i32;
        bmi.bmiHeader.biHeight = -(sh as i32); // 负 = 从上到下
        bmi.bmiHeader.biPlanes = 1u16;
        bmi.bmiHeader.biBitCount = 32u16;
        bmi.bmiHeader.biCompression = 0u32; // BI_RGB = 0

        let mut buf = vec![0u8; (sw as i64 * sh as i64 * 4) as usize];
        let lines = GetDIBits(
            mem,
            bmp,
            0,
            sh as u32,
            Some(buf.as_mut_ptr() as *mut c_void),
            &mut bmi,
            DIB_RGB_COLORS,
        );
        let _ = SelectObject(mem, old);
        let _ = DeleteObject(HGDIOBJ(bmp.0));
        let _ = DeleteDC(mem);
        let _ = ReleaseDC(None, screen);
        if lines == 0 {
            eprintln!("[capture] 失败: GetDIBits 返回 0 行（权限/DWM 合成问题）");
            return Err("截屏失败（GetDIBits 返回 0）".into());
        }

        // BGRA → RGBA（忽略 alpha）
        let mut rgba = Vec::with_capacity(buf.len());
        for c in buf.chunks_exact(4) {
            rgba.extend_from_slice(&[c[2], c[1], c[0], 255]);
        }

        let mut png_data = Vec::new();
        {
            let mut enc = png::Encoder::new(&mut png_data, sw as u32, sh as u32);
            enc.set_color(png::ColorType::Rgba);
            enc.set_depth(png::BitDepth::Eight);
            let mut w = enc.write_header().map_err(|e| e.to_string())?;
            w.write_image_data(&rgba).map_err(|e| e.to_string())?;
        }
        let b64 = base64::engine::general_purpose::STANDARD.encode(&png_data);
        eprintln!("[capture] 成功: PNG {} bytes, base64 {} chars", png_data.len(), b64.len());
        Ok(format!("data:image/png;base64,{}", b64))
    }
}

/// 调智谱 GLM-4V（多模态）：把裁剪后的图片（data URL）+ 提示词发给模型，返回文本。
#[tauri::command]
async fn ask_glm(image_b64: String, prompt: String) -> Result<String, String> {
    let key = match std::env::var("ZHIPU_API_KEY") {
        Ok(k) if !k.is_empty() => k,
        _ => {
            return Err(
                "未配置 ZHIPU_API_KEY：请把 key 写入 .env（ZHIPU_API_KEY=xxx，.env 放在 exe 同目录或 src-tauri/ 下）并重启程序".into(),
            )
        }
    };
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(90))
        .build()
        .map_err(|e| e.to_string())?;
    let payload = serde_json::json!({
        "model": "glm-4v-flash",
        "messages": [{
            "role": "user",
            "content": [
                {"type": "text", "text": prompt},
                {"type": "image_url", "image_url": {"url": image_b64}}
            ]
        }],
        "max_tokens": 1024,
        "temperature": 0.6
    });
    let resp = client
        .post("https://open.bigmodel.cn/api/paas/v4/chat/completions")
        .bearer_auth(key)
        .json(&payload)
        .send()
        .await
        .map_err(|e| format!("请求智谱失败（本机网络/360 防火墙拦截？）：{}", e))?;
    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("智谱返回 {}：{}", status.as_u16(), body));
    }
    let v: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
    let content = v
        .get("choices")
        .and_then(|c| c.get(0))
        .and_then(|c| c.get("message"))
        .and_then(|m| m.get("content"))
        .and_then(|c| c.as_str())
        .unwrap_or("（模型无返回）")
        .to_string();
    Ok(content)
}

/// 调智谱文本模型：把"画 X"的提示词发给模型，返回它生成的绘画原语 JSON 文本。
/// 前端再把这个 JSON 翻译成小火能画的笔画（线/圆/波浪/星/涂鸦）。
/// ref_desc：前端画之前联网（维基百科）查到的真实形状描述，作为空间参考注入，减少模型瞎编。
#[tauri::command]
async fn ask_draw(prompt: String, ref_desc: Option<String>) -> Result<String, String> {
    let key = match std::env::var("ZHIPU_API_KEY") {
        Ok(k) if !k.is_empty() => k,
        _ => return Err(
            "未配置 ZHIPU_API_KEY：请把 key 写入 .env（ZHIPU_API_KEY=xxx，.env 放在 exe 同目录或 src-tauri/ 下）并重启程序".into(),
        ),
    };
    let model = std::env::var("ZHIPU_DRAW_MODEL").unwrap_or_else(|_| "glm-4-flash".to_string());
    let instruction = r##"你是一个用极简原语作画的火柴人画师。把用户要画的东西用我们支持的原语组合出来，并【只输出一段 JSON，不要任何解释、不要 markdown 代码块、不要 ``` 】。

画布坐标系：300x300，原点左上角，x 向右、y 向下。主体落在底部中央，底部中点约 (150,270)，图形向上（y 减小）生长，坐标务必在 0~300 内。

布局要求：先在脑内框定一个约 180(宽)×210(高) 的 bounding box，居中在 (150,170) 附近，所有图形都落在框内、整体居中且比例协调；不要贴边、不要缩在角落、也不要画到框外。即便前端会再做一次归一化，良好的布局仍让画作更准。

支持的原语（prim 取值）与参数 a：
- line:    a=[x1,y1,x2,y2]                一条直线
- circle:  a=[cx,cy,r]                    一个圆（r 半径 8~40）
- wave:    a=[x1,y1,x2,y2,freq]            带正弦波动的线（freq 2~5）
- star:    a=[cx,cy,R,spikes]              多角星（spikes 5 或 6）
- scribble:a=[cx,cy,seed]                  一团涂鸦（seed 1~99999，用来表现蓬松/毛发/羊毛）
- dot:     a=[cx,cy,r]                     实心小圆点（画眼睛/鼻孔/斑点/烟花火星）
- tri:     a=[x1,y1,x2,y2,x3,y3]           实心三角形（画牛角/尖耳/羊角）
- poly:    a=[x1,y1,x2,y2,...]             开放折线轮廓（点数为偶数且≥4，勾身体/腿的轮廓）

每条笔画：{"prim":"...","a":[...],"color":"#RRGGBB"}。颜色用符合物体真实颜色的十六进制（草绿 #3aa655、花红 #ff5b8a、烟花多彩 #ff5b8a/#ffd23d/#6ab0ff、太阳金黄 #f5a623、牛棕 #8a5a2b、云灰 #9aa0a6、羊灰白 #d8d8dc）。

区分牛和羊的关键（务必画出，否则二者看起来一模一样）：
- 牛：棕色 #8a5a2b。身体一个较大圆/轮廓 + 头一个圆 + 4 条腿(line) + 【用 tri 画两只朝前的尖角】+ 【用 dot 画两只眼睛和 2~3 块斑点】。
- 羊：灰白 #d8d8dc。【用 scribble 在身体处堆 2~3 团涂鸦表现蓬松羊毛】+ 头小圆 + 4 条腿 + 【不要画角】+ 两个 dot 当眼睛。羊整体"毛茸茸一团"，牛"有角有斑"。

烟花（注意：烟花不是花！）：从中心点约 (150,160) 向四周放射 6~10 条 line（角度尽量均匀），每条末端用 dot 点缀火星，可用多色；【千万不要画成花瓣围绕中心的形状】。

输出格式（严格遵守）：
{"caption":"名称","strokes":[...]}

示例——牛：{"caption":"牛","strokes":[{"prim":"circle","a":[150,205,42],"color":"#8a5a2b"},{"prim":"circle","a":[205,170,22],"color":"#8a5a2b"},{"prim":"line","a":[130,250,130,278],"color":"#8a5a2b"},{"prim":"line","a":[170,250,170,278],"color":"#8a5a2b"},{"prim":"tri","a":[198,150,206,140,210,158],"color":"#8a5a2b"},{"prim":"dot","a":[198,168,6],"color":"#2b2b2f"},{"prim":"dot","a":[212,168,6],"color":"#2b2b2f"}]}
示例——羊：{"caption":"羊","strokes":[{"prim":"scribble","a":[150,205,12345],"color":"#d8d8dc"},{"prim":"scribble","a":[135,200,54321],"color":"#d8d8dc"},{"prim":"circle","a":[205,175,20],"color":"#d8d8dc"},{"prim":"line","a":[134,245,134,275],"color":"#8a5a2b"},{"prim":"line","a":[168,245,168,275],"color":"#8a5a2b"},{"prim":"dot","a":[200,172,6],"color":"#2b2b2f"},{"prim":"dot","a":[212,172,6],"color":"#2b2b2f"}]}
示例——烟花：{"caption":"烟花","strokes":[{"prim":"line","a":[150,160,110,100],"color":"#ff5b8a"},{"prim":"line","a":[150,160,190,100],"color":"#ffd23d"},{"prim":"line","a":[150,160,150,80],"color":"#6ab0ff"},{"prim":"line","a":[150,160,110,210],"color":"#ffd23d"},{"prim":"line","a":[150,160,190,210],"color":"#ff5b8a"},{"prim":"dot","a":[110,100,5],"color":"#ff5b8a"},{"prim":"dot","a":[190,100,5],"color":"#ffd23d"},{"prim":"dot","a":[150,80,5],"color":"#6ab0ff"}]}

要求：1) 只输出 JSON，不要 ``` 、不要任何额外文字；2) strokes 4~20 条；3) 只使用上面列出的 8 种 prim；4) 所有坐标在 0~300 内；5) 每条 stroke 的 a 长度必须与该 prim 要求完全一致（poly 为偶数且≥4）；6) 先规划 bbox 再落笔，整体居中于 (150,170) 附近，不要偏到一侧或缩成一点。"##;
    let mut user_msg = format!("{}\n\n现在请画：{}", instruction, prompt);
    if let Some(refd) = ref_desc {
        if !refd.trim().is_empty() {
            user_msg.push_str(&format!(
                "\n\n【网络检索到的真实描述（用于对照真实形状与主要部位/布局，安排笔画时务必参考，别凭空想象）】\n{}",
                refd
            ));
        }
    }
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .map_err(|e| e.to_string())?;
    let payload = serde_json::json!({
        "model": model,
        "messages": [{"role": "user", "content": user_msg}],
        "max_tokens": 900,
        "temperature": 0.7
    });
    let resp = client
        .post("https://open.bigmodel.cn/api/paas/v4/chat/completions")
        .bearer_auth(key)
        .json(&payload)
        .send()
        .await
        .map_err(|e| format!("请求智谱失败（本机网络/360 防火墙拦截？）：{}", e))?;
    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("智谱返回 {}：{}", status.as_u16(), body));
    }
    let v: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
    let content = v
        .get("choices").and_then(|c| c.get(0)).and_then(|c| c.get("message"))
        .and_then(|m| m.get("content")).and_then(|c| c.as_str())
        .unwrap_or("（模型无返回）").to_string();
    Ok(content)
}

/// 视觉自纠：把小火刚画的简笔画 PNG + 目标物体名发给多模态模型(GLM-4V)，
/// 让它判断"像不像 X、哪里歪/缺关键部位"，返回修正后的原语 JSON（前端再翻译+归一化）。
#[tauri::command]
async fn critique_draw(prompt: String, image_b64: String, prev_json: Option<String>) -> Result<String, String> {
    let key = match std::env::var("ZHIPU_API_KEY") {
        Ok(k) if !k.is_empty() => k,
        _ => return Err("未配置 ZHIPU_API_KEY：请把 key 写入 .env（ZHIPU_API_KEY=xxx，.env 放在 exe 同目录或 src-tauri/ 下）并重启程序".into()),
    };
    // 注意: glm-4v-plus 在本机实测报 429(余额不足/无可用资源包), 默认改用 glm-4v-flash(免费且可用)
    let model = std::env::var("ZHIPU_VISION_MODEL").unwrap_or_else(|_| "glm-4v-flash".to_string());
    let instruction = r##"你是一个火柴人画师助手。下面是一张已画好的简笔画（白底、黑/彩线条），它想表现的东西叫"X"。
请【只输出一段 JSON，不要解释、不要 markdown、不要 ``` 】，格式：{"caption":"名称","strokes":[...]}。
可用原语（prim）与参数 a（坐标空间 0-300，原点左上，x 向右、y 向下，主体居中在 (150,150) 附近）：
- line:   a=[x1,y1,x2,y2]
- circle: a=[cx,cy,r]        (r 8~40)
- wave:   a=[x1,y1,x2,y2,freq] (freq 2~5)
- star:   a=[cx,cy,R,spikes]  (spikes 5 或 6)
- scribble:a=[cx,cy,seed]     (seed 1~99999，表现蓬松/毛发)
- dot:    a=[cx,cy,r]          (实心点，画眼/斑/火星)
- tri:    a=[x1,y1,x2,y2,x3,y3] (实心三角，画角/耳)
- poly:   a=[x1,y1,x2,y2,...]  (开放折线轮廓，偶数≥4)
要求：1) 判断当前画是否真的像"X"；2) 不像就修正坐标让它更像，重点修掉错位/重叠/缺关键部位（如牛的角、羊的毛、动物的腿和眼、物体应有部件）；3) 颜色用符合真实的十六进制；4) 输出【完整】strokes（所有笔画都带上，不要只给改动的）；5) 只输出 JSON。"##;
    let prev = prev_json.unwrap_or_default();
    let user_text = format!("{}\n\n目标物体：{}\n\n上一版笔画（JSON）：{}\n\n如果上一版不像，请返回修正后的【完整】strokes。", instruction, prompt, prev);
    let payload = serde_json::json!({
        "model": model,
        "messages": [{
            "role": "user",
            "content": [
                {"type": "text", "text": user_text},
                {"type": "image_url", "image_url": {"url": format!("data:image/png;base64,{}", image_b64)}}
            ]
        }],
        "max_tokens": 1100,
        "temperature": 0.4
    });
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .map_err(|e| e.to_string())?;
    let resp = client
        .post("https://open.bigmodel.cn/api/paas/v4/chat/completions")
        .bearer_auth(key)
        .json(&payload)
        .send()
        .await
        .map_err(|e| format!("请求智谱视觉模型失败（网络/360 拦截？）：{}", e))?;
    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("智谱视觉返回 {}：{}", status.as_u16(), body));
    }
    let v: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
    let content = v
        .get("choices").and_then(|c| c.get(0)).and_then(|c| c.get("message"))
        .and_then(|m| m.get("content")).and_then(|c| c.as_str())
        .unwrap_or("（模型无返回）").to_string();
    Ok(content)
}

/// 聊天回复：把用户最近若干轮对话 + 当前输入发给智谱文本模型(GLM)，返回小火的回复文本。
/// history：[{ "role": "user" | "assistant", "content": "..." }] 的最近若干条，用于多轮上下文。
#[tauri::command]
async fn ask_reply(text: String, history: Option<Vec<serde_json::Value>>) -> Result<String, String> {
    let key = match std::env::var("ZHIPU_API_KEY") {
        Ok(k) if !k.is_empty() => k,
        _ => return Err("未配置 ZHIPU_API_KEY：请把 key 写入 .env（ZHIPU_API_KEY=xxx，.env 放在 exe 同目录或 src-tauri/ 下）并重启程序".into()),
    };
    let model = std::env::var("ZHIPU_CHAT_MODEL").unwrap_or_else(|_| "glm-4-flash".to_string());
    let system = "你是\"小火\"，一只住在用户桌面上的透明火柴人宠物。你可爱、真诚、话不多，用简体中文和用户聊天，回答简洁（1~3 句话），偶尔俏皮。你不会真的唱歌或画画，但会聊天、答疑、陪用户。不要使用 markdown，不要长篇大论。";
    let mut messages = vec![serde_json::json!({"role": "system", "content": system})];
    if let Some(h) = history {
        for m in h.iter().take(6) {
            let role = m.get("role").and_then(|v| v.as_str());
            let content = m.get("content").and_then(|v| v.as_str());
            if let (Some(role), Some(content)) = (role, content) {
                messages.push(serde_json::json!({"role": role, "content": content}));
            }
        }
    }
    messages.push(serde_json::json!({"role": "user", "content": text}));
    let payload = serde_json::json!({
        "model": model,
        "messages": messages,
        "max_tokens": 256,
        "temperature": 0.8
    });
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .map_err(|e| e.to_string())?;
    let resp = client
        .post("https://open.bigmodel.cn/api/paas/v4/chat/completions")
        .bearer_auth(key)
        .json(&payload)
        .send()
        .await
        .map_err(|e| format!("请求智谱失败（本机网络/360 防火墙拦截？）：{}", e))?;
    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("智谱返回 {}：{}", status.as_u16(), body));
    }
    let v: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
    let content = v
        .get("choices").and_then(|c| c.get(0)).and_then(|c| c.get("message"))
        .and_then(|m| m.get("content")).and_then(|c| c.as_str())
        .unwrap_or("（小火走神了）").to_string();
    Ok(content)
}

/// 进入/退出全屏框选：true=把主窗口全屏置顶并完全可交互；false=还原小窗并恢复穿透。
#[tauri::command]
fn toggle_pick(full: bool, app: tauri::AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        if full {
            // 只做「撑满 + 移到左上角 + 可交互」，绝不再碰 decorations / fullscreen / alwaysOnTop：
            // - 本窗配置已 decorations:false、alwaysOnTop:true，再调 set_decorations 会触发
            //   WebView2 重建 → 页面重载 → 框选层刚显示就被重置（黑窗"一闪而过"的真凶）；
            // - set_fullscreen(true) 同理会重建，且 transparent 窗 + 真全屏在 Windows 会闪退。
            // resizable:false 会让 set_size 撑不动 → 先临时放开。
            let _ = w.set_resizable(true);
            let sw = unsafe { GetSystemMetrics(SM_CXSCREEN) } as u32;
            let sh = unsafe { GetSystemMetrics(SM_CYSCREEN) } as u32;
            let sf = w.scale_factor().unwrap_or(1.0);
            let _ = w.set_size(tauri::Size::Physical(tauri::PhysicalSize::new(sw, sh)));
            let _ = w.set_position(tauri::Position::Physical(tauri::PhysicalPosition::new(0, 0)));
            let _ = w.set_ignore_cursor_events(false);
            eprintln!("[pick] 进入全屏框选: {}x{} (dpr {}, 已放开 resizable)", sw, sh, sf);
        } else {
            // 还原小窗（含左下角定位）+ 恢复穿透；不改 decorations/alwaysOnTop（配置本就是 false/true）
            let _ = w.set_size(tauri::Size::Logical(tauri::LogicalSize::new(480.0, 540.0)));
            if let Ok(Some(mon)) = w.current_monitor() {
                let msf = mon.scale_factor();
                let lh = mon.size().height as f64 / msf;
                let _ = w.set_position(tauri::LogicalPosition::new(24.0, (lh - 540.0 - 60.0).max(0.0)));
            }
            let _ = w.set_resizable(false);
            let _ = w.set_ignore_cursor_events(true);
            eprintln!("[pick] 退出框选，还原小窗 480x540");
        }
    }
    if let Some(st) = app.try_state::<AppState>() {
        *st.picking.lock().unwrap() = full;
    }
}
