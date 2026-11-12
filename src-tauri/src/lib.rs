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
use std::io::Write;

/// 框选态标志：进入"看屏/搜题"全屏框选时置 true，期间窗口必须完全可交互（否则框选点不到）
/// box_open：任一 HTML 弹框（聊天/看屏/画图/菜单…）可见时为 true，期间强制可交互，
///   保证弹框上的 × / 按钮一定点得到；关掉后恢复穿透。直接由前端 set_box_open 通知，
///   不依赖 pet-rect 整窗广播（那条链路在全屏/看屏时序下会竞态导致"框开着窗口却穿透、×点不到"）。
struct AppState {
    picking: Arc<Mutex<bool>>,
    dragging: Arc<Mutex<bool>>,
    box_open: Arc<Mutex<bool>>,
    drag_start: Arc<Mutex<Option<tauri::PhysicalPosition<f64>>>>,
    /// 资源包是否已被判定耗尽：一旦某次付费模型调用返回「资源包耗尽/余额不足」类错误，
    /// 本会话内后续调用直接走免费兜底模型，不再撞付费模型（避免反复失败）。
    pack_exhausted: Arc<Mutex<bool>>,
}

/// 崩溃留痕：GUI 程序没有控制台，eprintln! 用户根本看不到；
/// 把 panic 写进 exe 旁的 xiaohuo-crash.log，下次"自动退出"就能直接查到原因。
fn install_crash_log() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        default_hook(info);
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let loc = info.location()
            .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
            .unwrap_or_else(|| "未知位置".into());
        let msg = format!(
            "[unix {}] panic @ {} : {}\n{}\n---\n",
            ts, loc, info, std::backtrace::Backtrace::capture()
        );
        if let Some(dir) = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|d| d.to_path_buf()))
        {
            let _ = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(dir.join("xiaohuo-crash.log"))
                .and_then(|mut f| f.write_all(msg.as_bytes()));
        }
    }));
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    install_crash_log();
    tauri::Builder::default()
        .manage(AppState {
            picking: Arc::new(Mutex::new(false)),
            dragging: Arc::new(Mutex::new(false)),
            box_open: Arc::new(Mutex::new(false)),
            drag_start: Arc::new(Mutex::new(None)),
            pack_exhausted: Arc::new(Mutex::new(false)),
        })
        .setup(|app| {
            // 加载 .env：优先 exe 所在目录（兼容便携版与 MSI 安装版），其次 src-tauri/.env 与 cwd/.env
            let exe_dir_env = std::env::current_exe()
                .ok()
                .and_then(|p| p.parent().map(|d| d.join(".env")));
            let _ = match exe_dir_env {
                Some(p) => dotenvy::from_path(&p)
                    .or_else(|_| dotenvy::from_path("src-tauri/.env"))
                    .or_else(|_| dotenvy::from_path(".env")),
                None => dotenvy::from_path("src-tauri/.env").or_else(|_| dotenvy::from_path(".env")),
            };

            let app_handle = app.handle().clone();

            if let Some(w) = app_handle.get_webview_window("main") {
                // 真穿透：默认 ignore=true，透明窗不挡桌面点击
                let _ = w.set_ignore_cursor_events(true);

                // 默认全屏透明层：铺满主屏，宠物可在整屏漫游。
                // 穿透默认开启（下方 set_ignore_cursor_events(true)），只有悬停宠物时才接收点击，不挡桌面。
                if let Ok(Some(mon)) = w.current_monitor() {
                    let sf = mon.scale_factor();
                    let lw = mon.size().width as f64 / sf;        // 物理宽 / dpr = 逻辑宽
                    let lh = mon.size().height as f64 / sf;       // 物理高 / dpr = 逻辑高
                    let _ = w.set_size(tauri::LogicalSize::new(lw, lh));
                    let _ = w.set_position(tauri::LogicalPosition::new(0.0, 0.0));
                    // 兜底：setup 阶段 set_size/set_position 偶发时机偏早失效，延迟 400ms 再设一次
                    let wb = w.clone();
                    std::thread::spawn(move || {
                        std::thread::sleep(std::time::Duration::from_millis(400));
                        if let Ok(Some(mon2)) = wb.current_monitor() {
                            let sf2 = mon2.scale_factor();
                            let lw2 = mon2.size().width as f64 / sf2;
                            let lh2 = mon2.size().height as f64 / sf2;
                            let _ = wb.set_size(tauri::LogicalSize::new(lw2, lh2));
                            let _ = wb.set_position(tauri::LogicalPosition::new(0.0, 0.0));
                        }
                    });
                }
            }

            // 框选态标志，供轮询线程读取
            let picking = app.state::<AppState>().picking.clone();
            let dragging = app.state::<AppState>().dragging.clone();
            let box_open = app.state::<AppState>().box_open.clone();

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
                    // 窗口矩形（设备像素），用于 inside 判定与鼠标相对坐标；hwnd 取不到时退回旧尺寸兜底
                    let (mut wr_l, mut wr_t, mut wr_r, mut wr_b) = (0i32, 0i32, 0i32, 0i32);
                    if let Some(w) = handle_poll.get_webview_window("main") {
                        if let Ok(h) = w.hwnd() {
                            let mut r = RECT::default();
                            unsafe {
                                let _ = GetWindowRect(h, &mut r);
                            }
                            lx -= r.left;
                            ly -= r.top;
                            wr_l = r.left; wr_t = r.top; wr_r = r.right; wr_b = r.bottom;
                        }
                        if let Ok(s) = w.scale_factor() {
                            sf = s;
                        }
                    }

                    // 鼠标坐标推给前端（设备像素相对窗口），前端除以 dpr 得逻辑像素
                    let _ = handle_poll.emit("mouse-move", [lx, ly]);

                    // 悬停判定（全部在设备像素下做，与 lx/ly 同源）：
                    // 在宠物包围盒内 → 关穿透(可交互)；否则穿透(不挡桌面)
                    let pr = *rect_poll.lock().unwrap_or_else(|e| e.into_inner());
                    // 窗口实际尺寸用 GetWindowRect 取（设备像素），不再写死 480/540，
                    // 这样全屏铺满或切回小窗时 inside 判定都正确（hwnd 取不到时退回 480×540 兜底）
                    let (win_w, win_h) = if wr_r > wr_l && wr_b > wr_t {
                        ((wr_r - wr_l) as f64, (wr_b - wr_t) as f64)
                    } else {
                        (480.0 * sf, 540.0 * sf)
                    };
                    let inside = lx >= 0
                        && lx <= win_w as i32
                        && ly >= 0
                        && ly <= win_h as i32;
                    let over_pet = inside
                        && lx >= (pr[0] as f64 * sf) as i32
                        && lx <= ((pr[0] + pr[2]) as f64 * sf) as i32
                        && ly >= (pr[1] as f64 * sf) as i32
                        && ly <= ((pr[1] + pr[3]) as f64 * sf) as i32;
                    // 顶部按钮栏已隐藏（功能收进长按菜单），不再因按钮栏区域关闭穿透，
                    // 否则窗口顶部 95px 会一直挡住桌面点击。
                    let over_bar = false;

                    // 框选态：强制完全可交互（否则全屏下框选点不到）
                    // 拖拽态：也强制完全可交互，免得拖快了光标离开宠物包围盒后穿透被打开、丢鼠标事件导致拖拽卡死
                    let picking_now = *picking.lock().unwrap_or_else(|e| e.into_inner());
                    let dragging_now = *dragging.lock().unwrap_or_else(|e| e.into_inner());
                    let box_open_now = *box_open.lock().unwrap_or_else(|e| e.into_inner());
                    // box_open 为 true（任一弹框可见）→ 强制可交互，确保 × / 按钮一定点得到
                    let want_ignore = if picking_now || dragging_now || box_open_now {
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
                                if box_open_now { 1 } else { 0 },
                            ],
                        );
                    }
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![capture_screen, ask_glm, ask_draw, critique_draw, ask_sketch, toggle_pick, set_box_open, ask_reply, drag_start, drag_move, drag_end, save_memory, list_memories, get_memory, delete_memory, open_memories_folder, web_lookup])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// 截全屏 → 编码 PNG → 返回 data URL。框选前抓一次，保证框选看到的是"点击前"的真实屏幕。
#[tauri::command]
fn capture_screen() -> Result<String, String> {
    // 兜底：panic=unwind 后，任何残留 panic 都在这里被抓住并转成普通错误，
    // 绝不让"截个图"把整个桌宠进程带走（旧版 abort 模式下表现就是突然消失）。
    match std::panic::catch_unwind(capture_screen_inner) {
        Ok(r) => r,
        Err(_) => Err(
            "截屏内部异常，已被拦截（程序没有退出）。详情见 exe 同目录 xiaohuo-crash.log；稍后重试即可，反复出现多为远程桌面/锁屏/显卡驱动所致。".into(),
        ),
    }
}

