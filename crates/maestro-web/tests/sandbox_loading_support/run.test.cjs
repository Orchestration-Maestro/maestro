const {test} = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const http = require('node:http');
const {once} = require('node:events');
const {spawnSync} = require('node:child_process');
const runner = require('./run.cjs');
const inventory = require('./exports.json');
const chromium = require(process.env.PLAYWRIGHT_MODULE).chromium;
const executablePath = process.env.CHROMIUM_PATH;
const policy = inventory.expectedError.export;
const first = inventory.execution[0];
const all = [...inventory.execution, ...inventory.loading];

function fixture(t, overrides = {}, setup = '') {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'maestro-driver-test-'));
  t.after(() => fs.rmSync(root, {recursive: true, force: true}));
  runner.materialize(root);
  const bodies = Object.fromEntries(all.map(name => [name, '']));
  bodies[policy] = `await policyError();`;
  Object.assign(bodies, overrides);
  const source = `export default async function() { ${setup} }
async function policyError() {
  const frame = document.createElement('iframe');
  const html = '<script>window.complete()<\\/script>';
  frame.srcdoc = html;
  const loaded = new Promise(resolve => frame.onload = resolve);
  document.body.append(frame);
  await loaded;
}
` + Object.entries(bodies).map(([name, body]) => body === null ? '' : `export async function ${name}() {${body}}`).join('\n');
  fs.writeFileSync(path.join(root, 'sandbox.js'), source);
  return root;
}

async function execute(root, options = {}) {
  const lines = [];
  await runner.run({root, chromium, executablePath, inventory, log: line => lines.push(line), ...options});
  return lines;
}

function observedBrowser(observe) {
  return {async launch(options) {
    const browser = await chromium.launch(options);
    const original = browser.newPage.bind(browser);
    browser.newPage = async (...args) => {
      const page = await original(...args);
      await observe(page, browser);
      return page;
    };
    return browser;
  }};
}

test('inventory_rejects_empty_or_invalid_names', () => {
  for (const value of [null, {}, {...inventory, execution: []}, {...inventory, execution: ['bad-name']}, {...inventory, loading: [first, ...inventory.loading.slice(1)]}]) {
    assert.throws(() => runner.validateInventory(value));
  }
  const selected = runner.validateInventory(inventory);
  assert.equal(selected.execution.length, 26);
  assert.equal(selected.loading.length, 19);
});

test('inventory_requires_every_callable_export', async t => {
  for (const body of [null, '']) {
    const root = fixture(t, {[all.at(-1)]: body, [first]: "throw Error('must not run')"});
    if (body === '') fs.appendFileSync(path.join(root, 'sandbox.js'), `\nexport const not_callable = 9;`);
    const selected = body === null ? inventory : {...inventory, loading: [...inventory.loading.slice(0, -1), 'not_callable']};
    await assert.rejects(execute(root, {inventory: selected}), new RegExp(body === null ? all.at(-1) : 'not_callable'));
  }
});

test('exports_run_serially_in_fresh_pages', async t => {
  const seen = [];
  let closed = 0;
  let release, started, completed = false;
  const gate = new Promise(resolve => { release = resolve; });
  const entered = new Promise(resolve => { started = resolve; });
  const root = fixture(t, {
    [first]: "await window.serialWitness('first');",
    [inventory.execution[1]]: "await window.serialWitness('second');",
  }, `if (window.initialized) throw Error('reused module'); window.initialized = true;`);
  const driver = observedBrowser(async page => {
    assert.equal(seen.length, closed, 'previous page closed before next page');
    seen.push(page);
    page.once('close', () => closed++);
    await page.exposeBinding('serialWitness', async (_, phase) => {
      if (phase === 'first') { started(); await gate; completed = true; }
      else assert(completed, 'next export cannot precede prior settlement');
    });
  });
  const running = execute(root, {chromium: driver});
  await entered;
  assert.equal(seen.length, 2, 'only inventory and the pending first export');
  release();
  const lines = await running;
  assert.equal(seen.length, 46, 'inventory page plus 45 fresh assertion pages');
  assert.equal(new Set(seen).size, 46);
  assert.deepEqual(lines.filter(line => line.startsWith('START ')).map(line => line.split(' ')[2]), all);
  assert.deepEqual(lines.filter(line => line.startsWith('PASS ')).map(line => line.split(' ')[2]), all);
  assert(lines.includes('FAMILY execution 26/26'));
  assert(lines.includes('FAMILY loading 19/19'));
  assert.equal(lines.at(-1), 'COMPLETE 45/45');
});

