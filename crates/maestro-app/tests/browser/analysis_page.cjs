// Drive the committed tool report in Chromium and observe what it paints.
//
// The page keeps both library URLs as the reference writes them, so this script
// intercepts those requests and answers them from a digest-checked local cache:
// the exact versions are fetched once and reused. A missing cache entry downloads
// again, and a failed download, a digest mismatch or any request outside the
// script's own loopback origin fails the run instead of skipping.
const fs = require('node:fs');
const http = require('node:http');
const path = require('node:path');
const crypto = require('node:crypto');
const assert = require('node:assert/strict');

const checkout = path.resolve(__dirname, '../../../..');
const cache = path.join(checkout, 'target/browser-analysis-assets');
const RANGE_URL = 'https://cdn.jsdelivr.net/npm/@tailwindcss/browser@4';
const CHART_URL = 'https://cdn.jsdelivr.net/npm/chart.js@4.4.9/dist/chart.umd.min.js';
const assets = [
  {
    url: RANGE_URL,
    file: 'tailwind-browser-4.3.3.js',
    // The page names the major range; the pinned minor serves the same bytes.
    source: 'https://cdn.jsdelivr.net/npm/@tailwindcss/browser@4.3.3',
    sha256: '6d8c473ef2f8ad63feafc0bd76502dda31501a6c135dc4c6173f6268cde595be',
  },
  {
    url: CHART_URL,
    file: 'chart-4.4.9.umd.min.js',
    source: CHART_URL,
    sha256: 'bce154080959c574be0bb6b1a924ff32f08ebc6ff460c159171f51c53802c844',
  },
];
const deviceScaleFactor = 1.25;
const launchArgs = ['--no-sandbox', `--force-device-scale-factor=${deviceScaleFactor}`];

function digest(bytes) {
  return crypto.createHash('sha256').update(bytes).digest('hex');
}

async function download(asset) {
  let lastError;
  for (let attempt = 1; attempt <= 3; attempt += 1) {
    try {
      const response = await fetch(asset.source);
      if (!response.ok) throw new Error(`${response.status} ${response.statusText}`);
      const bytes = Buffer.from(await response.arrayBuffer());
      const actual = digest(bytes);
      if (actual !== asset.sha256) throw new Error(`sha256 ${actual} does not match the pin`);
      fs.writeFileSync(path.join(cache, asset.file), bytes);
      return;
    } catch (error) {
      lastError = error;
      await new Promise(resolve => setTimeout(resolve, 250 * attempt));
    }
  }
  throw new Error(`cannot fetch ${asset.source}: ${lastError && lastError.message}`);
}

async function cachedAssets() {
  fs.mkdirSync(cache, {recursive: true});
  const bodies = new Map();
  for (const asset of assets) {
    const file = path.join(cache, asset.file);
    if (!fs.existsSync(file)) await download(asset);
    const bytes = fs.readFileSync(file);
    const actual = digest(bytes);
    assert.equal(actual, asset.sha256, `${asset.file} digest`);
    bodies.set(asset.url, bytes);
  }
  return bodies;
}

// Answer the page's two library URLs from the verified cache and abort and record
// every other request outside `origin`, whatever its scheme, so a downloaded asset
// or a request that would leave the machine fails the run.
function serveFromCache(page, bodies, origin) {
  const escaped = [];
  page.route('**/*', route => {
    const url = route.request().url();
    const cached = bodies.get(url);
    if (cached) {
      route.fulfill({
        status: 200,
        contentType: 'application/javascript; charset=utf-8',
        body: cached,
      });
      return;
    }
    if (!url.startsWith(`${origin}/`)) {
      escaped.push(url);
      route.abort();
      return;
    }
    route.continue();
  });
  return escaped;
}

function serve(page) {
  const server = http.createServer((request, response) => {
    if (request.url !== '/') {
      response.statusCode = 404;
      response.end('not found');
      return;
    }
    response.setHeader('Content-Type', 'text/html; charset=utf-8');
    response.end(page);
  });
  return new Promise((resolve, reject) => {
    server.once('error', reject);
    server.listen(0, '127.0.0.1', () => resolve(server));
  });
}

