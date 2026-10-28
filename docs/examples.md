# Examples

The [terminal demonstrations](terminal/examples.md) exercise chat editing, image
rendering, key input and viewport history through the native terminal adapter.
Run them directly with Cargo in a disposable Unix terminal.

## Package examples

The package directory holds an `examples/` folder, beside the README and the
documentation. `examples/README.md` indexes two folders, `extensions/` and
`sdk/`, and each folder has its own index page. The [extension index](../examples/extensions/README.md) lists the shipped
sources and their usage pages. Its table is generated from each example's own
documentation; `just example-index-check` checks it against the tree. Each usage
page keeps its own controlled-run commands and host prerequisites.

A test checks both directions: every entry of a folder is listed in that
folder's index, and every relative link or table row an index carries names a
path that exists.

The default system prompt names this folder, and `get_examples_path` resolves it
from the package directory, so a source run and a standalone build expose the
same path. The standalone copy reads the README, the release history, the
documentation and the examples from the package directory.

## Controlled engine fixture

The DOOM filesystem test uses a small MIT C component, not game code or game
assets. Its committed bytes keep ordinary tests independent of wasi-sdk and
avoid relaxing Rust's unsafe-code policy for generated exports.

To regenerate it, install wasi-sdk 34, wit-bindgen-cli 0.62.0 and wasm-tools
1.262.0, then run from the workspace root:

```sh
WASI_SDK_PATH=/path/to/wasi-sdk python3 crates/maestro-extensions-wasmtime/tests/fixtures/regenerate_doom_files.py
```

Add `--check` to rebuild and compare the bytes without changing the fixture.
The integration test runs this check when `WASI_SDK_PATH` names an installed
SDK; otherwise it prints a `SKIP` message. The save and cleanup tests always
use the committed component.
