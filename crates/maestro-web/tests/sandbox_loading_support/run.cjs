// Build and execute retained sandbox exports without initializing libtest.
const fs = require('node:fs');
const http = require('node:http');
const path = require('node:path');
const os = require('node:os');
const {spawnSync} = require('node:child_process');
const crypto = require('node:crypto');
const assert = require('node:assert/strict');
const deviceScaleFactor = 1.25;
const launchArgs = ['--no-sandbox', `--force-device-scale-factor=${deviceScaleFactor}`, '--js-flags=--expose-gc'];
const families = ['execution', 'loading', 'contributions'];
const counts = {execution: 26, loading: 49, contributions: 194};
const total = Object.values(counts).reduce((sum, count) => sum + count);
// Runtime modules initialize without unsafe-eval; frames load their own document.
const contributionPolicy = "default-src 'self'; script-src 'self' 'wasm-unsafe-eval'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; object-src 'none'";
const fixtureModules = path.resolve(__dirname, '../fixtures/browser_modules');

function validateInventory(value) {
  assert(value && typeof value === 'object', 'missing inventory');
  assert.deepEqual(Object.keys(value).sort(), ['contributions', 'execution', 'expectedError', 'loading'], 'inventory fields');
  const names = [];
  for (const family of families) {
    assert(Array.isArray(value[family]) && value[family].length, `empty ${family} selection`);
    for (const name of value[family]) {
      assert(typeof name === 'string' && /^[a-z][a-z0-9_]*$/.test(name), `invalid ${family} export`);
      assert(!names.includes(name), `duplicate export ${name}`);
      names.push(name);
    }
  }
  for (const family of families) assert.equal(value[family].length, counts[family], `${family} inventory`);
  assert.deepEqual(value.expectedError, {
    export: 'inline_programs_preserve_text_policies_and_order', name: 'TypeError',
    message: 'window.complete is not a function', source: 'about:srcdoc', count: 1,
  });
  return value;
}

function command(program, args, options = {}) {
  console.log('COMMAND', program, ...args);
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'maestro-browser-command-'));
  const output = path.join(directory, 'stdout');
  const descriptor = fs.openSync(output, 'wx');
  try {
    const result = spawnSync(program, args, {...options, stdio: ['ignore', descriptor, 'inherit']});
    const text = fs.readFileSync(output, 'utf8');
    assert.ifError(result.error);
    assert.equal(result.status, 0, `${program} failed (${result.status})\n${text}`);
    return text;
  } finally {
    fs.closeSync(descriptor);
    fs.rmSync(directory, {recursive: true, force: true});
  }
}

function tools(env = process.env) {
  for (const key of ['PLAYWRIGHT_MODULE', 'CHROMIUM_PATH']) {
    assert(env[key] && path.isAbsolute(env[key]), `${key} must be absolute`);
    fs.accessSync(env[key]);
  }
  assert(Number(process.versions.node.split('.')[0]) >= 20, 'Node >=20 required');
  const entry = require.resolve(env.PLAYWRIGHT_MODULE);
  let directory = path.dirname(entry);
  while (!fs.existsSync(path.join(directory, 'package.json'))) directory = path.dirname(directory);
  assert.equal(JSON.parse(fs.readFileSync(path.join(directory, 'package.json'))).version, '1.64.1', 'Playwright version');
  const cli = command('wasm-bindgen', ['--version'], {env}).trim();
  assert.equal(cli, 'wasm-bindgen 0.2.129', 'wasm-bindgen CLI version');
  const locked = command('python3', ['-c',
    "import sys,tomllib; print(next(p['version'] for p in tomllib.load(open(sys.argv[1], 'rb'))['package'] if p['name']=='wasm-bindgen'))",
    path.resolve(__dirname, '../../../../Cargo.lock')], {env}).trim();
  assert.equal(cli, `wasm-bindgen ${locked}`, 'CLI must match locked binding');
  const chromiumVersion = command(env.CHROMIUM_PATH, ['--version'], {env}).trim();
  assert(/\b156\.0\.8078\.4\b/.test(chromiumVersion), 'Chromium version');
  console.log(`TOOLS Node ${process.versions.node} Playwright 1.64.1 ${chromiumVersion} ${cli}`);
  return require(entry).chromium;
}

