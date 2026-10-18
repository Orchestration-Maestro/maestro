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

## Image protocol: commands and media

`kitty::graphics::command` parses image control bytes incrementally and decodes
owned payloads. Commands retain transmission, deferred display, deletion and
animation parameters. `Response` writes exact protocol replies only when an
image identifier or number is present. This layer does not execute screen
operations or animation.

`kitty::graphics::image::LoadingImage` retains first-command metadata across
chunks. Completion inflates zlib data, decodes PNG to RGBA, checks dimensions
(1 through 10,000), then checks the exact raw pixel length. Direct accumulation,
file reads, inflated output and decoded pixels each have a 400 MiB limit.

Local media can be disabled independently with `Limits`. Regular files remain
on disk. Temporary files must resolve inside a canonical temporary root and
contain the protocol's filename marker; rejected paths remain untouched.
Cleanup of admitted temporary files is attempted after reading, including
ordinary read failures. Linux shared-memory objects use native names: after a
successful open, unlink is attempted on success or error. Invalid dimensions or ranges return
`InvalidData`. Native macOS shared memory is tracked by
[#1472](https://github.com/Orchestration-Maestro/maestro/issues/1472) and currently
returns `UnsupportedMedium`, as do Windows and Android.

PNG decoding expands palette entries and transparency, strips 16-bit channels
to their high bytes and converts gray, gray-alpha, RGB and RGBA to RGBA. PNG
geometry replaces the declared geometry. Parser and loader errors remain typed;
protocol error-message translation belongs to screen execution.
