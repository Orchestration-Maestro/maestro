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
of `crates/maestro-extensions-wasm/Cargo.toml` from the source paths, plus the
[extension guide lock](development.md#extension-guide-lock) from the workspace lock.
`just example-index-check` checks all three against the tree. Each usage page
keeps its own controlled-run commands and host prerequisites.

A test checks the generated index against the tree. A directory example has
one row for its documented `main.rs`; its support files need no separate
summaries. Other documented files retain their own rows.

The default system prompt names this folder, and `get_examples_path` resolves it
from the package directory, so a source run and a standalone build expose the
same path. The standalone copy reads the README, the release history, the
documentation and the examples from the package directory.

The [authored subagent component](packages/subagents.md) lives in its own package
crate. Build its `component` example rather than the shared extension-example crate.

## Adding an extension example

1. Add `examples/extensions/<name>.rs` with an opening `//!` description and a
   same-name Markdown usage page. `just example-index` generates its `cdylib`
   Cargo target in `crates/maestro-extensions-wasm/Cargo.toml`.
2. If a test builds the example by name, add this line after its opening docs:
   `// maestro-component: MAESTRO_<NAME>_COMPONENT`. The `component_preparation` tooling
   test fails when a test builds an unmarked example. The file stem names its Cargo target; for
   `<dir>/main.rs`, the directory name is used with hyphens replaced by underscores. Keep an existing variable's spelling unchanged.
3. Test the built component on the [test harness](extensions/testing.md) in
   `crates/maestro-extensions-wasmtime/tests/<name>_example.rs`, and declare the
   module in that crate's `tests/integration.rs`. The `<name>_example` module
   name lets nextest select it for component setup without a filter entry.
   Guest tests of other examples stay in
   `crates/maestro-extensions-wasm/src/tests/<name>_example.rs`, which that
   crate's build script discovers.
4. Run `just example-index` and the example's controlled tests.

Component preparation reads the source markers at run time, builds each marked
example and writes its named variable into nextest's environment file. No shared
component list or per-example setup-filter entry is needed.

The README table and Cargo's `[[example]]` targets are generated and committed.
Regenerate both after merging example additions. Native integration module
declarations remain explicit entries; keep them in name order and retain both
sides after a merge. Other docs link selected examples to explain a feature;
they are not inventories and need no row for every new example.
