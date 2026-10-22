# Examples

The [terminal demonstrations](terminal/examples.md) exercise chat editing, image
rendering, key input and viewport history through the native terminal adapter.
Run them directly with Cargo in a disposable Unix terminal.

## Package examples

The package directory holds an `examples/` folder, beside the README and the
documentation. `examples/README.md` indexes two folders, `extensions/` and
`sdk/`, and each folder has its own index page. No example ships yet: the
extension examples arrive with the extension work and the SDK examples with
[the embedding ticket](https://github.com/Orchestration-Maestro/maestro/issues/227).

A test reads the folder and its indexes in both directions: every file in a
folder is named in that folder's index, and every file an index names exists on
disk.

The default system prompt names this folder, and `get_examples_path` resolves it
from the package directory, so a source run and a standalone build expose the
same path. The standalone copy reads the README, the release history, the
documentation and the examples from the package directory. The example files
that do not ship yet are listed with their owners in
[the examples tracking issue](https://github.com/Orchestration-Maestro/maestro/issues/2074).
