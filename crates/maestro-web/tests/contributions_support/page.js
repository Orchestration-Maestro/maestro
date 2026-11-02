// Browser observations read by the contribution exports, which run without eval.

// Settle at the next change below `target`.
export function changed(target) {
  return new Promise(resolve => {
    const observer = new MutationObserver(() => { observer.disconnect(); resolve(); });
    observer.observe(target, {subtree: true, childList: true, characterData: true, attributes: true});
  });
}

// Record changes below `target`; each call returns `measure` of the records since the last,
// by default their count.
export function recorder(target, measure = records => records.length) {
  let records = [];
  const observer = new MutationObserver(taken => { records.push(...taken); });
  observer.observe(target, {subtree: true, childList: true, characterData: true, attributes: true,
    attributeOldValue: true, characterDataOldValue: true});
  return () => {
    records.push(...observer.takeRecords());
    const taken = measure(records);
    records = [];
    return taken;
  };
}

// A recorder measure: the UTF-16 code units the changes publish outside elements matching
// `skip`, counting each written attribute or character-data value and the text and attribute
// values of each inserted node.
export function written(skip) {
  return records => {
    // Walking backwards, the value a change wrote is the old value of the next change.
    const next = new Map();
    const latest = (node, key, current) => next.get(node)?.has(key) ? next.get(node).get(key) : current;
    const remember = (node, key, old) => next.set(node, (next.get(node) ?? new Map()).set(key, old));
    let units = 0;
    for (const record of records.toReversed()) {
      const node = record.target;
      const element = node.nodeType === Node.ELEMENT_NODE ? node : node.parentElement;
      if (skip && element?.closest(skip)) continue;
      if (record.type === 'attributes') {
        units += latest(node, record.attributeName, node.getAttribute(record.attributeName))?.length ?? 0;
        remember(node, record.attributeName, record.oldValue);
      } else if (record.type === 'characterData') {
        units += latest(node, '', node.data).length;
        remember(node, '', record.oldValue);
      } else {
        for (const added of record.addedNodes) units += inserted(added, latest);
      }
    }
    return units;
  };
}

// Text and attribute values below an inserted node; inserted text counts as first written.
function inserted(node, latest) {
  if (node.nodeType !== Node.ELEMENT_NODE) return latest(node, '', node.textContent ?? '').length;
  let units = node.textContent.length;
  for (const element of [node, ...node.querySelectorAll('*')]) {
    for (const attribute of element.attributes) units += attribute.value.length;
  }
  return units;
}

// Intervals between the next `count` animation frames, in milliseconds.
export async function frames(count) {
  const times = [await new Promise(requestAnimationFrame)];
  while (times.length <= count) times.push(await new Promise(requestAnimationFrame));
  return times.slice(1).map((time, index) => time - times[index]);
}

// The engine's used heap size, when the browser reports it.
export function heap() {
  return performance.memory?.usedJSHeapSize ?? null;
}

// Raw and gzip-compressed byte counts of each asset.
export async function sizes(urls) {
  const measured = {};
  for (const url of urls) {
    const bytes = await (await fetch(url)).arrayBuffer();
    const gzip = new Blob([bytes]).stream().pipeThrough(new CompressionStream('gzip'));
    measured[url] = {raw: bytes.byteLength, gzip: (await new Response(gzip).arrayBuffer()).byteLength};
  }
  return measured;
}

// Lowercase hexadecimal SHA-256 of a text's UTF-8 bytes.
export async function sha256(text) {
  const digest = await crypto.subtle.digest('SHA-256', new TextEncoder().encode(text));
  return [...new Uint8Array(digest)].map(byte => byte.toString(16).padStart(2, '0')).join('');
}

// Select every child of `node`; returns the selected text.
export function select(node) {
  getSelection().selectAllChildren(node);
  return getSelection().toString();
}

// The selected text and whether `root` contains the selection's anchor.
export function selection(root) {
  const selected = getSelection();
  return [selected.toString(), root.contains(selected.anchorNode)];
}

// Whether the browser's own find locates `text` from the start of the page.
export function find(text) {
  getSelection().removeAllRanges();
  return window.find(text);
}

// Pause the running transition of `property` on `element` at `time` milliseconds; returns
// its duration, or null when none runs.
export function seek(element, property, time) {
  const transition = element.getAnimations().find(a => a.transitionProperty === property);
  if (!transition) return null;
  transition.pause();
  transition.currentTime = time;
  return transition.effect.getTiming().duration;
}

// Finish every running animation of `element`.
export function finish(element) {
  for (const animation of element.getAnimations()) animation.finish();
}
