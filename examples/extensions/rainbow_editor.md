# Rainbow editor

On every `session_start` the example replaces the host's editor, including one
another extension installed, with an editor built on the application's custom
editor. Typing, submission, history, completion and the application's key
actions work as before. After each input the example checks the text: while it
contains `ultrathink` in any ASCII case, a frame advances every 60 ms and asks
the terminal to redraw; otherwise the animation stops. Text the host sets
without input is checked at the next keystroke.

Rendering colors each occurrence of the word in coral, yellow, green, teal,
blue, purple and pink, one character at a time. A brighter shine with dimmer
neighbors moves across the first ten characters, one per frame, then pauses for
ten frames. A restarted animation continues from the frame where it stopped.
Matching runs on the rendered rows, so a word that wraps or is split by the
cursor's styling stays uncolored. Releasing the editor, as the host does when a
later session start replaces it, cancels the pending frame.

Build with `--example rainbow_editor` using the [build command](README.md#building-an-example). Loading it is
opt-in and needs a component-enabled interactive host with editor replacement,
native waits and event-tail execution, and a terminal with 24-bit color. The
controlled `rainbow_editor_` runs supply terminal input and wait-completion
gates; they need no provider, credentials or installation.


## Controlled runs

```sh
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- just prepare-tests
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo nextest run --locked \
  -p maestro-extensions-wasm --lib -- rainbow_editor_
```
