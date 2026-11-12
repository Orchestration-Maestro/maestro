# Examples

The [terminal demonstrations](terminal/examples.md) exercise chat editing, image
rendering, key input and viewport history through the native terminal adapter.
Run them directly with Cargo in a disposable Unix terminal.

## Quick start

The examples live in the package repository, maestro-packages. A source checkout
fetches them once:

```sh
just setup
```

`just setup` ends by running `development examples`, which copies maestro-packages'
`examples/` folder into the checkout's `examples/`. Open
`examples/README.md` to see what is there.

## Package examples

The package directory holds an `examples/` folder, beside the README and the
documentation. `examples/README.md` indexes the folders, and each folder has its own
index page. The [extension index](../examples/extensions/README.md) lists the extension
sources and their usage pages. Each usage page keeps its own controlled-run commands
and host prerequisites.

The default system prompt names this folder, and `get_examples_path` resolves it
from the package directory, so a source run and a standalone build expose the
same path. The standalone copy reads the README, the release history, the
documentation and the examples from the package directory.

The [authored subagent component](packages/subagents.md) lives in its own package
crate. Build its `component` example rather than a shared extension-example crate.

## Fetching the tree

```sh
cargo run -p maestro-tooling --bin development -- examples [<ref>]
```

`<ref>` is a branch, tag or commit of maestro-packages. Without it, the command
fetches the default branch. The command works like this:

- It fetches with the `git` command into a scratch folder inside the checkout.
- It replaces `examples/` only after the fetch completes. A failed fetch prints
  git's error and leaves the previous tree.
- A source without an `examples/` folder fails the same way.
- `--source <repository>` fetches from another repository, such as a local
  one. Tests use it; it is not needed otherwise.

Git ignores the fetched tree. No Maestro test or CI step reads it, and the
development tools skip a shipped folder that git ignores. `copy-assets` copies
the tree into a standalone layout, so it fails and names the missing folder
when you have not fetched it.

## Adding an example

Examples are package content, so they are added in maestro-packages. Follow its
`docs/examples.md`: add the source with an opening `//!` description and a
same-name usage page, register it in the family crate that builds it, and add its
tests there.

Maestro keeps host-feature tests on its own fixture components under
`crates/maestro-extensions-wasm/tests/support`. A fixture that a test builds by
name carries `// maestro-component: MAESTRO_<NAME>_COMPONENT` after its opening
docs. `just example-index` generates its `cdylib` target in
`crates/maestro-extensions-wasm/Cargo.toml`, and component preparation writes the
named variable into nextest's environment file.
