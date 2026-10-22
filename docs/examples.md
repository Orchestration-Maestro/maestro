# Examples

The [terminal demonstrations](terminal/examples.md) exercise chat editing, image
rendering, key input and viewport history through the native terminal adapter.
Run them directly with Cargo in a disposable Unix terminal.

## Package examples

The package directory holds an `examples/` folder, beside the README and the
documentation: `examples/README.md` indexes an `extensions/` folder and an
`sdk/` folder. `extensions/` holds the author component example, which
`just extension-author-component` builds; `sdk/` gains one executable example
per landing, starting with the embedding work in #227. Each folder's index page
names every file the folder holds, and a test keeps both lists equal to the
folder.

The default system prompt names this folder, and `get_examples_path` resolves it
from the package directory, so a source run and a standalone build expose the
same path. The standalone copy reads the README, the release history, the
documentation and the examples from the package directory. The remaining example
files of the reference set are tracked, each with its owner, in #2074.