// The page's own animation completion is the render signal: no test here waits on
// a clock. Each chart reports completion for its canvas identifier, so a second
// histogram on the same canvas is a second completion.
function hookRenders() {
  window.__renders = 0;
  window.__done = {};
  window.__waiters = [];
  const finishing = canvas => {
    const id = canvas && canvas.id;
    if (!id) return;
    window.__done[id] = (window.__done[id] || 0) + 1;
    for (const waiter of window.__waiters.slice()) waiter();
  };
  window.__settled = (id, after) => new Promise(resolve => {
    const done = () => (window.__done[id] || 0) > after;
    if (done()) return resolve();
    return window.__waiters.push(() => {
      if (done()) resolve();
    });
  });
  let chart;
  Object.defineProperty(window, 'Chart', {
    configurable: true,
    get: () => chart,
    set: value => {
      chart = new Proxy(value, {
        construct(target, args) {
          const config = args[1] || (args[1] = {});
          const options = config.options || (config.options = {});
          options.animation = {...options.animation, onComplete: () => finishing(args[0])};
          return new target(...args);
        },
      });
    },
  });
}

const probe = () => {
  const role = name => getComputedStyle(document.documentElement).getPropertyValue(name).trim();
  const channels = value => {
    const hex = value.replace('#', '');
    return [0, 2, 4].map(offset => Number.parseInt(hex.slice(offset, offset + 2), 16));
  };
  // The pixels strictly inside one band of the canvas, so a bar never stands in
  // for axis text and axis text never stands in for a bar.
  const painted = (canvas, [red, green, blue], {fromX = 0, toX = 1, fromY = 0, toY = 1}) => {
    const context = canvas.getContext('2d');
    const {width, height} = canvas;
    const left = Math.floor(width * fromX);
    const top = Math.floor(height * fromY);
    const right = Math.floor(width * toX);
    const bottom = Math.floor(height * toY);
    const pixels = context.getImageData(left, top, right - left, bottom - top).data;
    let count = 0;
    for (let index = 0; index < pixels.length; index += 4) {
      if (pixels[index] === red && pixels[index + 1] === green && pixels[index + 2] === blue && pixels[index + 3] === 255) {
        count += 1;
      }
    }
    return count;
  };
  window.probe = {
    roles: {
      accent: channels(role('--maestro-color-accent')),
      muted: channels(role('--maestro-color-muted')),
      border: channels(role('--maestro-color-border')),
      text: channels(role('--maestro-color-text')),
      background: channels(role('--maestro-color-background')),
    },
    painted: (name, colour, band) => painted(document.querySelector(name), colour, band),
    summary: () => document.getElementById('toolSummary').textContent,
    bitmap: name => document.querySelector(name).toDataURL(),
    measure: (name, font) => {
      const element = document.querySelector(name);
      const canvas = document.createElement('canvas');
      const context = canvas.getContext('2d');
      context.font = font || getComputedStyle(element).font;
      return context.measureText(element.textContent).width;
    },
    // The ink extent of a canvas's bottom text row against the body role, so chart
    // typography is observed as painted. The chart library's own default size and
    // weight are named here because a canvas carries no computed style.
    chartTitle: (name, text) => {
      const canvas = document.querySelector(name);
      const {width, height} = canvas;
      const pixels = canvas.getContext('2d').getImageData(0, 0, width, height).data;
      const paintedRow = row => {
        let count = 0;
        for (let x = 0; x < width; x += 1) {
          if (pixels[(row * width + x) * 4 + 3] > 0) count += 1;
        }
        return count;
      };
      let bottom = height - 1;
      while (bottom >= 0 && paintedRow(bottom) === 0) bottom -= 1;
      let top = bottom;
      while (top > 0 && paintedRow(top - 1) > 0) top -= 1;
      let left = width;
      let right = -1;
      for (let y = top; y <= bottom; y += 1) {
        for (let x = 0; x < width; x += 1) {
          if (pixels[(y * width + x) * 4 + 3] === 0) continue;
          left = Math.min(left, x);
          right = Math.max(right, x);
        }
      }
      const measure = document.createElement('canvas').getContext('2d');
      measure.font = `normal 12px ${role('--maestro-font-body')}`;
      return {
        ink: right < left ? 0 : right - left + 1,
        declared: measure.measureText(text).width * window.devicePixelRatio,
      };
    },
    path: () => {
      const code = document.querySelector('main > p > code');
      return {text: code.textContent, images: code.querySelectorAll('img').length};
    },
    payload: () => globalThis.payloadExecuted === undefined,
    payloadName: () => [...document.querySelectorAll('#tools tbody tr td:first-child')]
      .map(cell => cell.textContent)
      .find(text => text.includes('payloadExecuted')) || '',
  };
};

