# Space invaders

Build with `--example space_invaders` using the [build command](README.md#building-an-example). Loading
it is opt-in and needs a component-enabled interactive host with custom views,
native sleep, terminal key helpers and a WASI random source. Run `/invaders` in
that host. A terminal
with the enhanced keyboard protocol is needed for smooth press/release movement;
without release events, movement or firing can remain held until the view closes.
The game uses a fixed 60-by-24 playfield and draws 30 rows including its border,
header and controls. Narrow terminals clip every row without changing the
playfield.

Move with Left/Right or A/D and fire with Space/F. Escape saves the game to a
`space-invaders-save` custom session entry and closes the view. Reopening it
loads the newest save from all session entries and starts paused. Any non-release
key resumes it; Escape or Q while paused closes it without changing the save.
During play, Q discards the save. R or Space after a win or loss starts again,
keeping the high score; a win advances the level. Starting a new level clears
held keys and the old save.

Each view owns one cancellable 50 ms ticking operation in an event listener tail.
The host's disposal cancels its pending sleep, removes the listener and clears
held keys. No provider, paid service, process or external file is needed. The
controlled runs invoke the built component's registered command and compare its
ANSI frames and session-entry saves with captured reference outputs. The shared
custom-view driver supplies tick completions and the host supplies random bytes;
no test constructs the game directly. The merge-queue run checks example line
coverage.

## Controlled runs

```sh
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- just prepare-tests
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo nextest run --locked \
  -p maestro-extensions-wasm --lib -- space_invaders_
```