function artifact(output, kind, name) {
  const messages = output.split('\n').filter(line => line.startsWith('{')).map(line => JSON.parse(line));
  const found = messages.filter(message => message.reason === 'compiler-artifact' && message.target.kind.includes(kind) && message.target.name === name);
  const files = found.flatMap(message => message.filenames).filter(file => file.endsWith('.wasm'));
  assert.equal(files.length, 1, `one emitted ${kind} wasm artifact`);
  console.log('ARTIFACT', files[0]);
  return files[0];
}

function packageBrowser(checkout, output) {
  const metadata = JSON.parse(command('cargo', ['metadata', '--locked', '--no-deps', '--format-version=1'], {cwd: checkout}));
  const target = metadata.target_directory;
  const compiled = command('cargo', ['test', '-p', 'maestro-web', '--test', 'integration', '--target', 'wasm32-unknown-unknown', '--locked', '--no-run', '--message-format=json'], {cwd: checkout});
  artifact(compiled, 'test', 'integration');
  const scratch = fs.mkdtempSync(path.join(os.tmpdir(), 'maestro-browser-test-'));
  try {
    // TOML stays data: resolve inherited dependencies, dev feature unions and current patches.
    command('python3', ['-c', String.raw`
import json, pathlib, shutil, sys, tomllib
root, scratch = map(pathlib.Path, sys.argv[1:])
workspace = tomllib.loads((root/'Cargo.toml').read_text())
member = tomllib.loads((root/'crates/maestro-web/Cargo.toml').read_text())
def absolute(d, base):
    if isinstance(d, str): return {'version':d}
    d = dict(d)
    if d.pop('workspace', False):
        extra = d
        d = absolute(workspace['workspace']['dependencies'][extra.get('package', name)], root)
        d['features'] = list(dict.fromkeys(d.get('features', []) + extra.get('features', [])))
        d.update({k:v for k,v in extra.items() if k!='features'})
    elif 'path' in d: d['path'] = str((base/d['path']).resolve())
    return d
deps = {}
sections = [member['dependencies'],member['target']['cfg(target_arch = "wasm32")']['dependencies'],member['dev-dependencies']]
for section in sections:
    for name, dep in section.items():
        dep = absolute(dep, root/'crates/maestro-web')
        if name in deps: dep['features'] = list(dict.fromkeys(deps[name].get('features', []) + dep.get('features', [])))
        deps[name] = dep
deps['maestro-web'] = {'path':str(root/'crates/maestro-web')}
def toml(v):
    if isinstance(v,dict): return '{'+', '.join(json.dumps(k)+' = '+toml(x) for k,x in v.items())+'}'
    return json.dumps(v)
text = '[package]\nname="maestro-browser-test"\nversion="0.0.0"\nedition="2024"\npublish=false\n[workspace]\n[lib]\ncrate-type=["cdylib"]\npath='+json.dumps(str(root/'crates/maestro-web/tests/integration.rs'))+'\n[dependencies]\n'
text += '\n'.join(json.dumps(k)+' = '+toml(v) for k,v in deps.items())+'\n'
for registry, patches in workspace.get('patch',{}).items():
    text += '[patch.'+json.dumps(registry)+']\n'
    text += '\n'.join(json.dumps(k)+' = '+toml(absolute(v, root)) for k,v in patches.items())+'\n'
text += '[profile.dev]\ndebug="line-tables-only"\n[profile.dev.package."*"]\ndebug=false\n'
(scratch/'Cargo.toml').write_text(text)
shutil.copy(root/'Cargo.lock', scratch/'Cargo.lock')
shutil.copy(root/'rust-toolchain.toml', scratch/'rust-toolchain.toml')
(scratch/'tests').symlink_to(root/'crates/maestro-web/tests', target_is_directory=True)
`, checkout, scratch]);
    const env = {...process.env, CARGO_TARGET_DIR: target};
    command('cargo', ['fetch', '--locked'], {cwd: checkout});
    command('cargo', ['metadata', '--offline', '--format-version=1'], {cwd: scratch, env});
    process.stdout.write(command('python3', ['-c', String.raw`
import pathlib, sys, tomllib
root, scratch = map(pathlib.Path, sys.argv[1:])
def identities(p):
    return {(v['name'],v['version'],v.get('source'),v.get('checksum')) for v in tomllib.loads(p.read_text())['package'] if 'source' in v}
a,b=identities(root/'Cargo.lock'),identities(scratch/'Cargo.lock')
assert not b-a, 'scratch introduced external dependency identities: '+repr(b-a)
print('LOCK external identity additions: 0')
`, checkout, scratch]));
    const runtime = artifact(command('cargo', ['rustc', '--lib', '--target', 'wasm32-unknown-unknown', '--offline', '--locked', '--message-format=json', '--', '--cfg', 'test'], {cwd: scratch, env}), 'cdylib', 'maestro_browser_test');
    fs.mkdirSync(output, {recursive: true});
    command('wasm-bindgen', [runtime, '--target', 'web', '--split-linked-modules', '--out-dir', output, '--out-name', 'sandbox']);
    materialize(output);
    packageModules(output, target);
  } finally {
    fs.rmSync(scratch, {recursive: true, force: true});
  }
  return output;
}