// The screenshot pixels of one element, decoded inside the page so no image
// library is needed: the tool report is observed as it is rasterized, not as it is
// declared. The element is scrolled into view first, as only visible pixels paint.
async function painted(page, selector) {
  const locator = page.locator(selector).first();
  await locator.scrollIntoViewIfNeeded();
  const found = await locator.boundingBox();
  assert.ok(found, `${selector} has a box`);
  const png = await page.screenshot({clip: found});
  const pixels = await page.evaluate(source => new Promise((resolve, reject) => {
    const image = new Image();
    image.onload = () => {
      const canvas = document.createElement('canvas');
      canvas.width = image.width;
      canvas.height = image.height;
      const context = canvas.getContext('2d');
      context.drawImage(image, 0, 0);
      resolve([...context.getImageData(0, 0, image.width, image.height).data]);
    };
    image.onerror = () => reject(new Error('the screenshot does not decode'));
    image.src = source;
  }), `data:image/png;base64,${png.toString('base64')}`);
  return {pixels, box: found};
}

// Pixels of one colour, and the ink extent of everything that is not the background.
function counted(pixels, [red, green, blue]) {
  let count = 0;
  for (let index = 0; index < pixels.length; index += 4) {
    if (pixels[index] === red && pixels[index + 1] === green && pixels[index + 2] === blue
      && pixels[index + 3] === 255) {
      count += 1;
    }
  }
  return count;
}

function inkWidth(pixels, width, background) {
  let left = width;
  let right = -1;
  for (let index = 0; index < pixels.length; index += 4) {
    const [red, green, blue] = [pixels[index], pixels[index + 1], pixels[index + 2]];
    if (red === background[0] && green === background[1] && blue === background[2]) continue;
    const column = (index / 4) % width;
    left = Math.min(left, column);
    right = Math.max(right, column);
  }
  return right < left ? 0 : right - left + 1;
}