test('assertion_failures_keep_sync_and_async_errors', async t => {
  for (const body of ["throw Error('sync sentinel')", "await Promise.reject(Error('async sentinel'))"]) {
    const root = fixture(t, {[first]: body});
    if (body.startsWith('throw')) {
      const file = path.join(root, 'sandbox.js');
      fs.writeFileSync(file, fs.readFileSync(file, 'utf8').replace(`export async function ${first}`, `export function ${first}`));
    }
    const lines = [];
    await assert.rejects(execute(root, {log: line => lines.push(line)}), new RegExp(`${first}.*sentinel`, 's'));
    assert(lines.includes(`PHASE ${first} page loaded`));
    assert(lines.includes(`START execution ${first}`));
    assert(!lines.includes(`PASS execution ${first}`));
  }
});

test('unexpected_page_errors_fail', async t => {
  const root = fixture(t, {}, 'await policyError();');
  await assert.rejects(execute(root), /inventory.*window.complete is not a function/s);
  const other = fixture(t, {[first]: `await policyError()`});
  await assert.rejects(execute(other), /window.complete is not a function/);
});

test('expected_policy_error_matches_once', async t => {
  const root = fixture(t);
  assert.equal((await execute(root)).at(-1), 'COMPLETE 45/45');
});

test('expected_policy_error_is_required', async t => {
  await assert.rejects(execute(fixture(t, {[policy]: ''})), /expected policy error count/);
});

test('extra_policy_errors_fail', async t => {
  await assert.rejects(execute(fixture(t, {[policy]: 'await policyError(); await policyError();'})), /expected policy error count/);
});

test('different_policy_errors_fail', async t => {
  await assert.rejects(execute(fixture(t, {[policy]: "throw TypeError('different sentinel')"})), /different sentinel/);
});

test('policy_error_in_other_exports_fails', async t => {
  await assert.rejects(execute(fixture(t, {[first]: 'await policyError();'})), /window.complete is not a function/);
});

test('policy_error_from_another_source_fails', async t => {
  await assert.rejects(execute(fixture(t, {[policy]: 'window.complete();'})), /window.complete is not a function/);
});

async function crashPage(page) {
  const session = await page.context().newCDPSession(page);
  // The protocol call itself rejects when its target is destroyed.
  await session.send('Page.crash').catch(() => {});
}

test('page_crashes_fail', async t => {
  const root = fixture(t, {[first]: 'await window.crashPage();'});
  let pages = 0;
  const driver = observedBrowser(async page => {
    if (++pages !== 2) return;
    await page.exposeBinding('crashPage', () => crashPage(page));
  });
  await assert.rejects(execute(root, {chromium: driver}), /page crashed|crash|closed/i);
});

test('browser_disconnects_fail', async t => {
  const root = fixture(t, {[first]: 'await window.disconnectBrowser(); await new Promise(() => {});'});
  let pages = 0;
  const driver = observedBrowser(async (page, browser) => {
    if (++pages === 2) await page.exposeBinding('disconnectBrowser', () => browser.close());
  });
  await assert.rejects(execute(root, {chromium: driver}), /disconnection|closed/i);
});

function request(url, pathname) {
  return new Promise((resolve, reject) => {
    const parsed = new URL(url);
    http.get({hostname: parsed.hostname, port: parsed.port, path: pathname}, response => {
      const chunks = [];
      response.on('data', chunk => chunks.push(chunk));
      response.on('end', () => resolve({status: response.statusCode, type: response.headers['content-type'], body: Buffer.concat(chunks)}));
    }).on('error', reject);
  });
}

