(() => {
  const block = new Set(['P', 'H1', 'H2', 'H3', 'H4', 'H5', 'H6', 'UL', 'OL', 'LI', 'BLOCKQUOTE', 'TABLE', 'THEAD', 'TBODY', 'TR', 'TH', 'TD', 'HR', 'DIV', 'PRE']);
  const roles = new Map([
    ['my-4', 'math-block'], ['maestro-math-block', 'math-block'],
    ['overflow-x-auto my-2 border border-border rounded', 'scroll'], ['maestro-table-scroll', 'scroll'],
    ['my-4 text-red-500 font-mono', 'math-fallback'], ['maestro-math-fallback', 'math-fallback'],
    ['text-red-500 font-mono', 'math-fallback'],
  ]);
  // Class names the matching math renderer spells differently from the recorded reference.
  const renames = new Map([['base', 'katex-base'], ['strut', 'katex-strut'], ['sizing', 'katex-sizing']]);
  const renamed = (cls) => cls.split(' ').map((t) => renames.get(t) ?? t).join(' ');
  const decode = (base64) => new TextDecoder().decode(Uint8Array.from(atob(base64), (c) => c.charCodeAt(0)));
  // The reference spells style declarations without spaces, prefixes tooltips with the
  // error class and fixes the error color; the matching renderer resolves the selected role.
  const attribute = (a, reference) => {
    let v = a.name === 'style' ? a.value.replace(/([:;])\s+/g, '$1') : a.value;
    if (reference && a.name === 'title') v = v.replace(/^ParseError: /, '');
    if (reference) v = v.replaceAll('#cc0000', 'var(--maestro-color-error)');
    return v;
  };
  const isBlock = (node) => node && node.nodeType === 1 && block.has(node.tagName);
  const first = (cls) => (cls || '').split(' ')[0];

  // Neutral tree of either the reference markup or the native nodes.
  function project(node, inMath, reference) {
    if (node.nodeType === 3) return node.data;
    if (node.nodeType !== 1) return null;
    const tag = node.tagName.toLowerCase();
    const cls = node.getAttribute('class');
    if (tag === 'code-block') {
      return ['code-block', node.getAttribute('language'), decode(node.getAttribute('code'))];
    }
    if (cls === 'maestro-code') {
      return ['code-block', node.querySelector('.maestro-code-label').textContent, node.querySelector('code').textContent];
    }
    if (cls === 'maestro-code-wrap' || (tag === 'div' && cls === 'mt-2' && node.firstElementChild?.tagName === 'CODE-BLOCK')) {
      return project(node.firstElementChild, inMath, reference);
    }
    const math = inMath || ['katex', 'katex-error', 'katex-display'].includes(first(cls));
    const attrs = {};
    for (const a of node.attributes) if (a.name !== 'class') attrs[a.name] = attribute(a, reference);
    if (math && cls) attrs.class = reference ? renamed(cls) : cls;
    else if (roles.has(cls)) attrs.role = roles.get(cls);
    const kids = [];
    const nodes = [...node.childNodes];
    nodes.forEach((child, i) => {
      const spacer = child.nodeType === 3 && /^\n+$/.test(child.data);
      if (spacer && !math && (isBlock(nodes[i - 1]) || isBlock(nodes[i + 1]) || i === 0 || i === nodes.length - 1)) return;
      const p = project(child, math, reference);
      if (p === null) return;
      if (typeof p === 'string' && typeof kids[kids.length - 1] === 'string') kids[kids.length - 1] += p;
      else kids.push(p);
    });
    return { t: tag, a: Object.fromEntries(Object.entries(attrs).sort()), c: kids };
  }
  function children(nodes, reference) {
    const holder = document.createElement('div');
    for (const c of nodes) holder.appendChild(c.cloneNode(true));
    return project(holder, false, reference).c;
  }
  function firstDifference(a, b, path) {
    if (typeof a === 'string' || typeof b === 'string' || a === null || b === null || Array.isArray(a) || Array.isArray(b)) {
      return JSON.stringify(a) === JSON.stringify(b) ? '' : `${path}: ${JSON.stringify(a)} != ${JSON.stringify(b)}`;
    }
    if (a.t !== b.t) return `${path}: <${a.t}> != <${b.t}>`;
    if (JSON.stringify(a.a) !== JSON.stringify(b.a)) return `${path}<${a.t}>: attrs ${JSON.stringify(a.a)} != ${JSON.stringify(b.a)}`;
    if (a.c.length !== b.c.length) return `${path}<${a.t}>: ${a.c.length} != ${b.c.length} children: ${JSON.stringify(a.c).slice(0, 400)} != ${JSON.stringify(b.c).slice(0, 400)}`;
    for (let i = 0; i < a.c.length; i++) {
      const d = firstDifference(a.c[i], b.c[i], `${path}/${a.t}[${i}]`);
      if (d) return d;
    }
    return '';
  }
  const fragment = (html) => {
    const t = document.createElement('template');
    t.innerHTML = html;
    return t.content;
  };
  // Structural tree of the native nodes, with formulas kept as their markup.
  function tree(nodes) {
    const out = [];
    const push = (n) => {
      if (n.kind === 'text' && out.at(-1)?.kind === 'text') out.at(-1).text += n.text;
      else out.push(n);
    };
    for (const node of nodes) {
      if (node.nodeType === 3) {
        if (!/^\n+$/.test(node.data) || !(isBlock(node.previousSibling) || isBlock(node.nextSibling) || !node.previousSibling || !node.nextSibling)) push({ kind: 'text', text: node.data });
        continue;
      }
      if (node.nodeType !== 1) continue;
      const tag = node.tagName.toLowerCase();
      const cls = first(node.getAttribute('class'));
      const kids = () => tree(node.childNodes);
      if (cls === 'maestro-math-block') push({ kind: 'math-block', html: node.firstElementChild.outerHTML });
      else if (cls === 'katex' || cls === 'katex-error') push({ kind: 'math-inline', html: node.outerHTML });
      else if (cls === 'maestro-code-wrap') push({ kind: 'code-block', children: [{ kind: 'text', text: node.querySelector('code').textContent.replace(/\n$/, '') }] });
      else if (cls === 'maestro-table-scroll') out.push(...kids());
      else if (tag === 'table') {
        const rows = [...node.querySelectorAll('tr')];
        push({ kind: 'table', children: rows.map((tr) => ({ kind: tr.parentElement.tagName === 'THEAD' ? 'head' : 'row', children: [...tr.children].map((cell) => ({ kind: 'cell', children: tree(cell.childNodes) })) })) });
      } else {
        const kind = { p: 'paragraph', strong: 'strong', em: 'em', a: 'link', blockquote: 'quote', ul: 'list', ol: 'list', li: 'item' }[tag];
        if (tag === 'code') push({ kind: 'code', text: node.textContent });
        else if (kind) push({ kind, children: kids() });
        else out.push(...kids());
      }
    }
    return out;
  }
  const mathProjection = (html, reference) => JSON.stringify(project(fragment(html).firstElementChild, true, reference));
  function treeDifference(actual, expected, path) {
    if (actual.kind !== expected.kind) return `${path}: ${actual.kind} != ${expected.kind}`;
    if (expected.kind === 'text' || expected.kind === 'code') {
      return actual.text === expected.text ? '' : `${path}: ${JSON.stringify(actual.text)} != ${JSON.stringify(expected.text)}`;
    }
    if (expected.kind === 'math-inline' || expected.kind === 'math-block') {
      const found = globalThis.maestro.formulas.find((f) => f.tex === expected.text && f.display === (expected.kind === 'math-block'));
      if (!found) return `${path}: no recorded formula ${JSON.stringify(expected.text)}`;
      return mathProjection(actual.html, false) === mathProjection(found.html, true) ? '' : `${path}: formula ${JSON.stringify(expected.text)} differs`;
    }
    if (actual.children.length !== expected.children.length) return `${path}<${expected.kind}>: ${actual.children.length} != ${expected.children.length} children: ${JSON.stringify(actual.children).slice(0, 300)}`;
    for (let i = 0; i < expected.children.length; i++) {
      const d = treeDifference(actual.children[i], expected.children[i], `${path}/${expected.kind}[${i}]`);
      if (d) return d;
    }
    return '';
  }
  // Names of the painted roles that do not resolve to the selected pack's colors.
  function paintFailures(c, thinking) {
    const rgb = (hex) => {
      const n = parseInt(hex.slice(1), 16);
      return `rgb(${n >> 16}, ${(n >> 8) & 255}, ${n & 255})`;
    };
    const style = (el, prop) => getComputedStyle(el)[prop];
    const q = (s) => document.querySelector(`.maestro-markdown ${s}`);
    const root = document.querySelector('.maestro-markdown');
    return Object.entries({
      text: [style(root, 'color'), thinking ? c.muted : c.text],
      link: [style(q('a'), 'color'), c.accent],
      code: [style(q('code:not(.hljs)'), 'backgroundColor'), c.panel],
      quote: [style(q('blockquote'), 'borderLeftColor'), c.border],
      header: [style(q('th'), 'backgroundColor'), c.panel],
      error: [style(q('.katex-error'), 'color'), c.error],
      label: [style(q('.maestro-code-label'), 'color'), c.muted],
      listing: [style(q('.maestro-code'), 'borderTopColor'), c.border],
      scroller: [style(q('.maestro-table-scroll'), 'borderTopColor'), c.border],
    }).filter(([, [actual, role]]) => actual !== rgb(role)).map(([name, [actual, role]]) => `${name}: ${actual} != ${rgb(role)}`);
  }
  // Compare a mounted formula at the recorder's typography and containing width.
  async function measureMath() {
    const container = document.querySelector('.maestro-markdown');
    container.style.cssText += ';width:800px;font-size:16px;font-family:"Times New Roman";line-height:normal';
    await document.fonts.ready;
    await Promise.all(['KaTeX_Main', 'KaTeX_Math', 'KaTeX_Size1', 'KaTeX_Size2'].map(f => document.fonts.load(`16px ${f}`)));
    await document.fonts.ready;
    const expected = JSON.parse(globalThis.mathExpected), branch = globalThis.mathBranch;
    if (container.querySelector('img,script,iframe')) return 'executable element in formula';
    if (branch === 'throw') {
      const fallback = container.querySelector('.maestro-math-fallback');
      if (!fallback || fallback.textContent !== globalThis.mathSource) return 'delimiter fallback text';
      return globalThis.maestro.diff(container, expected);
    }
    const root = container.querySelector('.katex,.katex-error');
    if (!root) return 'missing formula';
    const error = root.classList.contains('katex-error');
    if (error !== (branch === 'error')) return 'wrong error branch';
    if (error && (!root.title || root.style.color !== 'var(--maestro-color-error)')) return 'error tooltip or selected role';
    const origin = root.getBoundingClientRect();
    const bounds = r => ({left:r.x-origin.x, top:r.y-origin.y, width:r.width, height:r.height});
    const glyphs = [], walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT);
    while (walker.nextNode()) {
      const text = walker.currentNode;
      if (text.parentElement.closest('svg') || text.textContent === '\u200b') continue;
      const range = document.createRange(); range.selectNodeContents(text);
      const rect = range.getBoundingClientRect();
      if (rect.width || rect.height) glyphs.push({text:text.textContent, ...bounds(rect)});
    }
    const actual = {text:root.textContent, width:origin.width, height:origin.height, glyphs,
      svg:[...root.querySelectorAll('svg')].map(s => bounds(s.getBoundingClientRect()))};
    // Preserve every recorded field, ordering and repeated glyph occurrence.
    const compare = (a, b, path) => {
      if (typeof b === 'number') return Math.abs(a-b) <= 1/64 ? '' : `${path}: ${a} != ${b}`;
      if (b === null || typeof b !== 'object') return a === b ? '' : `${path}: text differs`;
      if (JSON.stringify(Object.keys(a)) !== JSON.stringify(Object.keys(b))) return `${path}: fields differ`;
      for (const key of Object.keys(b)) { const d = compare(a[key], b[key], `${path}/${key}`); if (d) return d; }
      return '';
    };
    return compare(actual, expected, 'formula');
  }
  globalThis.maestro = {
    measureMath,
    paintFailures,
    formulas: [],
    // First difference between the native structure and a recorded tree, or "".
    treeDiff: (root, expected) => {
      const actual = tree(root.childNodes);
      if (actual.length !== expected.length) return `top level: ${actual.length} != ${expected.length}: ${JSON.stringify(actual).slice(0, 400)}`;
      for (let i = 0; i < expected.length; i++) {
        const d = treeDifference(actual[i], expected[i], `[${i}]`);
        if (d) return d;
      }
      return '';
    },
    root: () => document.querySelector('.maestro-markdown'),
    // First difference between the native children of `root` and reference markup, or "".
    diff: (root, html) => firstDifference(
      { t: 'div', a: {}, c: children(root.childNodes, false) },
      { t: 'div', a: {}, c: children(fragment(html).childNodes, true) },
      '',
    ),
  };
})();