// Build each fixture module optimized from its own manifest and lock, with its own binding glue,
// into a copy of the fixture package.
function packageModules(output, target) {
  fs.cpSync(path.join(fixtureModules, 'package'), path.join(output, 'package'), {recursive: true});
  for (const name of ['a', 'b']) {
    const manifest = path.join(fixtureModules, `module_${name}`, 'Cargo.toml');
    const built = command('cargo', ['build', '--manifest-path', manifest, '--lib', '--release', '--target', 'wasm32-unknown-unknown', '--locked', '--message-format=json'], {env: {...process.env, CARGO_TARGET_DIR: target}});
    const wasm = artifact(built, 'cdylib', `contribution_module_${name}`);
    command('wasm-bindgen', [wasm, '--target', 'web', '--out-dir', path.join(output, 'package', 'modules', name), '--out-name', 'module']);
  }
}

// Digest every host file outside the module package.
function hostDigest(root) {
  const digest = crypto.createHash('sha256');
  const files = fs.readdirSync(root, {recursive: true}).filter(file => !file.startsWith('package') && fs.statSync(path.join(root, file)).isFile()).sort();
  for (const file of files) digest.update(file).update(fs.readFileSync(path.join(root, file)));
  return digest.digest('hex');
}

function materialize(root) {
  fs.writeFileSync(path.join(root, 'index.html'), '<!doctype html><body></body>');
  fs.writeFileSync(path.join(root, 'helper.js'), "export const answer='helper-ok'");
  fs.writeFileSync(path.join(root, 'sandbox.html'), `<script>addEventListener('message',e=>{if(e.data?.type==='sandbox-load'){document.open();document.write(e.data.code);document.close();parent.postMessage({type:'sandbox-installed'},'*');}});parent.postMessage({type:'sandbox-ready'},'*');</script>`);
}