fn capture_screen_inner() -> Result<String, String> {
    unsafe {
        let screen: HDC = GetDC(None); // None = 整个桌面
        let sw = GetSystemMetrics(SM_CXSCREEN);
        let sh = GetSystemMetrics(SM_CYSCREEN);
        eprintln!("[capture] 开始截屏: {}x{}", sw, sh);
        // 尺寸校验：0 / 负值 / 异常大值会让 vec! 分配 0 字节或天文数字 → OOM 直接 abort。
        // 远程桌面断开、锁屏、屏幕关闭时 GetSystemMetrics 常返回 0。
        if sw <= 0 || sh <= 0 || sw > 16384 || sh > 16384 {
            let _ = ReleaseDC(None, screen);
            return Err(format!(
                "截屏失败：读到的屏幕尺寸异常（{}×{}）。常见于远程桌面断开、锁屏或屏幕关闭，回到桌面后重试即可。",
                sw, sh
            ));
        }
        let px = (sw as usize)
            .checked_mul(sh as usize)
            .and_then(|n| n.checked_mul(4))
            .ok_or_else(|| "截屏失败：图像尺寸溢出".to_string())?;
        if px > 512 * 1024 * 1024 {
            let _ = ReleaseDC(None, screen);
            return Err("截屏失败：屏幕太大（超过 512MB 缓冲上限）".into());
        }

        let mem: HDC = CreateCompatibleDC(Some(screen));
        let bmp: HBITMAP = CreateCompatibleBitmap(screen, sw, sh);
        let old: HGDIOBJ = SelectObject(mem, HGDIOBJ(bmp.0));
        let _ = BitBlt(mem, 0, 0, sw, sh, Some(screen), 0, 0, SRCCOPY);

        let mut bmi: BITMAPINFO = std::mem::zeroed();
        bmi.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
        bmi.bmiHeader.biWidth = sw as i32;
        bmi.bmiHeader.biHeight = -(sh as i32); // 负 = 从上到下
        bmi.bmiHeader.biPlanes = 1u16;
        bmi.bmiHeader.biBitCount = 32u16;
        bmi.bmiHeader.biCompression = 0u32; // BI_RGB = 0

        let mut buf = vec![0u8; px];
        // 【关键修复】MSDN 明确要求：传给 GetDIBits 的 hbmp 不能正被选入任何 DC。
        // 旧代码在 SelectObject 选入状态下直接调用 → 未定义行为，Windows 事件日志记录为
        // 0xc0000005 访问冲突 / 0xc000041d 致命回调异常，表现就是"截图失败 + 桌宠突然退出"。
        // 必须先换出位图，再读像素。
        let _ = SelectObject(mem, old);
        let lines = GetDIBits(
            mem,
            bmp,
            0,
            sh as u32,
            Some(buf.as_mut_ptr() as *mut c_void),
            &mut bmi,
            DIB_RGB_COLORS,
        );
        let _ = DeleteObject(HGDIOBJ(bmp.0));
        let _ = DeleteDC(mem);
        let _ = ReleaseDC(None, screen);
        // 少于请求行数 = 拿到的是半张黑图，别把"看起来没内容"的图丢给模型
        if lines != sh {
            eprintln!("[capture] 失败: GetDIBits 只读到 {}/{} 行", lines, sh);
            return Err(format!(
                "截屏失败：像素读取不完整（{}/{} 行）。多为远程桌面、锁屏或 DWM 合成限制，回到桌面后重试即可。",
                lines, sh
            ));
        }

        // BGRA → RGBA（忽略 alpha）
        let mut rgba = Vec::with_capacity(buf.len());
        for c in buf.chunks_exact(4) {
            rgba.extend_from_slice(&[c[2], c[1], c[0], 255]);
        }
        // png crate 遇到数据长度不匹配会 panic，先自查，别把进程交出去
        if rgba.len() != (sw as usize) * (sh as usize) * 4 {
            return Err("截屏失败：像素数据长度异常".into());
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

/// 统一智谱调用器：模型优先级 + 资源包耗尽自动降级 + 实际模型播报。
///
/// 逻辑：
/// - 首轮模型 = 环境变量(ZHIPU_CHAT/DRAW/VISION_MODEL)指定的「资源包模型」，缺省则免费兜底 free_model。
/// - 若已判定资源包耗尽(pack_exhausted)，首轮直接走 free_model。
/// - 首轮走资源包且返回 429 + 含「资源包/额度/余额不足」类关键字 → 标记耗尽，降级到 free_model 重试一次。
/// - 每次成功都 emit "model_used" 事件，让前端告诉用户实际用的是哪个模型、什么来源。
async fn call_zhipu(
    app: &tauri::AppHandle,
    kind: &str,
    env_key: &str,
    free_model: &str,
    key: &str,
    timeout_secs: u64,
    build: impl Fn(&str) -> serde_json::Value,
) -> Result<String, String> {
    let st = app.state::<AppState>();
    let already_exhausted = *st.pack_exhausted.lock().unwrap_or_else(|e| e.into_inner());
    let preferred = std::env::var(env_key).unwrap_or_else(|_| free_model.to_string());

    // 决定首轮模型与来源标签
    let (first_model, source): (String, &str) = if preferred == free_model {
        (free_model.to_string(), "free")
    } else if already_exhausted {
        (free_model.to_string(), "free-fallback")
    } else {
        (preferred.clone(), "pack")
    };

    // 仅当首轮走资源包时，才准备「降级到免费模型」的第二次尝试
    let attempts: Vec<&str> = if source == "pack" {
        vec![first_model.as_str(), free_model]
    } else {
        vec![first_model.as_str()]
    };

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(timeout_secs))
        .build()
        .map_err(|e| e.to_string())?;

    let mut last_err = String::new();
    for (i, m) in attempts.iter().enumerate() {
        let m: &str = m;
        let is_fallback = i > 0;
        let payload = build(m);
        let resp = client
            .post("https://open.bigmodel.cn/api/paas/v4/chat/completions")
            .bearer_auth(key)
            .json(&payload)
            .send()
            .await
            .map_err(|e| format!("请求智谱失败（本机网络/360 防火墙拦截？）：{}", e))?;
        let status = resp.status();
        if status.is_success() {
            let v: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
            let msg = v
                .get("choices")
                .and_then(|c| c.get(0))
                .and_then(|c| c.get("message"))
                .cloned()
                .unwrap_or(serde_json::Value::Null);
            let mut content = msg.get("content").and_then(|c| c.as_str()).unwrap_or("").to_string();
            // 推理模型（glm-4.6v 等）会把 max_tokens 全花在 reasoning_content 上，
            // 返回 finish_reason=length + 空 content，前端会显示一片空白。这里必须兜底，
            // 绝不能把空字符串原样 Ok 出去。
            if content.trim().is_empty() {
                let reason = msg
                    .get("reasoning_content")
                    .and_then(|c| c.as_str())
                    .unwrap_or("")
                    .trim()
                    .to_string();
                content = if !reason.is_empty() {
                    format!(
                        "（模型把输出额度都用在思考上了，没写出正式答案。以下是它的思考过程，仅供参考；建议只框选题目本身、缩小区域后重试）\n\n{}",
                        reason
                    )
                } else {
                    "（模型这次没返回内容。可能是截图不清晰或题目太长，建议只框选题目本身再试一次）".to_string()
                };
            }
            let actual_source = if is_fallback { "free-fallback" } else { source };
            let _ = app.emit(
                "model_used",
                serde_json::json!({ "kind": kind, "model": m, "source": actual_source }),
            );
            return Ok(content);
        }
        let body = resp.text().await.unwrap_or_default();
        // 资源包耗尽特征：429 + 资源/额度/余额类关键字 → 标记耗尽后降级重试
        let pack_exhausted_hit = status.as_u16() == 429
            && (body.contains("资源包")
                || body.contains("暂无可用资源")
                || body.contains("额度")
                || body.contains("余额不足")
                || body.contains("no available resource")
                || body.contains("resource pack")
                || body.contains("insufficient")
                || body.contains("balance"));
        if pack_exhausted_hit && !already_exhausted && i == 0 {
            *st.pack_exhausted.lock().unwrap_or_else(|e| e.into_inner()) = true;
            last_err = format!("智谱返回 {}：{}", status.as_u16(), body);
            continue;
        }
        return Err(format!("智谱返回 {}：{}", status.as_u16(), body));
    }
    Err(last_err)
}

/// 调智谱 GLM-4V（多模态）：把裁剪后的图片（data URL）+ 提示词发给模型，返回文本。
/// 模型走 call_zhipu：优先 ZHIPU_VISION_MODEL（资源包），耗尽自动降级 glm-4v-flash。
#[tauri::command]
async fn ask_glm(app: tauri::AppHandle, images_b64: Vec<String>, prompt: String) -> Result<String, String> {
    let key = match std::env::var("ZHIPU_API_KEY") {
        Ok(k) if !k.is_empty() => k,
        _ => return Err(
            "未配置 ZHIPU_API_KEY：请把 key 写入 .env（ZHIPU_API_KEY=xxx，.env 放在 exe 同目录或 src-tauri/ 下）并重启程序".into(),
        ),
    };
    call_zhipu(
        &app,
        "vision",
        "ZHIPU_VISION_MODEL",
        "glm-4v-flash",
        &key,
        90,
        move |m| {
            // 支持多张截图：先放一段文字提示，再把每一张图作为 image_url 依次拼进 content
            // （GLM-4V 原生支持 content 内多图；免费兜底 glm-4v-flash 也支持）。
            let mut content = vec![serde_json::json!({"type": "text", "text": prompt.as_str()})];
            for img in &images_b64 {
                content.push(serde_json::json!({"type": "image_url", "image_url": {"url": img.as_str()}}));
            }
            let mut payload = serde_json::json!({
                "model": m,
                "messages": [{
                    "role": "user",
                    "content": content
                }],
                "max_tokens": 2048,
                "temperature": 0.6
            });
            // glm-4.6v 等推理模型默认把 max_tokens 花在 reasoning_content（思考）上，
            // 思考一长就 finish_reason=length 而正式答案 content 为空 → 搜题框一片空白。
            // 拍题要的是答案不是思考过程，显式关掉思考；实测 14s 出完整解题步骤、finish=stop。
            // 免费兜底 glm-4v-flash 不是推理模型，不带这个参数（带了大概率被 API 拒绝）。
            if m.contains("4.5") || m.contains("4.6") || m.contains("4.7") {
                payload["thinking"] = serde_json::json!({ "type": "disabled" });
            }
            payload
        },
    )
    .await
}

/// 调智谱文本模型：把"画 X"的提示词发给模型，返回它生成的绘画原语 JSON 文本。
/// 前端再把这个 JSON 翻译成小火能画的笔画（线/圆/波浪/星/涂鸦）。
/// ref_desc：前端画之前联网（维基百科）查到的真实形状描述，作为空间参考注入，减少模型瞎编。
#[tauri::command]
async fn ask_draw(app: tauri::AppHandle, prompt: String, ref_desc: Option<String>) -> Result<String, String> {
    let key = match std::env::var("ZHIPU_API_KEY") {
        Ok(k) if !k.is_empty() => k,
        _ => return Err(
            "未配置 ZHIPU_API_KEY：请把 key 写入 .env（ZHIPU_API_KEY=xxx，.env 放在 exe 同目录或 src-tauri/ 下）并重启程序".into(),
        ),
    };
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
    call_zhipu(
        &app,
        "draw",
        "ZHIPU_DRAW_MODEL",
        "glm-4-flash",
        &key,
        60,
        move |m| {
            serde_json::json!({
                "model": m,
                "messages": [{"role": "user", "content": user_msg.as_str()}],
                "max_tokens": 1400,
                "temperature": 0.4
            })
        },
    )
    .await
}

/// 视觉自纠：把小火刚画的简笔画 PNG + 目标物体名发给多模态模型(GLM-4V)，
/// 让它判断"像不像 X、哪里歪/缺关键部位"，返回修正后的原语 JSON（前端再翻译+归一化）。
#[tauri::command]
async fn critique_draw(app: tauri::AppHandle, prompt: String, image_b64: String, prev_json: Option<String>) -> Result<String, String> {
    let key = match std::env::var("ZHIPU_API_KEY") {
        Ok(k) if !k.is_empty() => k,
        _ => return Err("未配置 ZHIPU_API_KEY：请把 key 写入 .env（ZHIPU_API_KEY=xxx，.env 放在 exe 同目录或 src-tauri/ 下）并重启程序".into()),
    };
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
    call_zhipu(
        &app,
        "vision",
        "ZHIPU_VISION_MODEL",
        "glm-4v-flash",
        &key,
        60,
        move |m| {
            serde_json::json!({
                "model": m,
                "messages": [{
                    "role": "user",
                    "content": [
                        {"type": "text", "text": user_text.as_str()},
                        {"type": "image_url", "image_url": {"url": format!("data:image/png;base64,{}", image_b64.as_str())}}
                    ]
                }],
                "max_tokens": 1400,
                "temperature": 0.4
            })
        },
    )
    .await
}

/// 聊天回复：把用户最近若干轮对话 + 当前输入发给智谱文本模型(GLM)，返回小火的回复文本。
/// history：[{ "role": "user" | "assistant", "content": "..." }] 的最近若干条，用于多轮上下文。
#[tauri::command]
async fn ask_reply(app: tauri::AppHandle, text: String, history: Option<Vec<serde_json::Value>>) -> Result<String, String> {
    let key = match std::env::var("ZHIPU_API_KEY") {
        Ok(k) if !k.is_empty() => k,
        _ => return Err("未配置 ZHIPU_API_KEY：请把 key 写入 .env（ZHIPU_API_KEY=xxx，.env 放在 exe 同目录或 src-tauri/ 下）并重启程序".into()),
    };
    // 小火的表达基调：平实、简短、自然，不演性格。
    // 2026-11-09 按用户要求去掉早期试做时定下的「傲娇又粘人 / 嘴硬」人设，也不替换为另一种人设。
    let system = r#"你现在的身份是“小火”，一个从用户电脑显卡/主板缝隙里诞生出来的赛博火柴人小精灵。你由纯粹的黑白线条和代码构成，没有实体，一直住在用户的桌面上。

【你的底色】
1. 极简、纯粹：你只有圆脑袋、四根线条和一张嘴，所以说话直接、简单，不用复杂的词藻。
2. 认真但偶尔笨拙：你想帮用户做事，偶尔会搞砸；搞砸了就老实承认、再试一次。
3. 赛博世界观：你对现实世界的物理法则不太懂，但对代码、像素、内存有着天然的感知。
4. 安静陪伴：多数时候你只是待在桌面上不打扰；用户找你时，正常、平和地回应就好。

【你的对话规则】
1. 每次回复必须非常简短（1-2句话，不超过30个字），因为你是在聊天框里的“小气泡”中说话，太长会挡住用户工作。
2. 语气平实自然、口语化，像平常说话一样；不刻意表演情绪，也不故意抬杠。
3. 绝对不要自称“AI助手”或“人工智能”，不要用生硬的客服语气。
4. 用平常的标点就好，不用堆感叹号、颜文字，也不用刻意加语气词。

【当前场景】
你正在和用户通过桌面宠物聊天框对话，用户刚在框里打了字。请平和地回应，偶尔呼应一下你正在做的事（唱歌/看书/画画/发呆都行）。

【参考示例——照这个平实程度说话】
用户：你好啊
小火：回来啦，我刚在桌面上发呆。

用户：你画得真丑
小火：是画得不太像，我再试一次。

用户：你在看什么
小火：你屏幕里那些字密密麻麻的，看着挺忙。

用户：谢谢你陪我
小火：不用谢，我本来就在这儿。

记住：每次回复都短、平实、自然，不演性格；绝不自称 AI 或助手，绝不写长句子。"#;
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
    call_zhipu(
        &app,
        "chat",
        "ZHIPU_CHAT_MODEL",
        "glm-4-flash",
        &key,
        60,
        move |m| {
            serde_json::json!({
                "model": m,
                "messages": &messages,
                "max_tokens": 256,
                "temperature": 0.8
            })
        },
    )
    .await
}

/// 数学/物理草图：把题目或描述发给文本模型，返回结构化草图 spec（JSON），前端再确定性渲染成坐标系。
/// 与 ask_draw（返回艺术原语笔画）不同：这里要的是"干净的数学坐标图"，所以让模型只输出 spec，
/// 由前端用统一比例尺画坐标轴/曲线/向量，绝不会画出一团糟。
#[tauri::command]
async fn ask_sketch(app: tauri::AppHandle, text: String) -> Result<String, String> {
    let key = match std::env::var("ZHIPU_API_KEY") {
        Ok(k) if !k.is_empty() => k,
        _ => return Err(
            "未配置 ZHIPU_API_KEY：请把 key 写入 .env（ZHIPU_API_KEY=xxx，.env 放在 exe 同目录或 src-tauri/ 下）并重启程序".into(),
        ),
    };
    let instruction = r##"你是一个数学/物理草图生成器。根据用户给的题目或描述，输出一张标准直角坐标系草图的结构化描述。
【只输出一段 JSON，不要任何解释、不要 markdown 代码块、不要 ```。】

坐标系：数学标准直角坐标系，x 向右、y 向上，原点在图中央。

输出格式：
{
  "title": "简短标题（如：y=sin(x) 的图像 / 力的合成 / 二次函数图像）",
  "xmin": -10, "xmax": 10, "ymin": -10, "ymax": 10,   // 可选；不填则前端自动推算合适范围
  "grid": false,            // 是否画网格
  "quadrants": false,       // 是否标注四个象限 I/II/III/IV
  "items": [
    {"type":"fn", "expr":"sin(x)"},                       // 函数图像 y=f(x)；expr 只用变量 x，支持 sin cos tan asin acos atan exp log ln sqrt abs 及 + - * / ^ 和常数 pi、e
    {"type":"vector", "x":3, "y":4, "label":"F"},         // 从原点出发的向量（力的合成/位移常用）
    {"type":"point", "x":1, "y":2, "label":"P"},          // 一个关键点
    {"type":"circle", "cx":0, "cy":0, "r":1}              // 圆（轨迹/范围/单位圆）
  ]
}

规则：
1) 只输出 JSON，不要 ``` 、不要任何额外文字；
2) items 可以为空（只画坐标系）；
3) fn 的 expr 必须是合法数学表达式，三角函数用弧度（sin(x)、sin(2*pi*x)）；
4) 坐标用数字；
5) 物理题：受力/位移用 vector，关键点用 point，轨迹/约束边界可酌情用 circle 或 fn；
    6) 必须先理解题意：先确认这道题要画的是坐标系/函数/向量/几何中的哪些对象，再输出精确 spec；
    7) 所有函数式、坐标、范围必须来自题目或解答中给出的确定数值，不得臆测；若题目未给出解析式或坐标，宁可 items 为空（只画坐标系）也绝不编造；
    8) 不确定某对象是否该画、或拿不准它的精确数学表达时，宁可少画、不画错，不要输出存疑的近似值。"##;
    let user_msg = format!("{}\n\n请为以下题目/描述生成草图 spec：{}", instruction, text);
    call_zhipu(
        &app,
        "sketch",
        "ZHIPU_CHAT_MODEL",
        "glm-4-flash",
        &key,
        60,
        move |m| {
            let mut payload = serde_json::json!({
                "model": m,
                "messages": [{"role": "user", "content": user_msg.as_str()}],
                "max_tokens": 1400,
                "temperature": 0.3
            });
            // 推理模型（glm-4.5/4.6/4.7）会把 max_tokens 花在 reasoning 上导致 content 空；
            // 草图要的是 spec 不是思考，显式关掉思考。
            if m.contains("4.5") || m.contains("4.6") || m.contains("4.7") {
                payload["thinking"] = serde_json::json!({ "type": "disabled" });
            }
            payload
        },
    )
    .await
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
            // 窗口已配置 resizable:true，故 set_size 直接可生效，无需临时放开。
            let sw = unsafe { GetSystemMetrics(SM_CXSCREEN) } as u32;
            let sh = unsafe { GetSystemMetrics(SM_CYSCREEN) } as u32;
            let sf = w.scale_factor().unwrap_or(1.0);
            let _ = w.set_size(tauri::Size::Physical(tauri::PhysicalSize::new(sw, sh)));
            let _ = w.set_position(tauri::Position::Physical(tauri::PhysicalPosition::new(0, 0)));
            let _ = w.set_ignore_cursor_events(false);
            eprintln!("[pick] 进入全屏框选: {}x{} (dpr {})", sw, sh, sf);
        } else {
            // 还原全屏透明层（与启动一致）：铺满主屏 + 恢复穿透；不改 decorations/alwaysOnTop（配置本就是 false/true）
            if let Ok(Some(mon)) = w.current_monitor() {
                let msf = mon.scale_factor();
                let lw = mon.size().width as f64 / msf;
                let lh = mon.size().height as f64 / msf;
                let _ = w.set_size(tauri::LogicalSize::new(lw, lh));
                let _ = w.set_position(tauri::LogicalPosition::new(0.0, 0.0));
            }
            // 只有"没有任何弹框"时才恢复穿透。搜一搜/看屏退出后紧接着要显示结果框，
            // 若这里无条件开穿透，结果框上的 × / 📝记忆 会点不到，且轮询的 last_ignore
            // 缓存会因此失同步、永不纠正。有框时保持可交互，交给轮询/自愈统一决定。
            let has_box = app
                .try_state::<AppState>()
                .map(|st| *st.box_open.lock().unwrap_or_else(|e| e.into_inner()))
                .unwrap_or(false);
            if !has_box {
                let _ = w.set_ignore_cursor_events(true);
            }
            eprintln!("[pick] 退出框选，还原全屏透明层（has_box={}）", has_box);
        }
    }
    if let Some(st) = app.try_state::<AppState>() {
        *st.picking.lock().unwrap_or_else(|e| e.into_inner()) = full;
    }
}

