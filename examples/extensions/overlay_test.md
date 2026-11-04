# Overlay test

`/overlay-test` opens a floating overlay 70 cells wide over the conversation.
Its upper rows hold wide CJK and Hangul text, words in five theme colors, and
emoji: a joined sequence, a flag and single symbols. The right border lines up
on every row when the host measures cells and composes the overlay over the text
beneath it correctly. The menu below has four entries. Find and Launch hold an
inline text field; Preferences and Dismiss do not.

The up and down arrows move the selection and stop at either end. On an entry
with a field, typed characters go in at the cursor, Backspace deletes the
character before it, and the left and right arrows move it within the text.
Other control input is ignored. A field too short for its text scrolls sideways
to keep the cursor in view, centered until the cursor comes close to either end
of the text; Enter still reports the whole text. The cell under the cursor is
drawn in reverse video. While the overlay has focus, it also places the terminal's hardware
cursor on that cell, so an input method can show its composition there. Enter
closes the overlay and reports the entry, for example `Find: "query"`, or only
its name when its field is empty or it has none. Escape closes it without a
report.

Build with `--example overlay_test` using the [build command](README.md#building-an-example). Loading is opt-in and
needs a component-enabled interactive host with custom UI overlays and
notifications. The controlled `overlay_test_` runs load the built component
through the production runner and show the overlay on the real frame writer over
a recording terminal with supplied key input. They need no provider,
credentials or network.

## Controlled runs

```sh
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- just prepare-tests
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo nextest run --locked \
  -p maestro-extensions-wasmtime --test integration -- overlay_test_
```
