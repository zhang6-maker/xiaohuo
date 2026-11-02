// 从 index.html 抽取真实的 makeArtwork，渲染 18 个模板到一份预览 HTML（自动与代码同步）
import fs from 'fs';
import { TEMPLATES } from './src/templates.js';

const html = fs.readFileSync('D:/xiaohuo/index.html', 'utf8');
const m = [...html.matchAll(/<script[^>]*>([\s\S]*?)<\/script>/g)];
const script = m[m.length - 1][1];

function extractFn(src, name) {
  const re = new RegExp('function\\s+' + name + '\\s*\\([^)]*\\)\\s*\\{');
  const mm = re.exec(src); if (!mm) return null;
  let i = mm.index + mm[0].length - 1, depth = 0, start = mm.index;
  for (; i < src.length; i++) { if (src[i] === '{') depth++; else if (src[i] === '}') { depth--; if (depth === 0) { i++; break; } } }
  return src.slice(start, i);
}
const maSrc = extractFn(script, 'makeArtwork');

const TYPES = TEMPLATES;
const NAMES = { tree:'树', sun:'太阳', flower:'花', star:'星星', cloud:'云', cow:'牛', sheep:'羊', fireworks:'烟花(粒子)', cat:'猫', dog:'狗', rabbit:'兔', fish:'鱼', house:'房子', heart:'爱心', rocket:'火箭', flag:'国旗', moon:'月亮', christmas:'圣诞树', random:'随机' };

const htmlOut = `<!DOCTYPE html><html lang="zh-CN"><head><meta charset="UTF-8">
<title>小火 · 绘画模板预览 (v0.1.6)</title>
<style>body{background:#f5f6f8;font-family:system-ui,'PingFang SC','Microsoft YaHei',sans-serif;margin:0;padding:24px;color:#2b2b2f}
h1{font-size:20px} .sub{color:#888;margin:6px 0 18px;font-size:13px;line-height:1.6}
.grid{display:grid;grid-template-columns:repeat(6,1fr);gap:14px}
.cell{background:#fff;border:1px solid #e3e5e9;border-radius:12px;padding:8px;text-align:center}
.cell canvas{background:linear-gradient(180deg,#fafbff,#eef1f6);border-radius:8px}
.cap{font-size:13px;margin-top:6px;color:#555}
.note{margin-top:22px;background:#fff;border:1px solid #e3e5e9;border-radius:12px;padding:16px;font-size:13px;line-height:1.8}
.note b{color:#c0392b}</style></head><body>
<h1>小火 · 绘画模板预览</h1>
<div class="sub">以下 19 个模板由 index.html 的 makeArtwork 实时抽取渲染，与桌面/网页版小火画出来的一致。新增：国旗、月亮、圣诞树。</div>
<div class="grid" id="g"></div>
<div class="note">
  <b>本轮改动（针对反馈）：</b><br>
  1) <b>烟花修复</b>：画烟花时小火身体<b>随爆炸/升空颜色变化</b>（之前不变色）；整场时长 6.5s→11s，发射更密（间隔 850→600ms、同时火箭 5→7 发）、每发火星 26~44→42~68 颗、寿命更长 → 不再稀疏/一闪而过。<br>
  2) <b>圣诞树模板</b>：新增分层松塔+顶星+彩球的圣诞树（区别于普通树）；「圣诞/圣诞树」与节日日历 12-25 都映射到它，不再画成普通树。<br>
  3) <b>画前联网检索</b>：不认识的词，小火会先联网（维基百科）查一下这东西长啥样，把真实形状描述喂给智谱当空间参考，再动笔；查不到/超时也不影响照常画。
</div>
<script>
${maSrc}
const TYPES=${JSON.stringify(TYPES)}; const NAMES=${JSON.stringify(NAMES)};
const g=document.getElementById('g');
for(const t of TYPES){
  const cell=document.createElement('div'); cell.className='cell';
  const cv=document.createElement('canvas'); cv.width=200; cv.height=200;
  const ctx=cv.getContext('2d');
  const cx=100, cy=160;
  const strokes=makeArtwork(t,cx,cy);
  ctx.lineWidth=3; ctx.lineCap='round'; ctx.lineJoin='round';
  for(const st of strokes){
    ctx.strokeStyle=st.color; ctx.fillStyle=st.color; const a=st.a;
    if(st.prim==='line'){ctx.beginPath();ctx.moveTo(a[0],a[1]);ctx.lineTo(a[2],a[3]);ctx.stroke();}
    else if(st.prim==='circle'){ctx.beginPath();ctx.arc(a[0],a[1],a[2],0,Math.PI*2);ctx.stroke();}
    else if(st.prim==='wave'){const n=40;ctx.beginPath();ctx.moveTo(a[0],a[1]);for(let i=1;i<=n;i++){const tt=i/n;ctx.lineTo(a[0]+(a[2]-a[0])*tt,a[1]+(a[3]-a[1])*tt+Math.sin(tt*Math.PI*a[4])*8);}ctx.stroke();}
    else if(st.prim==='star'){const R=a[2],sp=a[3],n=sp*2;ctx.beginPath();for(let i=0;i<=n;i++){const ang=-Math.PI/2+i*Math.PI/sp;const rr=i%2?R*0.45:R;const x=a[0]+Math.cos(ang)*rr,y=a[1]+Math.sin(ang)*rr;i?ctx.lineTo(x,y):ctx.moveTo(x,y);}ctx.stroke();}
    else if(st.prim==='scribble'){let s=a[2];const rnd=()=>{s=(s*9301+49297)%233280;return s/233280;};ctx.beginPath();ctx.moveTo(a[0],a[1]);for(let i=1;i<=16;i++)ctx.lineTo(a[0]+(rnd()-0.5)*34,a[1]+(rnd()-0.5)*34);ctx.stroke();}
    else if(st.prim==='dot'){ctx.beginPath();ctx.arc(a[0],a[1],Math.max(a[2],3.5),0,Math.PI*2);ctx.fill();}
    else if(st.prim==='tri'){ctx.beginPath();ctx.moveTo(a[0],a[1]);ctx.lineTo(a[2],a[3]);ctx.lineTo(a[4],a[5]);ctx.closePath();ctx.fill();ctx.stroke();}
    else if(st.prim==='poly'){ctx.beginPath();ctx.moveTo(a[0],a[1]);for(let i=2;i<a.length;i+=2)ctx.lineTo(a[i],a[i+1]);ctx.stroke();}
  }
  const cap=document.createElement('div'); cap.className='cap'; cap.textContent=(NAMES[t]||t);
  cell.appendChild(cv); cell.appendChild(cap); g.appendChild(cell);
}
</script></body></html>`;

fs.mkdirSync('D:/Deliverables/2026-10-28/xiaohuo-paint', { recursive: true });
fs.writeFileSync('D:/Deliverables/2026-10-28/xiaohuo-paint/templates-preview.html', htmlOut);
console.log('已生成预览 HTML（含全部模板，自动同步自 index.html 的 makeArtwork）');