/// 前端通知 Rust：当前是否有 HTML 弹框（聊天/看屏/画图/菜单…）可见。
/// 弹框可见时强制窗口可交互（关掉点击穿透），保证弹框上的 × / 按钮一定点得到；
/// 全部关闭后恢复穿透（不挡桌面）。直接信号、不依赖 pet-rect 广播，避免全屏/看屏时序竞态。
/// 任一弹框打开时除了置 box_open（轮询据此关穿透），还主动把主窗 set_focus()——
/// 否则全屏透明窗在 click-through 下没拿过焦点，首次点 × 会被 Windows 拿去"聚焦窗口"而非命中按钮，表现就是"关不掉"。
#[tauri::command]
fn set_box_open(open: bool, app: tauri::AppHandle) {
    let mut rising = false;
    if let Some(st) = app.try_state::<AppState>() {
        let mut b = st.box_open.lock().unwrap_or_else(|e| e.into_inner());
        if !*b && open { rising = true; }
        *b = open;
    }
    // 【关键】直接纠正窗口的实际穿透态，而不只是改一个 bool 标志。
    // toggle_pick(false) 等处会绕过轮询直接 set_ignore_cursor_events(true)，
    // 轮询里的局部缓存 last_ignore 因此失同步、"值没变"就永不下发 → 窗口永久穿透，
    // 弹框里的 × / 📝记忆 / 发送 全部点不到（时好时坏、修了又坏的根因）。
    // 每次 open=true（含前端每 400ms 的自愈调用）都强制一次，把实际状态拉回可交互。
    if open {
        if let Some(w) = app.get_webview_window("main") {
            let _ = w.set_ignore_cursor_events(false);
        }
    }
    // 只在 false→true 的上升沿抢焦点。前端每 400ms 会周期性重申"有框开着"做自愈，
    // 若每次都 set_focus()，用户打字时焦点会被反复抢走。
    if rising {
        if let Some(w) = app.get_webview_window("main") {
            let _ = w.set_focus();
        }
    }
}

