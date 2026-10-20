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
  await srcdocError('window.complete()');
}
async function srcdocError(script) {
  const frame = document.createElement('iframe');
  frame.srcdoc = '<script>' + script + '<\\/script>';
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
  // A page error from the policy frame whose message differs.
  await assert.rejects(execute(fixture(t, {[policy]: `await srcdocError("throw TypeError('different sentinel')");`})), /different sentinel/);
});

test('policy_error_in_other_exports_fails', async t => {
  await assert.rejects(execute(fixture(t, {[first]: 'await policyError();'})), /window.complete is not a function/);
});

test('policy_error_from_another_source_fails', async t => {
  // The policy error raised by the top page instead of the policy frame.
  const topPage = "const script = document.createElement('script'); script.textContent = 'window.complete()'; document.body.append(script);";
  await assert.rejects(execute(fixture(t, {[policy]: topPage})), /window.complete is not a function/);
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


const sandboxId = 'probe:"雪]';
const sandboxSetup = String.raw`window.mountWitness = async function(nested, count = 0) {
  for (let i = 0; i <= count; i++) {
    const frame = document.createElement('iframe');
    if (i === 0) frame.id = 'obsolete';
    const loaded = new Promise(resolve => frame.onload = resolve);
    document.body.append(frame); await loaded;
  }
  let doc = document;
  if (nested) {
    const host = document.createElement('iframe');
    const loaded = new Promise(resolve => host.onload = resolve);
    document.body.append(host); await loaded; doc = host.contentDocument;
  }
  const decoy = doc.createElement('div'); decoy.id = 'probe:"雪]-decoy'; doc.body.append(decoy);
  const wrong = doc.createElement('iframe');
  wrong.srcdoc = '<script>window.S={value:"wrong"}</script><button id="target" onclick="window.S.clicked=true">decoy</button>';
  const wrongLoaded = new Promise(resolve => wrong.onload = resolve);
  decoy.append(wrong); await wrongLoaded; window.decoyWitness = wrong;
  const box = doc.createElement('div'); box.id = 'probe:"雪]'; doc.body.append(box);
  const target = doc.createElement('iframe'); target.sandbox = 'allow-scripts allow-modals';
  target.srcdoc = '<head><meta http-equiv="x-test" content="retained"><script>window.S={value:"right",started:true};window.done=Promise.resolve();</script></head><body><button id="target" onclick="window.S.clicked=true">click</button></body>';
  const loaded = new Promise(resolve => target.onload = resolve);
  box.append(target); await loaded;
}`;

function sandboxFixture(t, body, inspect, setup = sandboxSetup) {
  const driver = observedBrowser(async page => {
    const expose = page.exposeBinding.bind(page);
    page.exposeBinding = (name, callback, ...options) => expose(name, name === 'inspectSandbox'
      ? (context, id, action) => inspect(page, () => callback(context, id, action), action)
      : callback, ...options);
  });
  return execute(fixture(t, {[first]: body}, setup), {chromium: driver});
}

const sandboxState = {
  value: 'right', started: true, head: 'meta,script', body: 'button', scaffold: 0,
  metaParent: 'head', origin: 'null', href: 'about:srcdoc',
};

test('sandbox_inspection_ignores_detached_unrelated_frames', async t => {
  for (const nested of [false, true]) for (const action of ['state', 'click']) for (const remove of [false, true]) {
    let armed = false, detached = false;
    const body = `await mountWitness(${nested});
      await window.inspectSandbox(${JSON.stringify(sandboxId)}, ${JSON.stringify(action)});
      const state = await window.inspectSandbox(${JSON.stringify(sandboxId)}, 'state');
      console.log('WITNESS_STATE ' + JSON.stringify(state));
      if (window.decoyWitness.contentWindow.S.clicked) throw Error('decoy was clicked');`;
    let lines;
    await assert.doesNotReject(async () => {
      lines = await sandboxFixture(t, body, async (page, inspect) => {
        if (armed || !remove) return inspect();
        armed = true;
        const obsolete = page.frames().find(frame => frame.parentFrame() && frame.url() === 'about:blank');
        assert(obsolete, 'obsolete frame exists before inspection');
        let release;
        const gate = new Promise(resolve => { release = resolve; });
        const frameElement = obsolete.frameElement.bind(obsolete);
        obsolete.frameElement = async () => { await gate; return frameElement(); };
        const inspecting = inspect();
        inspecting.catch(() => {});
        try {
          const event = once(page, 'framedetached');
          await page.evaluate(() => document.getElementById('obsolete').remove());
          await event;
          detached = obsolete.isDetached();
          assert(detached, 'real frame detached during pending inspection');
        } finally { release(); }
        return inspecting;
      });
    }, 'live target must survive unrelated frame detachment');
    assert.equal(detached, remove);
    const snapshots = lines.filter(line => line.startsWith('WITNESS_STATE ')).map(line => JSON.parse(line.slice(14)));
    assert.deepEqual(snapshots, [{...sandboxState, ...(action === 'click' ? {clicked: true} : {})}]);
    if (action === 'click') {
      assert(lines.includes(`PHASE ${first} waiting for pointer-done ${sandboxId}; frame about:srcdoc`));
      assert(lines.includes(`PHASE ${first} click returned ${sandboxId}`));
    }
    assert.equal(lines.at(-1), 'COMPLETE 45/45');
  }
});

test('sandbox_inspection_reports_missing_mount', async t => {
  for (const id of ['missing', 'empty', sandboxId]) {
    const body = `document.body.innerHTML = '<div id="empty"></div><iframe sandbox="allow-scripts" srcdoc="opaque"></iframe>';
      await window.inspectSandbox(${JSON.stringify(id)}, 'state');`;
    await assert.rejects(sandboxFixture(t, body, (_, inspect) => inspect()), error => {
      assert.equal(error.cause.message.split('\n')[0], 'page.evaluate: Error: mounted frame not found ' + id);
      return true;
    });
  }
});

function observeLookup(page, observe) {
  const acquire = page.evaluateHandle.bind(page);
  page.evaluateHandle = async (...args) => {
    const handle = await acquire(...args);
    await observe(handle);
    return handle;
  };
}

test('sandbox_inspection_propagates_target_errors', async t => {
  for (const action of ['state', 'click', 'close']) {
    const body = `await mountWitness(false); await window.inspectSandbox(${JSON.stringify(sandboxId)}, '${action === 'close' ? 'state' : action}');`;
    let reached = false;
    await assert.rejects(sandboxFixture(t, body, async (page, inspect) => {
      observeLookup(page, handle => {
        const element = handle.asElement();
        const resolve = element.contentFrame.bind(element);
        element.contentFrame = async () => {
          if (action === 'close') { reached = true; await page.close(); return resolve(); }
          const frame = await resolve();
          const remove = async () => {
            const detached = once(page, 'framedetached');
            await page.evaluate(id => document.getElementById(id).firstElementChild.remove(), sandboxId);
            await detached;
            assert(frame.isDetached(), 'selected native frame was removed');
            reached = true;
          };
          if (action === 'state') {
            const evaluate = frame.evaluate.bind(frame);
            frame.evaluate = async (...args) => { await remove(); return evaluate(...args); };
          } else {
            const locator = frame.locator.bind(frame);
            frame.locator = (...args) => {
              const target = locator(...args), click = target.click.bind(target);
              target.click = async () => { await remove(); return click(); };
              return target;
            };
          }
          return frame;
        };
      });
      return inspect();
    }), error => {
      assert.match(error.message, new RegExp('FAIL ' + first));
      assert.match(error.message, /detached|removed|closed/i);
      return true;
    });
    assert(reached, 'native failing operation reached');
  }
});

test('sandbox_inspection_disposes_lookup_handles', async t => {
  for (const outcome of ['success', 'missing', 'close']) {
    const counts = {created: 0, disposed: 0};
    let released;
    const disposed = new Promise(resolve => { released = resolve; });
    const body = `await mountWitness(false);
      console.log('WITNESS_STATE ' + JSON.stringify(await window.inspectSandbox(${JSON.stringify(outcome === 'missing' ? 'absent' : sandboxId)}, 'state')));`;
    const running = sandboxFixture(t, body, (page, inspect) => {
      observeLookup(page, handle => {
        counts.created++;
        const dispose = handle.dispose.bind(handle);
        handle.dispose = async () => { await dispose(); counts.disposed++; released(); };
        if (outcome === 'close') {
          const element = handle.asElement(), resolve = element.contentFrame.bind(element);
          element.contentFrame = async () => { await page.close(); return resolve(); };
        }
      });
      return inspect();
    });
    if (outcome === 'success') {
      const lines = await running;
      assert.deepEqual(lines.filter(line => line.startsWith('WITNESS_STATE ')).map(line => JSON.parse(line.slice(14))), [sandboxState]);
    } else await assert.rejects(running, outcome === 'missing' ? /mounted frame not found absent/ : /closed/i);
    await disposed;
    assert.deepEqual(counts, {created: 1, disposed: 1}, outcome);
  }
});

test('sandbox_inspection_walks_each_document_once', async t => {
  for (const n of [8, 16, 32]) {
    const body = `await mountWitness(true, ${n - 2});
      console.log('WITNESS_STATE ' + JSON.stringify(await window.inspectSandbox(${JSON.stringify(sandboxId)}, 'state')));`;
    const lines = await sandboxFixture(t, body, async (page, inspect) => {
      await page.evaluate(() => {
        window.work = {queries: 0, frames: 0};
        const documents = [document];
        for (const doc of documents) for (const frame of doc.querySelectorAll('iframe')) {
          if (frame.contentDocument) documents.push(frame.contentDocument);
        }
        for (const doc of documents) {
          const query = doc.querySelectorAll.bind(doc);
          doc.querySelectorAll = selector => {
            const values = query(selector);
            if (selector !== 'iframe') return values;
            window.work.queries++;
            return { *[Symbol.iterator]() {
              for (const value of values) { window.work.frames++; yield value; }
            }};
          };
        }
      });
      const result = await inspect();
      const work = await page.evaluate(() => window.work);
      const bounded = observed => {
        assert(observed.queries <= n + 2, 'linear document queries');
        assert(observed.frames <= n + 2, 'linear iframe visits');
      };
      bounded(work);
      const rescanning = await page.evaluate(n => {
        window.work = {queries: 0, frames: 0};
        for (let i = 0; i < n + 2; i++) for (const frame of document.querySelectorAll('iframe')) void frame;
        return window.work;
      }, n);
      assert.throws(() => bounded(rescanning), /linear iframe visits/, 'same bound rejects repeated native scans');
      return result;
    });
    assert.deepEqual(lines.filter(line => line.startsWith('WITNESS_STATE ')).map(line => JSON.parse(line.slice(14))), [sandboxState]);
  }
});
