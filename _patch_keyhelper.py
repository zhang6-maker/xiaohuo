# -*- coding: utf-8 -*-
import io
p = r'D:\xiaohuo\src-tauri\src\lib.rs'
s = io.open(p, encoding='utf-8').read()

# 1) 在 call_zhipu 之前加 effective_key 助手
anchor = "/// 统一智谱调用器：模型优先级 + 资源包耗尽自动降级 + 实际模型播报。\n"
helper = '''/// 实际生效的 API key：DeepSeek 模式读 DEEPSEEK_API_KEY，否则读 ZHIPU_API_KEY。
/// call_zhipu 内部也会自己按 provider 取 key，这里主要让外层调用点不误报"未配置智谱 key"。
fn effective_api_key() -> Option<String> {
    let provider = std::env::var("MODEL_PROVIDER")
        .unwrap_or_else(|_| "zhipu".to_string()).to_lowercase();
    let var = if provider == "deepseek" { "DEEPSEEK_API_KEY" } else { "ZHIPU_API_KEY" };
    std::env::var(var).ok().filter(|k| !k.is_empty())
}

'''
assert s.count(anchor) == 1, ('anchor', s.count(anchor))
s = s.replace(anchor, helper + anchor, 1)

# 2) 5处 key 读取替换（两种写法）
old1 = 'let key = match std::env::var("ZHIPU_API_KEY") {\n        Ok(k) if !k.is_empty() => k,'
new1 = 'let key = match effective_api_key() {\n        Some(k) => k,'
n1 = s.count(old1)
assert n1 == 4, ('match pattern', n1)
s = s.replace(old1, new1)

# 5th: underscore pattern
old2 = 'let key = match std::env::var("ZHIPU_API_KEY") {\n        Ok(k) if !k.is_empty() => k,\n        _ => return Err('
# already covered above if same; check remaining
remaining = s.count('std::env::var("ZHIPU_API_KEY")')
print('remaining direct reads:', remaining)
# 处理 _ => 写法（可能上面已替换完）
s = s.replace('let key = match std::env::var("ZHIPU_API_KEY") {\n        Ok(k) if !k.is_empty() => k,\n        _ => return Err(',
              'let key = match effective_api_key() {\n        Some(k) => k,\n        None => return Err(')

# 3) 错误信息改成中性（DeepSeek 模式不该提示智谱 key）
s = s.replace('"未配置 ZHIPU_API_KEY：请把 key 写入 .env（ZHIPU_API_KEY=xxx，.env 放在 exe 同目录或 src-tauri/ 下）并重启程序".into()',
              '"未配置 API Key：请在 .env 填 ZHIPU_API_KEY（智谱通道）或 DEEPSEEK_API_KEY + MODEL_PROVIDER=deepseek（DeepSeek 通道），.env 放 exe 同目录后重启".into()')

io.open(p, 'w', encoding='utf-8', newline='').write(s)
print('effective_key applied; match replaced', n1)