test('resources_close_after_success_and_failure', async t => {
  for (const outcome of ['success', 'launch', 'init', 'assertion', 'crash', 'cleanup']) {
    const root = fixture(t, outcome === 'crash' ? {[first]: 'await window.crashPage();'} : outcome === 'assertion' ? {[first]: "throw Error('assertion sentinel')"} : {}, outcome === 'init' ? "throw Error('init sentinel')" : '');
    let browser, disconnected, url;
    // Observe the actual listening endpoint through the real page navigation.
    const driver = {async launch(options) {
      if (outcome === 'launch') throw Error('launch sentinel');
      browser = await chromium.launch(options);
      disconnected = once(browser, 'disconnected');
      const newPage = browser.newPage.bind(browser);
      browser.newPage = async (...args) => {
        const page = await newPage(...args);
        page.on('request', request => { url ||= new URL(request.url()).origin; });
        if (outcome === 'crash') await page.exposeBinding('crashPage', () => crashPage(page));
        return page;
      };
      if (outcome === 'cleanup') {
        const close = browser.close.bind(browser);
        browser.close = async () => { await close(); throw Error('cleanup sentinel'); };
      }
      return browser;
    }};
    // Capture the real endpoint even when browser launch fails.
    const listen = http.Server.prototype.listen;
    let endpoint;
    http.Server.prototype.listen = function (...args) { endpoint = this; return listen.apply(this, args); };
    try {
      if (outcome === 'success') await execute(root, {chromium: driver});
      else await assert.rejects(execute(root, {chromium: driver}), outcome === 'crash' ? /crash|closed/i : /sentinel/);
      assert.equal(endpoint.listening, false, outcome);
      if (browser) { await disconnected; assert.equal(browser.isConnected(), false); }
      if (url) await assert.rejects(request(url, '/'), /ECONNREFUSED/);
    } finally { http.Server.prototype.listen = listen; if (browser?.isConnected()) await browser.close(); }
  }
});

test('server_exposes_only_packaged_loopback_files', async t => {
  const root = fixture(t);
  const outside = fs.mkdtempSync(path.join(os.tmpdir(), 'maestro-outside-'));
  t.after(() => fs.rmSync(outside, {recursive: true, force: true}));
  fs.writeFileSync(path.join(outside, 'secret'), 'outside sentinel');
  fs.symlinkSync(path.join(outside, 'secret'), path.join(root, 'escape.js'));
  fs.writeFileSync(path.join(root, 'module.wasm'), Buffer.from([0, 97, 115, 109]));
  const {server, url} = await runner.serve(root);
  try {
    assert.equal(server.address().address, '127.0.0.1');
    for (const urlPath of ['/../../Cargo.toml', '/%2e%2e/Cargo.toml', '/escape.js', '/%5c..%5cCargo.toml']) {
      const response = await request(url, urlPath);
      assert.equal(response.status, 403);
      assert.equal(response.body.length, 0);
    }
    assert.equal((await request(url, '/missing.js')).status, 404);
    for (const [urlPath, type] of [['/', 'text/html'], ['/helper.js', 'application/javascript'], ['/module.wasm', 'application/wasm']]) {
      const response = await request(url, urlPath);
      assert.equal(response.status, 200);
      assert.equal(response.type, type);
      assert(response.body.length);
    }
  } finally { await new Promise(resolve => server.close(resolve)); }
});

