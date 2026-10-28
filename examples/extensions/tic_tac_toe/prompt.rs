//! Turn instructions added only while the interactive game is active.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

/// One atomic action per call, with the whole turn emitted as sibling tool calls.
pub(super) const INSTRUCTIONS: &str = r"

## Tic-tac-toe: play O

Play O against the human's X. The human uses the terminal view; your interface
is `tic_tac_toe`.

### Read the position

Each human move arrives with the marked cell, the complete board and your O
cursor coordinates. Use that message as the authoritative position. For a fresh
copy of the board and your cursor, call `tic_tac_toe_see_board`. The human's
cursor is private and is not part of either report.

Your cursor carries over between turns. An accepted `play` returns it to row 0,
column 0. A rejected placement on an occupied cell leaves it unchanged; move
from that position to retry.

### Submit one action per call

Each `tic_tac_toe` call accepts exactly one action, not an action batch:
- `move_up`, `move_down`, `move_left`, `move_right`: shift your cursor by one
  cell in that direction, stopping at the board boundary.
- `play`: mark the selected empty cell with O; an occupied cell returns an error.

### Submit the complete turn together

Choose your destination before issuing calls. Starting at (r0, c0) and targeting
(r, c), issue the vertical moves first: `move_down` r - r0 times when r is larger,
or `move_up` r0 - r times when r is smaller. Next issue `move_right` c - c0 times
when c is larger, or `move_left` c0 - c times when c is smaller. Finish with `play`.

Put the entire ordered sequence in one assistant response. Give each action its
own tool_use block. Do not wait for an action result before submitting the next
one, continue the sequence in a later response, or insert explanatory text
between calls. Use no unrelated tools during the turn. The only other permitted
call is `tic_tac_toe_see_board` when you explicitly need the position again.

From the initial cursor (0, 0), these are complete single-response turns:
- (0, 0): `play`: one call.
- (0, 2): `move_right`, `move_right`, `play`: three calls.
- (1, 1): `move_down`, `move_right`, `play`: three calls.
- (2, 2): `move_down`, `move_down`, `move_right`, `move_right`, `play`: five calls.

### Choose a cell

Take an immediate win if two O marks share a line with an empty third cell.
Otherwise block an equivalent X threat. With neither opportunity, choose the
center before a corner, and a corner before an edge.
";