async function run(url) {
  const {chromium} = require(process.env.PLAYWRIGHT_MODULE);
  const browser = await chromium.launch({executablePath: process.env.CHROMIUM_PATH, args: launchArgs});
  try {
    const page = await browser.newPage({deviceScaleFactor});
    const errors = [];
    const failures = [];
    const observe = (name, actual, expected) => {
      const ok = JSON.stringify(actual) === JSON.stringify(expected);
      if (!ok) failures.push(`${name}: ${JSON.stringify(actual)} !== ${JSON.stringify(expected)}`);
      console.log(`${ok ? 'PASS' : 'FAIL'} ${name}`);
    };
    const escaped = serveFromCache(page, await cachedAssets(), new URL(url).origin);
    await page.addInitScript(hookRenders);
    page.on('pageerror', error => errors.push(error.message));
    await page.goto(url);
    observe('cdn_assets_come_from_the_cache', escaped, []);
    if (escaped.length > 0) {
      throw new Error(`the page fetched ${escaped[0]} over the network`);
    }
    // Two frames let the pinned style library apply the utilities the page names.
    await page.evaluate(() => new Promise(done => requestAnimationFrame(() => requestAnimationFrame(done))));
    await page.evaluate(probe);
    const roles = await page.evaluate(() => window.probe.roles);

    await page.evaluate(() => window.__settled('toolHistogram', 0));
    const before = await page.evaluate(() => ({
      summary: window.probe.summary(),
      bitmap: window.probe.bitmap('#toolHistogram'),
    }));
    await page.locator('#tools tbody tr[data-row]').nth(1).click();
    await page.evaluate(() => window.__settled('toolHistogram', 1));
    const after = await page.evaluate(() => ({
      summary: window.probe.summary(),
      bitmap: window.probe.bitmap('#toolHistogram'),
    }));
    await page.selectOption('#toolSelect', '1');
    await page.evaluate(() => window.__settled('toolHistogram', 2));
    const explicit = await page.evaluate(() => window.probe.bitmap('#toolHistogram'));
    observe('row_click_changes_the_summary', after.summary !== before.summary, true);
    observe('row_click_repaints_the_histogram', after.bitmap !== before.bitmap, true);
    observe('row_click_paints_the_selected_histogram', after.bitmap === explicit, true);

    observe('payload_stayed_inert', await page.evaluate(() => window.probe.payload()), true);
    observe(
      'payload_name_stays_text',
      await page.evaluate(() => window.probe.payloadName()),
      '</script><script>globalThis.payloadExecuted=1</script>',
    );
    observe(
      'sessions_path_stays_text',
      await page.evaluate(() => window.probe.path()),
      {text: '/fixture/sessions-<img src=x>', images: 0},
    );

    observe(
      'bars_paint_the_accent',
      // Only the plot band where the tallest bar paints, never the axis labels.
      (await page.evaluate(
        colour => window.probe.painted('#tokens', colour, {fromX: 0.06, fromY: 0.1, toY: 0.6}) > 100,
        roles.accent,
      )),
      true,
    );
    observe(
      'chart_text_paints_the_muted_colour',
      // Only the label band, where no bar paints, so a bar cannot stand in for text.
      (await page.evaluate(
        colour => window.probe.painted('#toolHistogram', colour, {fromY: 0.75}) > 0,
        roles.muted,
      )),
      true,
    );
    observe(
      'chart_grid_paints_the_border_colour',
      // A canvas band holding grid lines and bars, never a DOM border, so only the
      // chart's own lines can paint the border role there.
      (await page.evaluate(
        colour => window.probe.painted('#tokens', colour, {fromY: 0.15, toY: 0.35}) > 100,
        roles.border,
      )),
      true,
    );
    const chartTitle = await page.evaluate(() =>
      window.probe.chartTitle('#toolHistogram', 'estimated tokens/result'));
    observe(
      'chart_title_paints_the_body_font',
      // The axis title row is measured against the role, so a chart drawn at another
      // size or weight paints a width the role does not explain.
      Math.abs(chartTitle.ink - chartTitle.declared) <= 4,
      true,
    );
    observe(
      'summary_text_paints_the_muted_colour',
      counted((await painted(page, '#toolSummary')).pixels, roles.muted) > 0,
      true,
    );
    observe(
      'row_border_paints_the_border_colour',
      counted((await painted(page, '#tools tbody tr[data-row]')).pixels, roles.border) > 0,
      true,
    );
    const heading = await painted(page, 'h1');
    observe('heading_paints_the_text_colour', counted(heading.pixels, roles.text) > 0, true);
    // The expected width comes from the shared brand role, not from the page's own
    // rule, so a lost declaration paints a width the role does not explain.
    const declared = await page.evaluate(() => {
      const role = getComputedStyle(document.documentElement)
        .getPropertyValue('--maestro-font-display').trim();
      const style = getComputedStyle(document.querySelector('h1'));
      return window.probe.measure('h1', `${style.fontWeight} ${style.fontSize} ${role}`);
    });
    const ink = inkWidth(heading.pixels, Math.round(heading.box.width * deviceScaleFactor), roles.background);
    observe('heading_paints_the_brand_font', Math.abs(ink / deviceScaleFactor - declared) <= 8, true);
    observe('page_errors', errors, []);
    // The check after navigation misses a request a later interaction makes.
    observe('no_request_left_the_loopback_origin', escaped, []);
    if (failures.length > 0) {
      throw new Error(failures.join('; '));
    }
  } finally {
    await browser.close();
  }
}

(async () => {
  for (const key of ['PLAYWRIGHT_MODULE', 'CHROMIUM_PATH']) {
    assert(process.env[key] && path.isAbsolute(process.env[key]), `${key} must be absolute`);
  }
  const fixture = path.resolve(process.argv[2] || path.join(__dirname, '../fixtures/analysis/tool-report.html'));
  const server = await serve(fs.readFileSync(fixture, 'utf8'));
  const {port} = server.address();
  try {
    await run(`http://127.0.0.1:${port}/`);
    console.log('PASS analysis_page');
  } finally {
    await new Promise(resolve => server.close(resolve));
  }
})().catch(error => {
  console.error(`FAIL analysis_page: ${error.message}`);
  process.exitCode = 1;
});
