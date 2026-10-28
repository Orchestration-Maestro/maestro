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

## Adding an extension example

1. Add `examples/extensions/<name>.rs` with an opening `//!` description and a
   same-name Markdown usage page. Register its `cdylib` Cargo target in
   `crates/maestro-extensions-wasm/Cargo.toml`, in name order.
2. If tests need a prepared component, add this line after its opening docs:
   `// maestro-component: MAESTRO_<NAME>_COMPONENT`. The source file stem must
   match its Cargo target. Keep an existing variable's spelling unchanged.
3. Add guest tests as `src/tests/<name>_example.rs`. The build script discovers
   those modules. Executable integration tests also use `<name>_example` as
   their module name; nextest selects every `_example::` module for component
   setup without adding it to the filter.
4. Run `just example-index` and the example's controlled tests.

Component preparation reads the source markers at run time, builds each marked
example and writes its named variable into nextest's environment file. No shared
component list or per-example setup-filter entry is needed.

The README table is derived from source documentation but remains committed.
Cargo's out-of-package `[[example]]` targets and any native integration module
declarations remain shared, explicit entries. Keep each in name order: additions
only overlap alphabetical neighbours, rather than every queued example. Retain
both target/module entries after a merge and regenerate the README table. Other
docs link selected examples to explain a feature; they are not inventories and
need no row for every new example.
