# -*- coding: utf-8 -*-
import io
p = r"D:\xiaohuo\index.html"
lines = io.open(p, encoding='utf-8').read().split('\n')

# 定位 noteBox 起始与结束（第 491 行是 </div>）
start = None
for i, ln in enumerate(lines):
    if ln.startswith('<div id="noteBox" class="box hidden"'):
        start = i
        break
assert start is not None
end = None
for j in range(start, start + 40):
    if lines[j] == '</div>':
        end = j
        break
assert end is not None, 'end not found'
print('replace lines', start + 1, '..', end + 1)

BTN = ('background:%s;border:1px solid %s;color:#f3f3f5;border-radius:5px;'
       'padding:3px 4px;cursor:pointer;font-size:11px;')
NEW = '''<div id="noteBox" class="box hidden" style="inset:0;max-width:none;width:100%;height:100%;background:transparent;border:none;box-shadow:none;padding:0;pointer-events:none;z-index:300;">
  <!-- 合并后的单一面板：拖标题栏移动、▾ 收起、位置记忆；占屏只有右上角一小块 -->
  <div id="notePanel" style="position:fixed;left:calc(100vw - 268px);top:10px;width:258px;max-height:calc(100vh - 20px);display:flex;flex-direction:column;pointer-events:auto;background:rgba(28,28,32,.96);border:1px solid #3a3a40;border-radius:10px;box-shadow:0 10px 28px rgba(0,0,0,.5);z-index:304;font-size:12px;color:#f3f3f5;">
    <div id="notePanelHead" style="display:flex;align-items:center;gap:6px;padding:7px 9px;cursor:move;user-select:none;flex:none;">
      <span style="flex:1;white-space:nowrap;overflow:hidden;text-overflow:ellipsis;color:#cfd2d8;">🖋️ 跟随笔记</span>
      <button id="noteCollapse" title="收起 / 展开" style="background:#3a3a42;border:1px solid #55555f;color:#f3f3f5;border-radius:5px;padding:1px 7px;cursor:pointer;font-size:11px;line-height:1.2;">▾</button>
      <button class="close" style="position:static;font-size:15px;line-height:1;color:#fff;background:rgba(60,60,68,.9);border:none;border-radius:5px;padding:2px 7px;cursor:pointer;">×</button>
    </div>
    <div id="notePanelBody" style="padding:0 9px 9px;overflow:auto;">
      <div id="noteBar" style="display:flex;flex-wrap:wrap;gap:5px;align-items:center;padding:2px 0 7px;border-bottom:1px solid #3a3a40;">
        <div class="swatches" id="noteSwatches" style="display:flex;flex-wrap:wrap;gap:4px;"></div>
        <input type="color" id="notePicker" value="#ffd24d" style="width:26px;height:22px;border:none;background:none;cursor:pointer;padding:0;" />
        <input type="range" id="noteSize" min="1" max="40" value="4" style="width:58px;accent-color:#6ab0ff;" />
        <button id="noteEraser" style="%s">🧽</button>
        <button id="notePin" style="%s">📌 重钉</button>
        <button id="noteDone" style="%s">完成</button>
      </div>
      <div id="noteListTop" style="padding:7px 0 0;">
        <div style="display:grid;grid-template-columns:1fr 1fr;gap:4px;">
          <button id="noteToggle" style="%s">🚫 全部关闭</button>
          <button id="noteArch" style="%s">📥 存档选中</button>
          <button id="noteDel" style="%s">🗑 删除选中</button>
          <button id="noteArchAll" style="%s">📥 存档全部</button>
          <button id="noteClear" style="grid-column:1 / span 2;%s">🧹 全部清除</button>
        </div>
      </div>
      <div id="noteList" style="margin-top:7px;max-height:36vh;overflow:auto;line-height:1.5;"></div>
      <div id="noteHint" style="margin-top:7px;color:#9aa0a6;font-size:11px;line-height:1.45;"></div>
    </div>
  </div>
</div>''' % (
    BTN % ('#2b2b30', '#4a4a52'),
    BTN % ('#2b4a6b', '#3f6a8c'),
    BTN % ('#2f6b3a', '#3f8a4c'),
    BTN % ('#2b2b30', '#4a4a52'),
    BTN % ('#2b4a6b', '#3f6a8c'),
    BTN % ('#5a3030', '#7a4040'),
    BTN % ('#2b4a6b', '#3f6a8c'),
    BTN % ('#3a3a42', '#55555f'),
)

out = lines[:start] + NEW.split('\n') + lines[end + 1:]
io.open(p, 'w', encoding='utf-8', newline='').write('\n'.join(out))
print('ok, total lines', len(out))
