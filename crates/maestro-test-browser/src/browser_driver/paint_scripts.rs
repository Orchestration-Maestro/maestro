//! Browser-native screenshot decoding and pixel classification for reading paint scenes.

/// Install the page's decoded-screenshot store and its pixel predicates once per document.
///
/// Screenshots stay in the page as decoded image data, so only counts cross the protocol.
/// Pixel positions are CSS coordinates of each device pixel's centre.
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
  globalThis.__maestroPaint = {
    load: (label, source) => new Promise((resolve, reject) => {
      const decoded = new Image();
      decoded.onload = () => {
        const canvas = document.createElement('canvas');
        canvas.width = decoded.width; canvas.height = decoded.height;
        const context = canvas.getContext('2d');
        context.drawImage(decoded, 0, 0);
        shots.set(label, context.getImageData(0, 0, decoded.width, decoded.height));
        resolve(null);
      };
      decoded.onerror = () => reject(Error('the screenshot does not decode'));
      decoded.src = source;
    }),
    element,
    box: selector => { const r = element(selector).getBoundingClientRect(); return [r.left, r.top, r.right, r.bottom]; },
    rgb: value => value.match(/[a-f0-9]{2}/gi).slice(0, 3).map(hex => parseInt(hex, 16)),
    colour: value => typeof value === 'string' ? __maestroPaint.rgb(JSON.parse(globalThis.colors)[value]) : value,
    count: (label, rect, colour) => {
      let n = 0;
      scan(label, (x, y, data, i) => { if (inside(x, y, rect) && colour.every((v, k) => Math.abs(data[i + k] - v) <= 1)) n++; });
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
    pixel: (label, [x, y]) => {
      const {data, width} = image(label), d = devicePixelRatio, i = (Math.floor(y * d) * width + Math.floor(x * d)) * 4;
      return [...data.subarray(i, i + 4)];
    },
    restyle: (nodes, saved) => nodes.forEach((node, i) => saved[i] === null ? node.removeAttribute('style') : node.setAttribute('style', saved[i])),
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
