# Tic-tac-toe

Build with `--example tic_tac_toe`, then load the component into an interactive
component-enabled host with custom UI, keyboard and terminal-width helpers,
message renderers and native waits. Loading and playing are opt-in. A real agent
opponent needs a selected model and its configured credentials and can incur
provider charges. The controlled runs below use supplied waits and a loopback
provider; they need no credentials, paid service or extension installation.

Run `/tic-tac-toe`. You play X with arrow keys and Enter or Space; the agent plays
O through `tic_tac_toe`. Escape or Q closes the view without deleting the board.
After a finished game, R closes the view; opening the command again starts a new
game. The command names the session `Tic-Tac-Toe`.

Each tool call performs one action: `move_up`, `move_down`, `move_left`,
`move_right` or `play`. Moves wait 300 ms and clamp at the edges. The agent emits
all moves and the final play as sibling calls in one response; the tool's
sequential execution mode keeps play behind those waits. A successful play
resets the agent cursor to (0,0); an occupied-cell error leaves it in place for a
retry. `tic_tac_toe_see_board` restates the board and that cursor. The human's
cursor stays private to the view.

Human moves, cursor moves and successful plays append `tic-tac-toe-save` session
entries; occupied-cell errors also save the unchanged board. Reopening or
navigating a branch restores its custom saves and tool-result snapshots in
branch order. A branch with no game snapshots starts empty. Invalid saved marks
or coordinates are rejected before rendering or indexing. Expanded move messages and
tool results show static boards; the final banner highlights the
winning line. The example writes no file directly and adds no built-in tool.


## Controlled runs

```sh
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- just prepare-tests
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo nextest run --locked \
  -p maestro-extensions-wasm --lib -- tic_tac_toe_
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo nextest run --locked \
  -p maestro --test integration -- tic_tac_toe_
```