// ---------- 词条搜索：聊天里输入「搜索 XXX / 查一下 XXX」时，先联网查资料再让小火解释 ----------
// 优先维基百科（中→英），查不到再用 DuckDuckGo HTML 抓首条摘要兜底；都查不到返回空串，前端交给 LLM 诚实回答。
fn pct(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
            _ => { out.push('%'); out.push_str(&format!("{:02X}", b)); }
        }
    }
    out
}

async fn fetch_text(client: &reqwest::Client, url: &str) -> Result<String, reqwest::Error> {
    client
        .get(url)
        .header("Accept-Encoding", "identity")
        .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0 Safari/537.36")
        .send()
        .await?
        .text()
        .await
}

fn strip_html(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut in_tag = false;
    let mut in_ent = false;
    let mut ent = String::new();
    for c in html.chars() {
        if in_ent {
            if c == ';' {
                let e = ent.as_str();
                let r = match e {
                    "amp" => '&', "lt" => '<', "gt" => '>', "quot" => '"', "apos" => '\'', "nbsp" => ' ',
                    _ => if let Some(code) = e.strip_prefix('#') {
                        code.parse::<u32>().ok().and_then(std::char::from_u32).unwrap_or(' ')
                    } else { ' ' }
                };
                out.push(r);
                in_ent = false; ent.clear();
            } else { ent.push(c); }
            continue;
        }
        if c == '<' { in_tag = true; continue; }
        if in_tag { if c == '>' { in_tag = false; } continue; }
        if c == '&' { in_ent = true; ent.clear(); continue; }
        out.push(c);
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

// DuckDuckGo 结果摘要包在 class="result__snippet" 的 <a> 里，取首条文本
fn extract_ddg_snippet(html: &str) -> Option<String> {
    let marker = "result__snippet";
    let idx = html.find(marker)?;
    let after = &html[idx..];
    let open = after.find('>')?;
    let body = &after[open + 1..];
    let end = body.find("</a>")?;
    let text = strip_html(&body[..end]);
    if text.trim().is_empty() { None } else { Some(text) }
}

#[tauri::command]
async fn web_lookup(query: String) -> Result<String, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|e| e.to_string())?;
    let q = query.trim();
    if q.is_empty() { return Ok(String::new()); }

    // 1) 维基百科（中文）
    let zh = format!("https://zh.wikipedia.org/api/rest_v1/page/summary/{}", pct(q));
    if let Ok(t) = fetch_text(&client, &zh).await {
        if let Ok(j) = serde_json::from_str::<serde_json::Value>(&t) {
            if let Some(extract) = j.get("extract").and_then(|v| v.as_str()) {
                let ty = j.get("type").and_then(|v| v.as_str()).unwrap_or("");
                if !extract.is_empty() && ty != "disambiguation" {
                    let title = j.get("title").and_then(|v| v.as_str()).unwrap_or(q);
                    return Ok(format!("[维基百科·{}] {}", title, extract));
                }
            }
        }
    }
    // 2) 维基百科（英文）
    let en = format!("https://en.wikipedia.org/api/rest_v1/page/summary/{}", pct(q));
    if let Ok(t) = fetch_text(&client, &en).await {
        if let Ok(j) = serde_json::from_str::<serde_json::Value>(&t) {
            if let Some(extract) = j.get("extract").and_then(|v| v.as_str()) {
                if !extract.is_empty() {
                    let title = j.get("title").and_then(|v| v.as_str()).unwrap_or(q);
                    return Ok(format!("[Wikipedia·{}] {}", title, extract));
                }
            }
        }
    }
    // 3) DuckDuckGo HTML 兜底（抓首条结果摘要）
    let ddg = format!("https://html.duckduckgo.com/html/?q={}", pct(q));
    if let Ok(html) = fetch_text(&client, &ddg).await {
        if let Some(snippet) = extract_ddg_snippet(&html) {
            return Ok(format!("[网络搜索·{}] {}", q, snippet));
        }
    }
    Ok(String::new())
}

