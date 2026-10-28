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
of `crates/maestro-extensions-wasm/Cargo.toml` from the source paths. It generates
the component table of `prepare_author_component` from each source's component
marker.
`just example-index-check` checks all three against the tree. Each usage page
keeps its own controlled-run commands and host prerequisites.

A test checks both directions: every entry of a folder is listed in that
folder's index, and every relative link or table row an index carries names a
path that exists.

The default system prompt names this folder, and `get_examples_path` resolves it
from the package directory, so a source run and a standalone build expose the
same path. The standalone copy reads the README, the release history, the
documentation and the examples from the package directory.

Support files supply their index descriptions through Markdown frontmatter or a
JSON theme name. The [dynamic resources example](../examples/extensions/dynamic-resources.md)
shows a component contributing a skill, prompt template and theme from its installed directory.
