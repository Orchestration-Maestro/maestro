# Examples

The [terminal demonstrations](terminal/examples.md) exercise chat editing, image
rendering, key input and viewport history through the native terminal adapter.
Run them directly with Cargo in a disposable Unix terminal.

## Package examples

The package directory holds an `examples/` folder, beside the README and the
documentation. `examples/README.md` indexes two folders, `extensions/` and
`sdk/`, and each folder has its own index page. The [extension index](../examples/extensions/README.md) lists the shipped
sources and their usage pages. `just example-index` generates its table from each
example's own documentation. The same command generates the `[[example]]` targets
of `crates/maestro-extensions-wasm/Cargo.toml` from the source paths. `just example-index-check` checks both against the tree. Each usage page
keeps its own controlled-run commands and host prerequisites.

A test checks the generated index against the tree. A directory example has
one row for its documented `main.rs`; its support files need no separate
summaries. Other documented files retain their own rows.

The default system prompt names this folder, and `get_examples_path` resolves it
from the package directory, so a source run and a standalone build expose the
same path. The standalone copy reads the README, the release history, the
documentation and the examples from the package directory.

## Adding an extension example

1. Add `examples/extensions/<name>.rs` with an opening `//!` description and a
   same-name Markdown usage page. `just example-index` generates its `cdylib`
   Cargo target in `crates/maestro-extensions-wasm/Cargo.toml`.
2. If tests need a prepared component, add this line after its opening docs:
   `// maestro-component: MAESTRO_<NAME>_COMPONENT`. The file stem names its Cargo target; for
   `<dir>/main.rs`, the directory name is used with hyphens replaced by underscores. Keep an existing variable's spelling unchanged.
3. Add guest tests as `src/tests/<name>_example.rs`. The build script discovers
   those modules. Executable integration tests also use `<name>_example` as
   their module name; nextest selects every `_example::` module for component
   setup without adding it to the filter.
4. Run `just example-index` and the example's controlled tests.

Component preparation reads the source markers at run time, builds each marked
example and writes its named variable into nextest's environment file. No shared
component list or per-example setup-filter entry is needed.

The README table and Cargo's `[[example]]` targets are generated and committed.
Regenerate both after merging example additions. Native integration module
declarations remain explicit entries; keep them in name order and retain both
sides after a merge. Other docs link selected examples to explain a feature;
they are not inventories and need no row for every new example.

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