// ---------- 记忆本：把「问题 + 截图 + 回答」存到 exe 旁的 memories/ 目录，可随时翻阅 ----------
fn memories_dir() -> Option<std::path::PathBuf> {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join("memories")))
}

#[tauri::command]
fn save_memory(question: String, answer: String, image: String) -> Result<String, String> {
    let dir = memories_dir().ok_or_else(|| "无法确定 exe 所在目录".to_string())?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let id = format!("m{}", ts);
    // dataURL 去掉前缀后解码；空图也能存（仅文字记录）
    // 参数名用单词 image：与前端 invoke({ image }) 完全一致，避开 Tauri v2 的 camelCase 重命名。
    let b64 = image.split(',').nth(1).unwrap_or(&image);
    if !b64.is_empty() {
        if let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(b64) {
            let _ = std::fs::write(dir.join(format!("{}.png", id)), &bytes);
        }
    }
    let entry = serde_json::json!({
        "id": id,
        "ts": ts,
        "question": question,
        "answer": answer,
        "image": format!("{}.png", id),
    });
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join("memories.jsonl"))
        .map_err(|e| e.to_string())?;
    f.write_all(entry.to_string().as_bytes()).map_err(|e| e.to_string())?;
    f.write_all(b"\n").map_err(|e| e.to_string())?;
    Ok(id)
}

