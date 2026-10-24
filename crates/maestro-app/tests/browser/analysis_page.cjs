// Drive the committed tool report in Chromium and observe what it paints.
//
// The page keeps both library URLs as the reference writes them, so this script
// serves those URLs from a local cache: the exact versions are fetched once,
// checked against the digests below and reused. A missing cache entry downloads
// again; a failed download or a digest mismatch fails the run instead of skipping.
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

async function assets_() {
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

function serve(page, bodies) {
  const server = http.createServer((request, response) => {
    try {
      if (request.url === '/') {
        response.setHeader('Content-Type', 'text/html; charset=utf-8');
        response.end(page);
        return;
      }
      const found = [...bodies].find(([url]) => new URL(url).pathname === request.url);
      if (!found) throw new Error(`unexpected request ${request.url}`);
      response.setHeader('Content-Type', 'application/javascript; charset=utf-8');
      response.end(found[1]);
    } catch (error) {
      // Log the detail; never echo request or exception text into a response.
      console.log(`UNEXPECTED ${request.url}: ${error.message}`);
      response.statusCode = 404;
      response.setHeader('Content-Type', 'text/plain; charset=utf-8');
      response.end('not found');
    }
  });
  return new Promise((resolve, reject) => {
    server.once('error', reject);
    server.listen(0, '127.0.0.1', () => resolve(server));
  });
}

async function stablePaint(page, selector) {
  const read = () => page.evaluate(name => {
    const canvas = document.querySelector(name);
    return canvas ? canvas.toDataURL() : '';
  }, selector);
  let previous = '';
  for (let attempt = 0; attempt < 200; attempt += 1) {
    const current = await read();
    if (current && current === previous) return current;
    previous = current;
    await new Promise(resolve => setTimeout(resolve, 50));
  }
  throw new Error(`${selector} never finished painting`);
}

async function waitFor(page, condition, what) {
  for (let attempt = 0; attempt < 200; attempt += 1) {
    if (await page.evaluate(condition)) return;
    await new Promise(resolve => setTimeout(resolve, 50));
  }
  throw new Error(`${what} never happened`);
}

const probe = () => {
  const role = name => getComputedStyle(document.documentElement).getPropertyValue(name).trim();
  const channels = value => {
    const hex = value.replace('#', '');
    return [0, 2, 4].map(offset => Number.parseInt(hex.slice(offset, offset + 2), 16));
  };
  const painted = (canvas, [red, green, blue], fromFraction) => {
    const context = canvas.getContext('2d');
    const {width, height} = canvas;
    const top = Math.floor(height * fromFraction);
    const pixels = context.getImageData(0, top, width, height - top).data;
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
      font: role('--maestro-font-body'),
    },
    painted: (name, colour, band) => painted(document.querySelector(name), colour, band),
    summary: () => document.getElementById('toolSummary').textContent,
    bitmap: name => document.querySelector(name).toDataURL(),
    border: () => {
      const row = document.querySelector('#tools tr[data-row]');
      const style = getComputedStyle(row);
      return {width: style.borderTopWidth, colour: style.borderTopColor};
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

async function run(url) {
  const {chromium} = require(process.env.PLAYWRIGHT_MODULE);
  const browser = await chromium.launch({executablePath: process.env.CHROMIUM_PATH, args: launchArgs});
  let failed = 0;
  try {
    const page = await browser.newPage({deviceScaleFactor});
    const errors = [];
    page.on('pageerror', error => errors.push(error.message));
    await page.goto(url);
    await waitFor(page, () => typeof window.Chart === 'function', 'the pinned chart library');
    await page.evaluate(probe);
    const roles = await page.evaluate(() => window.probe.roles);

    const failures = [];
    const observe = (name, actual, expected) => {
      const ok = JSON.stringify(actual) === JSON.stringify(expected);
      if (!ok) failures.push(`${name}: ${JSON.stringify(actual)} !== ${JSON.stringify(expected)}`);
      console.log(`${ok ? 'PASS' : 'FAIL'} ${name}`);
    };

    await stablePaint(page, '#toolHistogram');
    const before = await page.evaluate(() => ({
      summary: window.probe.summary(),
      bitmap: window.probe.bitmap('#toolHistogram'),
    }));
    await page.click('#tools tbody tr[data-row]:nth-child(2)');
    await waitFor(page, () => document.getElementById('toolSelect').value === '1', 'the row click to select');
    await stablePaint(page, '#toolHistogram');
    const after = await page.evaluate(() => ({
      summary: window.probe.summary(),
      bitmap: window.probe.bitmap('#toolHistogram'),
    }));
    await page.selectOption('#toolSelect', '1');
    await stablePaint(page, '#toolHistogram');
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
      (await page.evaluate(colour => window.probe.painted('#tokens', colour, 0) > 100, roles.accent)),
      true,
    );
    observe(
      'chart_text_paints_the_muted_colour',
      // Only the label band, where no bar paints, so a bar cannot stand in for text.
      (await page.evaluate(colour => window.probe.painted('#toolHistogram', colour, 0.75) > 0, roles.muted)),
      true,
    );
    await waitFor(page, () => window.probe.border().width !== '0px', 'the row border width');
    const border = await page.evaluate(() => window.probe.border());
    const borderColour = `rgb(${roles.border.join(', ')})`;
    observe('row_border_paints_the_border_colour', border.colour, borderColour);

    observe('page_errors', errors, []);
    if (failures.length > 0) {
      throw new Error(failures.join('; '));
    }
  } finally {
    await browser.close();
  }
  return failed;
}

(async () => {
  for (const key of ['PLAYWRIGHT_MODULE', 'CHROMIUM_PATH']) {
    assert(process.env[key] && path.isAbsolute(process.env[key]), `${key} must be absolute`);
  }
  const fixture = path.resolve(process.argv[2] || path.join(__dirname, '../fixtures/analysis/tool-report.html'));
  const bodies = await assets_();
  const server = await serve(fs.readFileSync(fixture, 'utf8'), bodies);
  const {port} = server.address();
  try {
    const failed = await run(`http://127.0.0.1:${port}/`);
    assert.equal(failed, 0);
    console.log('PASS analysis_page');
  } finally {
    await new Promise(resolve => server.close(resolve));
  }
})().catch(error => {
  console.error(`FAIL analysis_page: ${error.message}`);
  process.exitCode = 1;
});
