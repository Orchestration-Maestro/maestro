# Border status editor

On `session_start`, the example hides the working row, replaces the footer with
an empty one and installs an editor built on the application's custom editor,
with no side padding. Text, input and application actions stay the custom
editor's; only the two border rows change.

The bottom border shows `<provider>/<model> · <thinking level>` on the left, or
`no model · <thinking level>` without a model. On the right it shows
`ctx <percent>%/<window>k · <directory> (<branch>)`. The percentage and the
window in thousands of tokens round to whole numbers, halves upward. Without a
percentage or a window the estimate reads `ctx ?`. The directory shows the home
directory as `~` for the home itself and the paths below it; any other path,
including a sibling whose name starts like the home, keeps its full spelling.

The branch is read once per session start, in the background, with
`git branch --show-current` in the session directory. A directory outside a
repository, a missing directory, an empty answer or a Git failure shows no
branch and reports no error.

While the agent works, the top border shows a braille spinner. `agent_start`
requests a redraw and advances one of ten frames every 80 ms, requesting a
redraw for each; a second `agent_start` continues from the current frame.
`agent_end` stops the frames and restores the plain border. `session_shutdown`
stops the frames and any later redraw request.

Each border keeps one corner cell at each end and at least three fill cells
between its texts. A narrow row cuts the right text first, then the left. A
one-cell row is a single border cell, and a zero-cell row is empty. While
completion suggestions show, the bottom border stays plain so that every
suggestion remains visible; the status returns when the list closes.

Build with `--example border_status_editor` using the
[build command](README.md#building-an-example). Loading it is opt-in and needs
a component-enabled native host with an interactive UI, native waits and
event-tail execution; the branch also needs Git on `PATH`. The controlled runs
load the built component through the production runner with a supplied model,
context estimate, theme and terminal in disposable directories, and drive the
spinner through held native waits. They need Git but no provider, credentials
or paid service.

## Controlled runs

```sh
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- just prepare-tests
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo nextest run --locked \
  -p maestro-extensions-wasm --lib -- border_status_editor_
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo nextest run --locked \
  -p maestro-extensions-wasmtime --test integration -- border_status_editor_
```
