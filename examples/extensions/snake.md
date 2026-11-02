# Snake

Build with `--example snake`, then load the component into a component-enabled
interactive host with custom UI, keyboard helpers, native waits and a WASI random
source. Loading and playing are opt-in; no model, provider or network is needed.
Run `/snake` to open the 40 by 15 board. Arrow keys or WASD steer; each food is
worth 10 points. Escape saves the current game in a `snake-save` session entry
and closes the view. Q closes it and clears the save. A restored game starts
paused: any key except Escape or Q resumes it; those two keys close it without
changing the saved game. After a collision, R or Space restarts with the high
score retained. The newest save is read from the whole session, not only the
current branch. A finished saved game starts a new game with its high score.

The controlled `snake_` runs use the built component with supplied terminal
input, random bytes, history and tick completions. Closing the view cancels its
pending native wait and removes its event listener. The example never writes a
file directly; saving and clearing append ordinary session entries through the
host. Narrow views show fewer columns without changing the game's collision
boundaries.

## Controlled runs

```sh
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- just prepare-tests
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo nextest run --locked \
  -p maestro-extensions-wasm --lib -- snake_
```