async function serve(root) {
  root = fs.realpathSync(root);
  const server = http.createServer((request, response) => {
    response.setHeader('Access-Control-Allow-Origin', '*');
    try {
      const url = decodeURIComponent(request.url.split('?')[0]);
      assert(!url.includes('\\') && !url.includes('\0') && !url.split('/').includes('..'));
      if (url === '/contributions.html') {
        response.setHeader('Content-Security-Policy', contributionPolicy);
        response.setHeader('Content-Type', 'text/html');
        response.end('<!doctype html><body></body>');
        return;
      }
      const relative = url === '/' ? 'index.html' : url.replace(/^\//, '');
      let file = path.resolve(root, relative);
      assert(file.startsWith(root + path.sep));
      // Preserve fixture-relative helper imports without exposing the source tree.
      if (path.basename(file) === 'helper.js') file = path.join(root, 'helper.js');
      file = fs.realpathSync(file);
      if (!file.startsWith(root + path.sep)) throw new Error('path outside packaged root');
      const bytes = fs.readFileSync(file);
      response.setHeader('Content-Type', {'.html': 'text/html', '.js': 'application/javascript', '.wasm': 'application/wasm'}[path.extname(file)] || 'application/octet-stream');
      response.end(bytes);
    } catch (error) {
      response.writeHead(error.code === 'ENOENT' ? 404 : 403);
      response.end();
    }
  });
  await new Promise((resolve, reject) => {
    server.once('error', reject);
    server.listen(0, '127.0.0.1', resolve);
  });
  return {server, url: `http://127.0.0.1:${server.address().port}`};
}

async function inspectSandbox({page}, id, action, phase) {
  const element = await page.evaluateHandle(id => {
    const documents = [document];
    for (const doc of documents) {
      for (const frame of doc.querySelectorAll('iframe')) {
        if (frame.parentElement.id === id) return frame;
        if (frame.contentDocument) documents.push(frame.contentDocument);
      }
    }
    return null;
  }, id);
  let selected;
  try {
    selected = await element.asElement()?.contentFrame();
  } finally {
    await element.dispose();
  }
  if (!selected) throw new Error('mounted frame not found ' + id);
  if (action === 'click') {
    phase(`waiting for pointer-done ${id}; frame ${selected.url()}`);
    await selected.evaluate(() => { window.pointerDone = null; });

    let session;
    try { session = await page.context().newCDPSession(selected); }
    catch (error) {
      if (!error.message.includes('does not have a separate CDP session')) throw error;
    }
    if (!session) await selected.locator('#target').click();
    else {
      try {
        const target = selected.locator('#target');
        const point = await target.evaluate(node => {
          node.scrollIntoView({block: 'center', inline: 'center', behavior: 'instant'});
          const box = node.getBoundingClientRect();
          return {x: box.x + box.width / 2, y: box.y + box.height / 2};
        });
        for (const type of ['mouseMoved', 'mousePressed', 'mouseReleased']) {
          await session.send('Input.dispatchMouseEvent', {type, ...point,
            button: type === 'mouseMoved' ? 'none' : 'left',
            buttons: type === 'mousePressed' ? 1 : 0, clickCount: 1});
        }
      } finally { await session.detach(); }
    }
    const delivered = await selected.evaluate(() => window.pointerDone);
    if (!delivered?.trusted || !delivered.atTarget) {
      throw new Error('native click did not reach mounted target ' + id);
    }
    phase(`click returned ${id}`);
    return;
  }
  return selected.evaluate(async () => {
    if (document.readyState !== 'complete') await new Promise(r => addEventListener('load', r, {once: true}));
    if (window.S?.started) await window.done;
    return {...window.S, head: [...document.head.children].map(e => e.localName).join(','), body: [...document.body.children].map(e => e.localName).join(','), scaffold: document.querySelectorAll('svg,template').length, metaParent: document.querySelector('meta[http-equiv]')?.parentElement.localName, origin: self.origin, href: location.href};
  });
}

async function run({root, chromium, executablePath, inventory, log = console.log}) {
  validateInventory(inventory);
  const {server, url} = await serve(root);
  let browser, closing = false, disconnected = false, failure;
  let rejectDisconnect;
  const disconnect = new Promise((_, reject) => { rejectDisconnect = reject; });
  // The rejection can precede an awaiting operation during page teardown.
  disconnect.catch(() => {});
  try {
    browser = await chromium.launch({executablePath, args: launchArgs});
    browser.on('disconnected', () => {
      disconnected = true;
      if (!closing) rejectDisconnect(new Error('unexpected browser disconnection'));
    });
    const names = families.flatMap(family => inventory[family]);
    const passed = {execution: 0, loading: 0, contributions: 0};
    const hostBefore = hostDigest(root);
    // Preflight the entire inventory before invoking any assertion.
    await withPage('inventory', async page => {
      const missing = await page.evaluate(async names => {
        const module = await import('/sandbox.js');
        await module.default();
        return names.filter(name => typeof module[name] !== 'function');
      }, names);
      assert.deepEqual(missing, [], `missing/non-callable exports: ${missing.join(', ')}`);
    });
    for (const family of families) {
      for (let index = 0; index < inventory[family].length; index += 2) {
        await Promise.all(inventory[family].slice(index, index + 2).map(async name => {
          log(`START ${family} ${name}`);
          // Exports receive the page bootstrap's module import, as an embedder supplies it.
          await withPage(name, page => page.evaluate(async name => {
            const module = await import('/sandbox.js');
            await module.default();
            await module[name](url => import(url));
          }, name), family === 'contributions' ? '/contributions.html' : '/');
          passed[family]++;
          log(`PASS ${family} ${name}`);
        }));
      }
      log(`FAMILY ${family} ${passed[family]}/${inventory[family].length}`);
    }
    const hostAfter = hostDigest(root);
    assert.equal(hostAfter, hostBefore, 'host bundle changed while modules loaded');
    log(`HOST ${hostBefore} unchanged`);
    log(`COMPLETE ${families.reduce((sum, family) => sum + passed[family], 0)}/${total}`);

    async function withPage(name, action, entry = '/') {
      const pageOptions = {deviceScaleFactor};
      if (name === "tile_dom_remove_does_not_open_and_uses_current_callback") pageOptions.hasTouch = true;
      const page = await Promise.race([browser.newPage(pageOptions), disconnect]);
      const errors = [];
      const phase = message => log(`PHASE ${name} ${message}`);
      let pageFailure, rejectPage;
      const failed = new Promise((_, reject) => { rejectPage = reject; });
      failed.catch(() => {});
      const fail = error => { pageFailure ||= error; rejectPage(error); };
      page.on('crash', () => fail(new Error(`page crashed: ${name}`)));
      page.on('pageerror', error => {
        const expected = inventory.expectedError;
        if (name === expected.export && error.name === expected.name && error.message === expected.message && /(?:\(|\s)about:srcdoc(?::|\))/.test(error.stack || '')) errors.push(error);
        else fail(new Error(`${name}: ${error.message}`, {cause: error}));
      });
      page.on('console', message => {
        log(message.text());
        if (message.text().includes('panicked at')) fail(new Error(`${name}: ${message.text()}`));
      });
      let error;
      try {
        await Promise.race([(async () => {
          await page.exposeBinding('inspectSandbox', (context, id, action) => inspectSandbox(context, id, action, phase));
          await page.exposeBinding('sandboxDriverPhase', ({frame}, message) => phase(`${message}; frame ${frame.url()}`));
          // Real keyboard input and the accessibility tree for the contribution exports.
          await page.exposeBinding('driverPress', ({page}, key) => page.keyboard.press(key));
          await page.exposeBinding('driverAria', ({page}, selector) => page.locator(selector).ariaSnapshot());
          // Real pointer movement or a click at `[x, y, click]` in page coordinates.
          await page.exposeBinding('driverPointer', ({page}, point) => {
            const [x, y, click] = JSON.parse(point);
            return click ? page.mouse.click(x, y) : page.mouse.move(x, y);
          });
          await page.addInitScript(() => {
            addEventListener('message', event => {
              if (event.data?.type === 'pointer-done') void window.sandboxDriverPhase('pointer-done received');
            });
            addEventListener('click', () => { void window.sandboxDriverPhase('click dispatched'); }, true);
          });
          await page.goto(url + entry);
          phase('page loaded');
          await action(page);
          phase('export resolved');
        })(), failed, disconnect]);
      } catch (caught) { error = caught; }
      try { await page.close(); } catch (caught) { error ||= caught; }
      error ||= pageFailure;
      if (error) throw new Error(`FAIL ${name}: ${error.message}`, {cause: error});
      assert.equal(errors.length, name === inventory.expectedError.export ? inventory.expectedError.count : 0, `${name}: expected policy error count`);
      if (name === inventory.expectedError.export) log(`POLICY ${name} ${errors.length}/${inventory.expectedError.count}`);
      assert(!disconnected, `${name}: browser disconnected`);
    }
  } catch (error) { failure = error; }
  finally {
    closing = true;
    try { if (browser) await browser.close(); } catch (error) { failure ||= error; }
    try { await new Promise((resolve, reject) => server.close(error => error ? reject(error) : resolve())); } catch (error) { failure ||= error; }
  }
  if (failure) throw failure;
}

async function main() {
  const chromium = tools();
  const checkout = path.resolve(__dirname, '../../../..');
  console.log('CHECKOUT', command('git', ['rev-parse', 'HEAD'], {cwd: checkout}).trim());
  const inventory = validateInventory(require('./exports.json'));
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'maestro-browser-package-'));
  try {
    packageBrowser(checkout, root);
    await run({root, chromium, executablePath: process.env.CHROMIUM_PATH, inventory});
  } finally { fs.rmSync(root, {recursive: true, force: true}); }
}

module.exports = {deviceScaleFactor, launchArgs, validateInventory, tools, packageBrowser, materialize, serve, run};
if (require.main === module) main().catch(error => { console.error(error); process.exitCode = 1; });
