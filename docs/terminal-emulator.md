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

## Cell and row records

`Cell` and `page::row::Row` are eight-byte `Copy` records with typed accessors.
Cells distinguish scalar text, text with a cluster suffix and background-only
colours. Width, protection, link presence, style identifiers and shell semantics
are separate fields. Rows track wrapping, managed content, prompts and redraws.
These flags describe table references; the records do not own those tables.

`Style` keeps foreground, background and underline colour sources separate.
Indexed colours resolve against the supplied palette. Background-only cells take
precedence over a style's background. The optional bold policy brightens low
palette indices or replaces default-coloured text; it leaves other direct colours
unchanged. Text attributes and the six underline kinds retain their identity.

## Page storage

`Page` owns its rows, cells, deduplicated styles, cluster suffixes and hyperlink
bytes. Copying rows between pages preserves those values after the source is
cleared or dropped. Partial row copies leave unselected cells alone and combine
both rows' image-placeholder summaries. Full row copies take the source summary.
These summaries do not depend on managed cell ownership. Moves and
swaps transfer managed content without duplicating its ownership; clearing cells
releases their references. `insert_hyperlink` retains an identity until a successful
`set_hyperlink` consumes it. Call `release_hyperlink` for an abandoned identity or
a refused assignment to return its string storage. Reinitialising a page resets its storage and dirty
state at the same capacity.

Cluster suffixes use first-fit 16-byte chunks; hyperlink strings use 32-byte
chunks. Fragmentation can exhaust contiguous space before the total free-byte
count reaches zero. Capacity errors identify the affected table or allocator so
the consuming terminal owner can grow storage and retry.

`Page::layout` computes logical byte regions for rows, cells and each managed
table, including their alignment and target page-size rounding. These are
storage accounting bytes, not the native allocator's process memory usage.
Selected rows can report the capacity needed for their unique styles and links,
every linked cell and chunk-rounded suffix and string bytes. This accounting is
the input to retained-history limits; the history owner applies those limits.

## Character sets and byte transitions

`charset_table` returns the 256-entry ASCII, British or DEC special graphics
mapping. Unicode input has no byte table and returns `None`. `CharsetSlot` names
the four stored sets; `CharsetActiveSlot` distinguishes the left and right sets.

`parse_table::TABLE` is indexed by input byte and `parser::State`. Each entry
contains the destination state and `parser::TransitionAction`. It describes byte
transitions only; it does not execute commands or maintain a terminal screen.
