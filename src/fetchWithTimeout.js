// 统一外部请求封装：超时 + 重试 + 缓存。维基 / Iconify / QD 三个 HTTP 服务共用。
// 注意：Tauri 的 invoke（Rust 桥，如 ask_glm / critique_draw）不走这里，
// 它的超时在 ai.js 单独用 AbortController 处理（HTTP fetch 无法 abort 原生 invoke）。
const _cache = new Map();

// 成功响应在 cache 启用时，包装成 { ok, status, json(), text() }，与原生 Response 接口兼容，
// 调用方用 r.ok / r.status / r.json() / r.text() 写法无需改动。
function wrap(r, text) {
  return {
    ok: r.ok, status: r.status, headers: r.headers,
    text: () => Promise.resolve(text),
    json: () => Promise.resolve(JSON.parse(text)),
  };
}

export async function fetchWithTimeout(url, opts = {}) {
  const { timeout = 7000, retries = 0, cache = false, ...fetchOpts } = opts;
  // 缓存命中：GET 文本缓存（默认 60s，或传毫秒自定义）
  if (cache) {
    const hit = _cache.get(url);
    if (hit && Date.now() - hit.t < (cache === true ? 60000 : cache)) {
      return wrap({ ok: true, status: 200 }, hit.text);
    }
  }
  const doFetch = async () => {
    const ac = new AbortController();
    const timer = setTimeout(() => ac.abort(), timeout);
    try { return await fetch(url, { ...fetchOpts, signal: ac.signal }); }
    finally { clearTimeout(timer); }
  };
  let lastErr;
  for (let i = 0; i <= retries; i++) {
    try {
      const r = await doFetch();
      if (cache && r.ok) {
        const text = await r.clone().text();
        _cache.set(url, { t: Date.now(), text });
        return wrap(r, text);
      }
      return r;
    } catch (e) {
      lastErr = e;
      if (i < retries) await new Promise(res => setTimeout(res, 300 * (i + 1)));
    }
  }
  throw lastErr || new Error('fetchWithTimeout failed: ' + url);
}