test('tool_versions_must_match_the_supplied_contract', t => {
  for (const env of [{}, {PLAYWRIGHT_MODULE: 'relative', CHROMIUM_PATH: executablePath}, {PLAYWRIGHT_MODULE: process.env.PLAYWRIGHT_MODULE, CHROMIUM_PATH: 'relative'}]) {
    assert.throws(() => runner.tools(env), /must be absolute/);
  }
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'maestro-tool-version-'));
  t.after(() => fs.rmSync(directory, {recursive: true, force: true}));
  fs.writeFileSync(path.join(directory, 'package.json'), JSON.stringify({version: '0.0.0', main: 'index.js'}));
  fs.writeFileSync(path.join(directory, 'index.js'), '');
  assert.throws(() => runner.tools({...process.env, PLAYWRIGHT_MODULE: directory}), /Playwright version/);
  fs.symlinkSync(process.execPath, path.join(directory, 'wasm-bindgen'));
  assert.throws(() => runner.tools({...process.env, PATH: directory}), /wasm-bindgen CLI version/);
  assert.throws(() => runner.tools({...process.env, CHROMIUM_PATH: process.execPath}), /Chromium version/);
  assert.equal(runner.tools(), chromium);
});

test('packaging_keeps_linked_modules_and_locked_sources', t => {
  const checkout = path.resolve(__dirname, '../../../..');
  assert.doesNotMatch(fs.readFileSync(path.join(__dirname, 'run.cjs'), 'utf8'), /\btimeout\b/, 'build commands carry no deadline');
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'maestro-package-proof-'));
  t.after(() => fs.rmSync(root, {recursive: true, force: true}));
  const authored = ['Cargo.toml', 'Cargo.lock', 'crates/maestro-web/Cargo.toml',
    ...['integration.rs', 'sandbox_execution_browser.rs', 'sandbox_loading_browser.rs'].map(file => `crates/maestro-web/tests/${file}`)];
  const before = authored.map(file => fs.readFileSync(path.join(checkout, file)));
  const home = fs.mkdtempSync(path.join(os.tmpdir(), 'maestro-package-home-'));
  t.after(() => fs.rmSync(home, {recursive: true, force: true}));
  assert.deepEqual(fs.readdirSync(home), [], 'initial Cargo home is empty');
  const child = spawnSync(process.execPath, ['-e',
    'require(process.argv[1]).packageBrowser(process.argv[2], process.argv[3])',
    require.resolve('./run.cjs'), checkout, root], {
    env: {...process.env, CARGO_HOME: home}, encoding: 'utf8',
  });
  assert.ifError(child.error);
  assert.equal(child.status, 0, child.stdout + child.stderr);
  const lines = child.stdout.split('\n');
  const fetch = lines.indexOf('COMMAND cargo fetch --locked');
  const metadata = lines.indexOf('COMMAND cargo metadata --offline --format-version=1');
  const identity = lines.indexOf('LOCK external identity additions: 0');
  assert.equal(lines.filter(line => line === 'COMMAND cargo fetch --locked').length, 1);
  assert(fetch >= 0 && metadata > fetch && identity > metadata, 'locked fetch precedes offline preparation and identity check');
  for (const [index, file] of authored.entries()) assert.deepEqual(fs.readFileSync(path.join(checkout, file)), before[index], file);
  const bindings = fs.readFileSync(path.join(root, 'sandbox.js'), 'utf8');
  for (const name of all) assert(bindings.includes(`export function ${name}(`), name);
  const snippets = path.join(root, 'snippets');
  const files = fs.readdirSync(snippets, {recursive: true}).filter(file => file.endsWith('.js'));
  assert(files.length > 0, 'split linked modules emitted');
  for (const file of files) {
    const original = file.split('/tests/')[1];
    assert(original, 'linked file comes from the existing tests tree');
    assert.deepEqual(fs.readFileSync(path.join(snippets, file)), fs.readFileSync(path.join(__dirname, '..', original)), original);
  }
  assert(bindings.includes('snippets/'), 'bindings reference packaged modules');
  for (const family of ['execution', 'loading']) {
    const source = fs.readFileSync(path.join(__dirname, `../sandbox_${family}_browser.rs`), 'utf8');
    const names = [...source.matchAll(/pub async fn (\w+)\(\)/g)].map(match => match[1]);
    assert.deepEqual(inventory[family], names);
  }
});