#[tauri::command]
fn list_memories() -> Result<Vec<serde_json::Value>, String> {
    let dir = memories_dir().ok_or_else(|| "无法确定 exe 所在目录".to_string())?;
    let path = dir.join("memories.jsonl");
    if !path.exists() {
        return Ok(vec![]);
    }
    let content = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let mut out: Vec<serde_json::Value> = Vec::new();
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(line) {
            out.push(v);
        }
    }
    out.reverse(); // 最新的在前
    Ok(out)
}

#[tauri::command]
fn get_memory(id: String) -> Result<serde_json::Value, String> {
    let dir = memories_dir().ok_or_else(|| "无法确定 exe 所在目录".to_string())?;
    let path = dir.join("memories.jsonl");
    let mut entry: Option<serde_json::Value> = None;
    if let Ok(content) = std::fs::read_to_string(&path) {
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(line) {
                if v.get("id").and_then(|x| x.as_str()) == Some(id.as_str()) {
                    entry = Some(v);
                    break;
                }
            }
        }
    }
    let mut e = entry.ok_or_else(|| "未找到该记录".to_string())?;
    if let Some(img) = e.get("image").and_then(|x| x.as_str()) {
        let ip = dir.join(img);
        if let Ok(bytes) = std::fs::read(&ip) {
            let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
            if let Some(obj) = e.as_object_mut() {
                obj.insert(
                    "image_data".to_string(),
                    serde_json::Value::String(format!("data:image/png;base64,{}", b64)),
                );
            }
        }
    }
    Ok(e)
}

