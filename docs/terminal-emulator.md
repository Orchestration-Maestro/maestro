# Terminal emulator

`maestro-terminal` is the layer-0 terminal emulator for workspace panes. It is
pure Rust, inherits the workspace's unsafe-code prohibition and has no build
script or foreign toolchain. Its full contract is the
[terminal emulator specification](specs/terminal-emulator.md).

## Codepoint width

[`maestro_terminal::unicode::codepoint_width`](https://docs.rs/maestro-terminal/latest/maestro_terminal/unicode/fn.codepoint_width.html)
returns a Unicode scalar's terminal width in cells: zero, one or two. It accepts
`char`, so surrogate and out-of-range values cannot enter the interface.

The width uses Unicode 17 compiled properties, tailored for terminal cells.
Controls, default-ignorable characters and nonspacing or enclosing marks are
zero-width; soft hyphen remains visible. Emoji modifiers and prepend characters
keep their standalone widths. Wide and fullwidth characters, regional indicators
and multi-em dashes occupy two cells.

This measures one scalar, not a grapheme cluster. The toolkit's text layout keeps
its separate tab and emoji-sequence rules unchanged.

See [terminal emulator libraries](libraries/terminal-emulator.md) for the data
source and feature selection.
