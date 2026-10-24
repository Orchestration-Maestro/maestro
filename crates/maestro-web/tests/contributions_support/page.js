// Browser observations read by the contribution exports, which run without eval.

// Settle at the next change below `target`.
export function changed(target) {
  return new Promise(resolve => {
    const observer = new MutationObserver(() => { observer.disconnect(); resolve(); });
    observer.observe(target, {subtree: true, childList: true, characterData: true, attributes: true});
  });
}

// Count mutation records below `target`; each call returns the records since the last.
export function recorder(target) {
  let count = 0;
  const observer = new MutationObserver(records => { count += records.length; });
  observer.observe(target, {subtree: true, childList: true, characterData: true, attributes: true});
  return () => {
    count += observer.takeRecords().length;
    const taken = count;
    count = 0;
    return taken;
  };
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
