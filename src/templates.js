// TEMPLATES —— 画法单一真相源。
// 所有"小火能画的实物 / 概念"在此登记一次，校验脚本、预览生成器、knownTemplate
// 都从这份清单读取，杜绝各文件各自硬编码导致的 18/19 漂移。
//
// 两套机制清晰分工：
//   QD_CLASSES       —— Quick Draw 真人手绘数据类目（运行时由 dist/qd/manifest.json 提供数据）。
//   KNOWN_TEMPLATE_KEYS —— 代码内置模板（knownTemplate 命中，优先级高于 QD / 图标 / LLM）。
//
// 校验函数 validateQdManifest 在运行时比对 manifest 与 QD_CLASSES，
// 任何缺失 / 多余都会 console.warn，第一时间暴露漂移。

// QD 类目（与 dist/qd/manifest.json 的 categories 保持一致，这里是其静态真相源）
export const QD_CLASSES = [
  'apple', 'bird', 'book', 'car', 'cat', 'chair', 'cup', 'dog', 'duck',
  'fish', 'flower', 'house', 'mountain', 'star', 'sun', 'tree',
];

// 代码侧已知模板（knownTemplate 支持），优先级高于 QD / 图标 / LLM
export const KNOWN_TEMPLATE_KEYS = [
  'fireworks', 'cow', 'sheep', 'cat', 'dog', 'rabbit', 'fish', 'house',
  'heart', 'rocket', 'moon', 'flag', 'christmas',
];

// 运行时校验：manifest.zhToKey 的值集合必须与 QD_CLASSES 完全一致
export function validateQdManifest(manifest) {
  if (!manifest || !manifest.zhToKey) {
    return { ok: false, missing: [...QD_CLASSES], extra: [] };
  }
  const actual = new Set(Object.values(manifest.zhToKey));
  const expected = new Set(QD_CLASSES);
  const missing = QD_CLASSES.filter(c => !actual.has(c));
  const extra = [...actual].filter(c => !expected.has(c));
  return { ok: missing.length === 0 && extra.length === 0, missing, extra };
}