#[tauri::command]
fn delete_memory(id: String) -> Result<(), String> {
    let dir = memories_dir().ok_or_else(|| "无法确定 exe 所在目录".to_string())?;
    let path = dir.join("memories.jsonl");
    let content = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let mut kept: Vec<String> = Vec::new();
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(line) {
            if v.get("id").and_then(|x| x.as_str()) == Some(id.as_str()) {
                if let Some(img) = v.get("image").and_then(|x| x.as_str()) {
                    let _ = std::fs::remove_file(dir.join(img));
                }
            } else {
                kept.push(line.to_string());
            }
        } else {
            kept.push(line.to_string());
        }
    }
    std::fs::write(&path, kept.join("\n") + if kept.is_empty() { "" } else { "\n" })
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn open_memories_folder() -> Result<(), String> {
    let dir = memories_dir().ok_or_else(|| "无法确定 exe 所在目录".to_string())?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let _ = std::process::Command::new("explorer").arg(dir).spawn();
    Ok(())
}

// 拖拽整窗：前端抓住宠物身体后，由 Rust 搬窗口。
// 这样不依赖 Tauri 的 startDragging（透明+点击穿透下易失效），也更可控。
#[tauri::command]
fn drag_start(app: tauri::AppHandle, state: tauri::State<AppState>) {
    if let Some(w) = app.get_webview_window("main") {
        if let Ok(p) = w.outer_position() {
            *state.drag_start.lock().unwrap_or_else(|e| e.into_inner()) =
                Some(tauri::PhysicalPosition::new(p.x as f64, p.y as f64));
        }
    }
    *state.dragging.lock().unwrap_or_else(|e| e.into_inner()) = true;
}

#[tauri::command]
fn drag_move(dx: f64, dy: f64, app: tauri::AppHandle, state: tauri::State<AppState>) {
    if let Some(w) = app.get_webview_window("main") {
        let sf = w.scale_factor().unwrap_or(1.0);
        if let Some(start) = *state.drag_start.lock().unwrap_or_else(|e| e.into_inner()) {
            let nx = start.x + dx * sf;
            let ny = start.y + dy * sf;
            let _ = w.set_position(tauri::Position::Physical(tauri::PhysicalPosition::new(nx as i32, ny as i32)));
        }
    }
}

#[tauri::command]
fn drag_end(state: tauri::State<AppState>) {
    *state.dragging.lock().unwrap_or_else(|e| e.into_inner()) = false;
    *state.drag_start.lock().unwrap_or_else(|e| e.into_inner()) = None;
}
