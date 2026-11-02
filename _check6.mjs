import fs from 'fs';
import vm from 'vm';

function extractLargestScript(html) {
  const re = /<script[^>]*>([\s\S]*?)<\/script>/g;
  let m, best = '';
  while ((m = re.exec(html))) { if (m[1].length > best.length) best = m[1]; }
  return best;
}
function readScript(file) {
  let s = extractLargestScript(fs.readFileSync(file, 'utf8'));
  s = s.replace(/^\s*import\s+[^;]*;?\s*$/mg, '');
  return s;
}
function grab(code, re, name) {
  const m = code.match(re);
  if (!m) { console.log('  FAIL 抽取失败:', name); return null; }
  return m[0];
}

function makeProxy() {
  const fn = function () { return p; };
  const p = new Proxy(fn, {
    get(t, k) {
      if (k === 'value') return 'cat';
      if (k === 'textContent') return '';
      if (k === 'length') return 0;
      if (k === Symbol.toPrimitive) return () => '';
      return p;
    },
    set() { return true; }, apply() { return p; },
  });
  return p;
}
function buildSandbox() {
  const document = makeProxy();
  const window = {};
  window.addEventListener = () => {}; window.removeEventListener = () => {};
  window.innerWidth = 1280; window.innerHeight = 720; window.devicePixelRatio = 1; window.location = { href: '' };
  return {
    window, document,
    console, performance: { now: () => Date.now() },
    requestAnimationFrame: () => 0,
    setTimeout: () => 0, clearTimeout: () => {}, setInterval: () => 0, clearInterval: () => {},
    AbortController: class { abort() {} signal = {} },
    MutationObserver: class { observe() {} disconnect() {} },
    ResizeObserver: class { observe() {} disconnect() {} },
    IntersectionObserver: class { observe() {} disconnect() {} },
    getComputedStyle: () => ({ getPropertyValue: () => '' }),
    CustomEvent: class {}, Event: class {}, Image: class {},
    fetch: () => Promise.reject(new Error('offline')), invoke: () => Promise.reject(new Error('offline')),
    Math, JSON, Date, Object, Array, String, Number, Boolean, RegExp, Promise, Symbol, parseInt, parseFloat, isFinite,
  };
}

let pass = 0, fail = 0;
function check(name, cond, extra) {
  if (cond) { pass++; console.log('  PASS', name); }
  else { fail++; console.log('  FAIL', name, extra !== undefined ? JSON.stringify(extra) : ''); }
}

for (const file of ['D:/xiaohuo/index.html', 'D:/xiaohuo/web/index.html']) {
  console.log('==', file.split('/').pop(), '==');
  const code = readScript(file);
  // 1) 整脚本 vm 运行：验证顶层执行不崩（含我们的改动）
  const box = buildSandbox(); box.globalThis = box; box.window.document = box.document;
  try { vm.createContext(box); vm.runInContext(code, box, { filename: file }); console.log('  PASS 脚本顶层执行无异常'); }
  catch (e) { console.log('  FAIL 脚本顶层执行抛错:', e.message); fail++; continue; }

  // 2) 单独抽取 ZH_EN + toIconQueries + pickIcon + PREF_ICON_SETS + ICON_EXCLUDE 做逻辑单测
  const zh = grab(code, /const ZH_EN = \{[\s\S]*?\n  \};/, 'ZH_EN');
  const pref = grab(code, /const PREF_ICON_SETS = \[[^\]]*\]/, 'PREF_ICON_SETS');
  const exc = grab(code, /const ICON_EXCLUDE = \[[^\]]*\]/, 'ICON_EXCLUDE');
  const toQ = grab(code, /function toIconQueries\(v\) \{[\s\S]*?\n  \}/, 'toIconQueries');
  const pick = grab(code, /function pickIcon\(icons\) \{[\s\S]*?\n  \}/, 'pickIcon');
  if (!zh || !pref || !exc || !toQ || !pick) { fail++; continue; }
  let api;
  try {
    api = new Function(zh + '\n' + pref + '\n' + exc + '\n' + toQ + '\n' + pick + '\n return { toIconQueries, pickIcon };')();
  } catch (e) { console.log('  FAIL 抽取块执行抛错:', e.message); fail++; continue; }

  const q = api.toIconQueries;
  check('toIconQueries("画只小猫")→含cat', JSON.stringify(q('画只小猫')).includes('cat'), q('画只小猫'));
  check('toIconQueries("机器人")→robot', JSON.stringify(q('机器人')).includes('robot'), q('机器人'));
  check('toIconQueries("鼠标")→cursor(最长优先)', JSON.stringify(q('鼠标')).includes('cursor'), q('鼠标'));
  check('toIconQueries("一只红色的小鸟")→bird(去修饰)', JSON.stringify(q('一只红色的小鸟')).includes('bird'), q('一只红色的小鸟'));
  check('toIconQueries("啊吧次大")→[](中文不认识交LLM)', Array.isArray(q('啊吧次大')) && q('啊吧次大').length === 0, q('啊吧次大'));
  check('toIconQueries("hello")→[hello]', JSON.stringify(q('hello')) === '["hello"]', q('hello'));
  check('toIconQueries("圣诞树")→christmas-tree', JSON.stringify(q('圣诞树')).includes('christmas-tree'), q('圣诞树'));
  const p = api.pickIcon;
  check('pickIcon 优选线稿集', (p(['noto:foo','mdi:cat','lucide:dog']) || '').startsWith('mdi') || (p(['noto:foo','mdi:cat','lucide:dog'])||'').startsWith('lucide'), p(['noto:foo','mdi:cat','lucide:dog']));
  check('pickIcon 排除logo选fluent', p(['logos:github','fluent:car']) === 'fluent:car', p(['logos:github','fluent:car']));
  check('pickIcon([])→null', p([]) === null);
}

console.log('\n结果: ' + pass + ' 通过, ' + fail + ' 失败');
process.exit(fail ? 1 : 0);
