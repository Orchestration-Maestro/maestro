//! Browser-native screenshot decoding and pixel classification.

/// Install the page's decoded-screenshot store and its pixel predicates once per document.
///
/// Screenshots stay in the page as decoded image data, so only counts cross the protocol.
/// Pixel positions are CSS coordinates of each device pixel's centre. `count` applies the report
/// observer's role-pixel rule: an opaque pixel whose RGB channels are each within `tolerance`
/// (default exact) of the role's. `dark`
/// counts opaque pixels with no channel above `limit`. `extent`
/// returns the box of the device-pixel centres where two captures differ inside `rect`, or null.
pub(super) const PAINT: &str = r"(() => {
  if (globalThis.__maestroPaint) return;
  const shots = new Map();
  const inside = (x, y, [l, t, r, b]) => x > l && x < r && y > t && y < b;
  const image = label => {
    const shot = shots.get(label);
    if (!shot) throw Error(`missing screenshot ${label}`);
    return shot;
  };
  const scan = (label, visit) => {
    const {data, width, height} = image(label), d = devicePixelRatio;
    for (let y = 0; y < height; y++) for (let x = 0; x < width; x++) visit((x + .5) / d, (y + .5) / d, data, (y * width + x) * 4);
  };
  const element = selector => {
    const node = typeof selector === 'string' ? document.querySelector(selector) : selector;
    if (!node) throw Error(`paint element missing ${selector}`);
    return node;
  };
  let saved = null;
  const restyle = (nodes, styles) => nodes.forEach((node, i) => styles[i] === null ? node.removeAttribute('style') : node.setAttribute('style', styles[i]));
  globalThis.__maestroPaint = {
    store: (label, shot) => { shots.set(label, shot); return null; },
    element,
    box: selector => { const r = element(selector).getBoundingClientRect(); return [r.left, r.top, r.right, r.bottom]; },
    rgb: value => value.match(/[a-f0-9]{2}/gi).slice(0, 3).map(hex => parseInt(hex, 16)),
    colour: value => typeof value === 'string' ? __maestroPaint.rgb(JSON.parse(globalThis.colors)[value]) : value,
    count: (label, rect, colour, tolerance = 0) => {
      let n = 0;
      scan(label, (x, y, data, i) => { if (inside(x, y, rect) && colour.every((v, k) => Math.abs(data[i + k] - v) <= tolerance) && data[i + 3] === 255) n++; });
      return n;
    },
    differ: (first, second, rect, outside) => {
      const other = image(second).data;
      let n = 0;
      scan(first, (x, y, data, i) => {
        if (rect && inside(x, y, rect) === Boolean(outside)) return;
        for (let k = 0; k < 4; k++) if (data[i + k] !== other[i + k]) { n++; return; }
      });
      return n;
    },
    dark: (label, rect, limit) => {
      let n = 0;
      scan(label, (x, y, data, i) => { if (inside(x, y, rect) && data[i] <= limit && data[i + 1] <= limit && data[i + 2] <= limit && data[i + 3] === 255) n++; });
      return n;
    },
    extent: (first, second, rect) => {
      const other = image(second).data;
      let box = null;
      scan(first, (x, y, data, i) => {
        if (!inside(x, y, rect)) return;
        for (let k = 0; k < 4; k++) if (data[i + k] !== other[i + k]) {
          box = box ? [Math.min(box[0], x), Math.min(box[1], y), Math.max(box[2], x), Math.max(box[3], y)] : [x, y, x, y];
          return;
        }
      });
      return box;
    },
    pixel: (label, [x, y]) => {
      const {data, width} = image(label), d = devicePixelRatio, i = (Math.floor(y * d) * width + Math.floor(x * d)) * 4;
      return [...data.subarray(i, i + 4)];
    },
    mutate: (nodes, change) => { saved = [nodes, nodes.map(node => node.getAttribute('style'))]; nodes.forEach(change); return null; },
    restore: () => { restyle(...saved); saved = null; return null; },
    viewport: (label, {left, top, scale, viewport, clip, canvas, holes}) => {
      const counts = {red: 0, green: 0, magenta: 0, escaped: 0, holes: 0};
      const within = (p, [l, t, r, b]) => p[0] > l + .7 && p[0] < r - .7 && p[1] > t + .7 && p[1] < b - .7;
      const beyond = (p, [l, t, r, b]) => p[0] < l - .7 || p[0] > r + .7 || p[1] < t - .7 || p[1] > b + .7;
      scan(label, (x, y, data, i) => {
        const [red, green, blue] = data.subarray(i, i + 3), p = [(x - left) / scale[0], (y - top) / scale[1]];
        const isRed = red === 255 && green === 0 && blue === 0, isGreen = red === 0 && green === 255 && blue === 0, isMagenta = red === 255 && green === 0 && blue === 255;
        if (isRed) counts.red++;
        if (isGreen) counts.green++;
        if (isMagenta) counts.magenta++;
        if (isRed && (beyond(p, viewport) || beyond(p, clip) || beyond(p, canvas))) counts.escaped++;
        if (isMagenta && (beyond(p, clip) || beyond(p, canvas))) counts.escaped++;
        if ((isRed || isMagenta) && holes.some(hole => within(p, hole))) counts.holes++;
      });
      return counts;
    },
  };
})()";

/// Decode a real browser screenshot into its image data using the browser's image decoder.
pub(super) const DECODE: &str = r"source => new Promise((resolve, reject) => {
  const image = new Image();
  image.onload = () => {
    const canvas = document.createElement('canvas');
    canvas.width = image.width; canvas.height = image.height;
    const context = canvas.getContext('2d');
    context.drawImage(image, 0, 0);
    resolve(context.getImageData(0, 0, image.width, image.height));
  };
  image.onerror = () => reject(Error('the screenshot does not decode'));
  image.src = source;
})";
