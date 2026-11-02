// 校验两个 HTML 的脚本语法 + makeArtwork 全模板(TEMPLATES) + 归一化数学
import fs from 'fs';
import { TEMPLATES } from './src/templates.js';

const FILES = ['D:/xiaohuo/index.html', 'D:/xiaohuo/web/index.html'];

function extractScript(html) {
  // 取最后一个 <script> ... </script>（含 type="module" 或经典 <script>）
  const m = [...html.matchAll(/<script[^>]*>([\s\S]*?)<\/script>/g)];
  return m[m.length - 1][1];
}
function stripImport(src) {
  return src.split('\n').filter(l => !/^\s*import\s/.test(l)).join('\n');
}
function extractFn(src, name) {
  const re = new RegExp('function\\s+' + name + '\\s*\\([^)]*\\)\\s*\\{');
  const m = re.exec(src);
  if (!m) return null;
  let i = m.index + m[0].length - 1, depth = 0, start = m.index;
  for (; i < src.length; i++) {
    if (src[i] === '{') depth++;
    else if (src[i] === '}') { depth--; if (depth === 0) { i++; break; } }
  }
  return src.slice(start, i);
}
function extractConstObj(src, name) {
  const re = new RegExp('const ' + name + ' = \\{[\\s\\S]*?\\n  \\};');
  const m = re.exec(src);
  return m ? m[0] : null;
}
function syntaxCheck(src, label) {
  try { new Function(stripImport(src)); console.log('  ✅ 语法OK  ' + label); return true; }
  catch (e) { console.log('  ❌ 语法错误 ' + label + ': ' + e.message); return false; }
}

const PRIM_SPEC = { line: 4, wave: 5, circle: 3, star: 4, scribble: 3, dot: 3, tri: 6, poly: 'even>=4' };
const TYPES = TEMPLATES;

let allOk = true;
for (const f of FILES) {
  console.log('\n=== ' + f.split('/').pop() + ' ===');
  const html = fs.readFileSync(f, 'utf8');
  const script = extractScript(html);
  const ok = syntaxCheck(script, '整体脚本');
  if (!ok) allOk = false;

  // 关键新函数/特性是否到位
  const must = ["case 'think'", 'FireworkFX', 'normalizeArtworkCoords', 'PRIM_SCHEMA', 'FESTIVALS', 'todayFestival', 'maybeFestivalPaint', 'headTilt'];
  for (const k of must) if (!script.includes(k)) { console.log('  ❌ 缺少: ' + k); allOk = false; }
  if (!script.includes("case 'think'")) {} // already checked
  console.log('  关键特性: ' + must.filter(k => script.includes(k)).length + '/' + must.length);

  // 抽取 makeArtwork 做模板校验
  const maSrc = extractFn(script, 'makeArtwork');
  try {
    const makeArtwork = new Function(maSrc + '; return makeArtwork;')();
    let fail = 0;
    for (const t of TYPES) {
      const strokes = makeArtwork(t, 100, 200);
      if (!Array.isArray(strokes)) { console.log('  ❌ ' + t + ' 非数组'); fail++; continue; }
      for (const s of strokes) {
        const spec = PRIM_SPEC[s.prim];
        if (!spec) { console.log('  ❌ ' + t + ' 未知 prim ' + s.prim); fail++; continue; }
        if (!Array.isArray(s.a)) { console.log('  ❌ ' + t + ' a 非数组'); fail++; continue; }
        if (spec === 'even>=4') { if (s.a.length < 4 || s.a.length % 2 !== 0) { console.log('  ❌ ' + t + ' poly 长度异常'); fail++; } }
        else if (s.a.length !== spec) { console.log('  ❌ ' + t + ' ' + s.prim + ' 长度应为 ' + spec + ' 实得 ' + s.a.length); fail++; }
        if (!/^#[0-9a-fA-F]{6}$/.test(s.color)) { console.log('  ❌ ' + t + ' 颜色格式 ' + s.color); fail++; }
        // 坐标有限性（粗略：全部数有限）
        for (const v of s.a) if (typeof v === 'number' && !isFinite(v)) { console.log('  ❌ ' + t + ' 坐标非有限'); fail++; break; }
      }
    }
    console.log(fail === 0 ? '  ✅ makeArtwork 全部模板通过（长度/颜色/坐标）' : '  ❌ makeArtwork 失败 ' + fail + ' 处');
    if (fail) allOk = false;
  } catch (e) { console.log('  ❌ makeArtwork 抽取/执行失败: ' + e.message); allOk = false; }

  // 抽取归一化相关函数做数学校验
  const normSrc = extractFn(script, 'normalizeArtworkCoords');
  const ppSrc = extractFn(script, 'primPos');
  const eiSrc = extractFn(script, 'evenIdx');
  const schemaSrc = extractConstObj(script, 'PRIM_SCHEMA');
  if (normSrc && ppSrc && eiSrc && schemaSrc) {
    try {
      const api = new Function(schemaSrc + '\n' + eiSrc + '\n' + ppSrc + '\n' + normSrc + '\nreturn { normalizeArtworkCoords, PRIM_SCHEMA, primPos };')();
      // 样例：超大坐标 → 应居中到 (100,100) 框内且缩放进 150
      const strokes = [
        { prim: 'circle', a: [1000, 1000, 50], color: '#000000' },
        { prim: 'line', a: [2000, 2000, 3000, 3000], color: '#000000' },
        { prim: 'star', a: [1500, 1500, 80, 5], color: '#000000' },
      ];
      api.normalizeArtworkCoords(strokes, 100, 100, 150);
      // bbox 计算：minX1000 maxX3000 → 中心2000，scale=150/2000=0.075
      const c = strokes[0].a;        // (1000-2000)*0.075+100 = 25
      const r = strokes[0].a[2];      // 50*0.075 = 3.75
      const lc = strokes[1].a;        // line 全部点： (1000-2000)*.075+100=25 ; (3000-2000)*.075+100=175
      const sr = strokes[2].a[2];      // star R 80*0.075 = 6
      const okPt = Math.abs(c[0]-25)<0.01 && Math.abs(c[1]-25)<0.01 && Math.abs(r-3.75)<0.01
                && Math.abs(lc[0]-100)<0.01 && Math.abs(lc[1]-100)<0.01 && Math.abs(lc[2]-175)<0.01 && Math.abs(lc[3]-175)<0.01
                && Math.abs(sr-6)<0.01;
      console.log(okPt ? '  ✅ 归一化数学正确（居中+缩放+半径同步）' : '  ❌ 归一化数学偏差: ' + JSON.stringify(strokes));
      if (!okPt) allOk = false;
    } catch (e) { console.log('  ❌ 归一化校验异常: ' + e.message); allOk = false; }
  } else { console.log('  ❌ 归一化相关函数抽取失败'); allOk = false; }
}
console.log('\n=== 总结果: ' + (allOk ? '全部通过 ✅' : '存在问题 ❌') + ' ===');
process.exit(allOk ? 0 : 1);
