# Overlay QA tests

Build with `--example overlay_qa_tests` using the [build command](README.md#building-an-example). Loading it is
opt-in and needs a component-enabled interactive host with custom overlay UI,
keyboard helpers and native waits. `/overlay-overflow` also needs `bash` on
`PATH`: it runs a 30-second script that prints long, coloured and hyperlinked
lines, and Escape stops it. No command calls a model, provider or network.

Escape or Ctrl+C closes every demonstration.

- `/overlay-animation` redraws a frame counter, bouncing bar, spinner and colour
  block about 30 times a second and counts the frames drawn in each second.
- `/overlay-anchors` shows a 40-column overlay at each of the nine anchors in
  turn. Space or Right moves to the next anchor, Enter reports the selected
  anchor and Escape cancels without a report.
- `/overlay-margins` and `/overlay-percent` cycle through five margin and offset
  placements, or five row and column percentages, with Space or Right.
- `/overlay-stack` shows three overlapping overlays 400 ms apart. Enter or Escape
  closes the front one, and the final notification lists the overlays in the
  order they closed.
- `/overlay-overflow` streams the script's output into an overlay 90 columns
  wide, or as wide as a narrower terminal. Each line is cut at the border. Up
  and Down scroll.
- `/overlay-edge` places an overlay against the right edge, and
  `/overlay-maxheight` draws 21 rows into an overlay that keeps 10.
- `/overlay-sidepanel` shows a menu a quarter of the terminal wide, at least 30
  columns, that hides while the terminal is narrower than 100 columns. Up and
  Down move the selection.
- `/overlay-toggle` hides its overlay for one second when you press `t`, through
  the handle `on_handle` received.
- `/overlay-passive` clears the editor and shows a typing view beside a
  non-capturing timer that never takes focus.
- `/overlay-focus` clears the editor and shows three overlapping non-capturing
  panels. Tab moves focus through them and back to the controller, and the
  focused panel draws on top.
- `/overlay-streaming` clears the editor, adds a timestamped line every 500 ms
  and keeps the newest eight. Tab moves typing between three input panels.

The controlled `overlay_qa_tests_` runs load the built component into the
production runner with a real frame writer on a recording terminal. They read
the drawn screen, deliver keys through the frame writer's focus and supply a
paused clock for the native waits. The overflow run supplies a controlled process
in place of `bash` and writes its output. No run needs a provider, credentials
or an installation.

## Controlled runs

```sh
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- just prepare-tests
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo nextest run --locked \
  -p maestro-extensions-wasmtime --test integration -- overlay_qa_tests_
```
