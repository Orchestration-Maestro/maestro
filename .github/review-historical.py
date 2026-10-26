"""Run exact review-named tests against genuine pre-capability revisions."""
import os
from pathlib import Path
import subprocess
import sys

root = Path.cwd()
logs = root / 'proof-logs'
logs.mkdir(exist_ok=True)

def git(*args):
    return subprocess.check_output(['git', *args], text=True).strip()

def tree(label, revision):
    path = root.parent / ('historical-' + label)
    subprocess.run(['git', 'worktree', 'add', '--detach', str(path), revision], check=True)
    return path

def source(path):
    return (root / path).read_text()

def function(path, name):
    text = source(path)
    start = text.index('fn ' + name + '(')
    # These named functions end at their first column-zero closing brace.
    end = text.index('\n}', start) + 2
    return text[start:end] + '\n'

def append(path, text):
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open('a') as f:
        f.write('\n' + text)

def red(path, name, package, scope, marker):
    command = [str(Path.home() / '.local/bin/capped'), 'cargo', 'test', '--locked', '-p', package, *scope, name, '--', '--nocapture']
    log = logs / (package + '-' + name + '-red.log')
    with log.open('w') as f:
        f.write('Historical source: ' + git('-C', str(path), 'rev-parse', 'HEAD') + '\nExact test transplanted from: ' + git('rev-parse', 'HEAD') + '\nCOMMAND: ' + repr(command) + '\n')
        f.flush()
        result = subprocess.run(command, cwd=path, stdout=f, stderr=subprocess.STDOUT)
        f.write('\nEXIT: ' + str(result.returncode) + '\n')
    text = log.read_text()
    print(text[-14000:], flush=True)
    assert result.returncode != 0, 'Historical test unexpectedly passed: ' + name
    assert marker in text, 'Missing expected pre-capability diagnostic: ' + marker

kind = sys.argv[1]
if kind in ('guest', 'guest-decoder'):
    relative = 'crates/maestro-extensions-wasm/src/tests/native_io.rs'
    path = tree('decoder-guest', 'b2a473db^')
    append(path / relative, function(relative, 'http_request') + '\n#[test]\n' + function(relative, 'native_response_text_uses_the_existing_decoder'))
    fixture = path / 'crates/maestro-extensions-wasm/src/tests/native_http_host.rs'
    text = fixture.read_text().replace('pub struct Bank {', 'pub struct Bank {\n    /// Controlled text supplied by the decoder test.\n    pub text: String,')
    text = text.replace('impl Default for Bank {\n    fn default() -> Self {\n        Self {', 'impl Default for Bank {\n    fn default() -> Self {\n        Self {\n            text: String::new(),')
    fixture.write_text(text)
    red(path, 'native_response_text_uses_the_existing_decoder', 'maestro-extensions-wasm', ['--lib'], 'no method named `text`')
    for name in (['native_watch_close_releases_its_registration', 'native_watch_failures_do_not_leak_callbacks'] if kind == 'guest' else []):
        path = tree(name, 'e64f8528')
        append(path / relative, '#[test]\n' + function(relative, name))
        red(path, name, 'maestro-extensions-wasm', ['--lib'], 'no method named `watch`')
elif kind == 'process':
    path = tree('process', 'b850cc19^')
    relative = 'crates/maestro-process/src/byte_queue/tests.rs'
    (path / relative).parent.mkdir(parents=True, exist_ok=True)
    (path / relative).write_text(source(relative))
    append(path / 'crates/maestro-process/src/byte_queue.rs', '#[cfg(test)]\nmod tests;')
    red(path, 'retained_front_reads_only_unwritten_bytes', 'maestro-process', ['--lib'], 'no method named `take_front`')
    (path / 'crates/maestro-process/src/byte_queue.rs').write_text(subprocess.check_output(['git', 'show', 'b850cc19^:crates/maestro-process/src/byte_queue.rs'], text=True))
    # The raw owner does not exist in this revision. Wire only its test module, not production.
    append(path / 'crates/maestro-process/src/child_process.rs', '#[cfg(test)]\nmod raw;')
    relative = 'crates/maestro-process/src/child_process/raw/tests.rs'
    (path / relative).parent.mkdir(parents=True, exist_ok=True)
    (path / relative).write_text(source(relative))
    (path / 'crates/maestro-process/src/child_process/raw.rs').write_text('#[cfg(test)]\nmod tests;\n')
    red(path, 'raw_read_failure_keeps_prefix_and_sibling_controls', 'maestro-process', ['--lib'], 'RawChild')
    relative = 'crates/maestro-process/tests/raw_children.rs'
    (path / relative).write_text(source(relative))
    append(path / 'crates/maestro-process/tests/integration.rs', 'mod raw_children;')
    for name in ['raw_child_output_close_is_independent_of_exit', 'raw_child_pipes_survive_control_drop_and_reap_once']:
        red(path, name, 'maestro-process', ['--test', 'integration'], 'unresolved imports')
elif kind == 'integration':
    relative = 'crates/maestro/tests/native_io/http.rs'
    path = tree('decoder-host', 'b2a473db^')
    append(path / relative, function(relative, 'bodies') + '\n#[test]\n' + function(relative, 'native_response_text_uses_the_existing_decoder'))
    name = 'native_response_text_uses_the_existing_decoder'
    command = [str(Path.home() / '.local/bin/capped'), 'cargo', 'nextest', 'run', '--locked', '-p', 'maestro', '--no-fail-fast', '-E', 'test(' + name + ')']
    log = logs / ('maestro-' + name + '-red.log')
    with log.open('w') as f:
        f.write('Historical source: ' + git('-C', str(path), 'rev-parse', 'HEAD') + '\nExact test transplanted from: ' + git('rev-parse', 'HEAD') + '\nCOMMAND: ' + repr(command) + '\n'); f.flush()
        result = subprocess.run(command, cwd=path, stdout=f, stderr=subprocess.STDOUT)
    text = log.read_text(); print(text[-16000:], flush=True)
    assert result.returncode == 100 and 'FAIL' in text and name in text
    path = tree('fault-host', 'b850cc19^')
    relative = 'crates/maestro/tests/native_io/faults.rs'
    (path / relative).parent.mkdir(parents=True, exist_ok=True)
    (path / relative).write_text(source(relative))
    (path / 'crates/maestro/tests/native_io.rs').write_text('#[cfg(target_os = "linux")]\nmod faults;\n')
    append(path / 'crates/maestro/tests/integration.rs', 'mod native_io;')
    red(path, 'native_child_read_failure_follows_the_real_prefix', 'maestro', ['--test', 'integration'], 'native_io')
else:
    raise ValueError(kind)
