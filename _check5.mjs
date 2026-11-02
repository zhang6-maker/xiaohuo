import fs from 'fs';
import { TEMPLATES } from './src/templates.js';

const files = ['D:/xiaohuo/index.html', 'D:/xiaohuo/web/index.html'];

function extractMainScript(html) {
  const re = /<script[^>]*>([\s\S]*?)<\/script>/g;
  let m, best = '';
  while ((m = re.exec(html))) { if (m[1].length > best.length) best = m[1]; }
  return best;
}

function stripModuleSyntax(js) {
  // remove top-level import/export lines so we can syntax-check as a plain script
  return js.split('\n').filter(l => !/^\s*import\s/.test(l) && !/^\s*export\s/.test(l)).join('\n');
}

function extractMakeArtwork(js) {
  const i = js.indexOf('function makeArtwork');
  if (i < 0) return null;
  let j = js.indexOf('{', i);
  let depth = 0, k = j;
  for (; k < js.length; k++) {
    if (js[k] === '{') depth++;
    else if (js[k] === '}') { depth--; if (depth === 0) { k++; break; } }
  }
  return js.slice(i, k);
}

const PRIM_LEN = { line: 4, wave: 5, circle: 3, star: 4, scribble: 3, dot: 3, tri: 6, poly: 'even>=4' };
function validateArtwork(strokes) {
  if (!Array.isArray(strokes)) return 'not array';
  for (const s of strokes) {
    if (!s || !s.prim || !Array.isArray(s.a)) return 'bad stroke ' + JSON.stringify(s);
    const need = PRIM_LEN[s.prim];
    if (need === undefined) return 'unknown prim ' + s.prim;
    if (need === 'even>=4') {
      if (s.a.length < 4 || s.a.length % 2 !== 0) return 'poly bad len ' + s.a.length;
    } else if (s.a.length !== need) return s.prim + ' len ' + s.a.length + ' != ' + need;
    for (const v of s.a) if (typeof v !== 'number' || !isFinite(v)) return 'non-finite coord ' + JSON.stringify(s.a);
    if (typeof s.color !== 'string' || !/^#[0-9a-fA-F]{6}$/.test(s.color)) return 'bad color ' + s.color;
  }
  return null;
}

let allOk = true;
for (const f of files) {
  const html = fs.readFileSync(f, 'utf8');
  const jsRaw = extractMainScript(html);
  const js = stripModuleSyntax(jsRaw);
  let syntaxOk = true, syntaxErr = '';
  try { new Function(js); } catch (e) { syntaxOk = false; syntaxErr = e.message; }
  const ma = extractMakeArtwork(js);
  let tplOk = true, tplErr = '';
  if (!ma) { tplOk = false; tplErr = 'makeArtwork not found'; }
  else {
    try {
      const makeArtwork = new Function(ma + '\nreturn makeArtwork;')();
      for (const t of TEMPLATES) {
        const s = makeArtwork(t, 300, 300);
        if (!Array.isArray(s)) { tplOk = false; tplErr = t + ' returned ' + JSON.stringify(s); break; }
        const e = validateArtwork(s); if (e) { tplOk = false; tplErr = t + ': ' + e; break; }
      }
    } catch (e) { tplOk = false; tplErr = e.message; }
  }
  const ok = syntaxOk && tplOk;
  if (!ok) allOk = false;
  console.log(`${ok ? 'PASS' : 'FAIL'}  ${f.split('/').pop()}  syntax=${syntaxOk}${syntaxOk ? '' : ' (' + syntaxErr + ')'}  templates=${tplOk}${tplOk ? '' : ' (' + tplErr + ')'}`);
}
console.log(allOk ? '\nALL OK' : '\nHAS FAILURES');
