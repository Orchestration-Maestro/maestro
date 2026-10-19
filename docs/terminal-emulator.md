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

## Parsing

`color::Rgb::parse` accepts named X11 colours, hexadecimal channels and decimal
intensities. Palette assignments retain separate grammar and overflow errors.
The default indexed palette and dynamic colours support independent overrides
and default replacement. X11 lookup folds ASCII case only and keeps internal
spaces unchanged.

`sgr::Parser` iterates borrowed numeric parameters with positional colon bits.
It returns typed attributes, including direct and indexed foreground, background
and underline colours. Unknown attributes retain the complete input plus the
unrecognised group or remaining suffix. This decoder does not apply attributes
to screen cells.

`parser::Parser` consumes one byte at a time and returns three ordered action
slots: exit, transition and entry. CSI parameters and CSI/ESC intermediates are
writable borrowed views; DCS hook fields are read-only. Parameters saturate at
65535 without losing digit presence on long input. The parser retains up to 24
parameters and four intermediate bytes, with positional bits for colon separators.
DCS and APC payload bytes remain separate from their start/end actions.

`osc::Parser` decodes titles, directory reports, clipboard payloads, hyperlinks,
semantic prompt markers and colour requests. Returned commands own their raw
bytes and ordered request lists; resetting or dropping the decoder does not
invalidate them. Text fields are bytes, not validated UTF-8, URLs or base64.
The decoder preserves BEL versus ST for colour replies. Allocation availability
is selected by `osc::Parser::init`; the composed VT parser starts with fixed
capture and exposes `osc_parser.alloc` for callers that need request lists or
larger clipboard captures. Title capture remains fixed in either mode.

OSC parsing does not change a screen, access a clipboard, open a link or write a
reply. Those effects belong to the command consumer.

## History and viewport

`PageList` owns ordered pages, the active screen and retained history. Its byte
limit charges logical page layouts, not allocator RSS. A finite limit is raised
to the minimum needed for the active area and an extra standard page. Growth
prunes old pages when possible, but never removes the active screen to meet a
budget. Zero disables access to historical viewport rows; an absent limit leaves
history unbounded. Retention tests check bounded logical bytes, at least the
recorded retained rows and unchanged visible content. Exact history row counts
can differ with the storage layout.

Points address the active screen, history, viewport or all retained screen rows.
The viewport follows the active screen, stays at the oldest row or follows a
tracked position. Absolute and signed-delta scrolling saturate at the history
and active boundaries. The scrollbar reports total rows, viewport offset and
visible height.

A `Pin` names a checked page lifetime and cell coordinate. Stale, foreign or
out-of-bounds pins miss. Ordinary pins keep their coordinates during in-place
edits; tracked pins follow moved content through growth, page replacement,
splitting, erasure and resize. Release tracked handles explicitly with
`untrack_pin`. Reset keeps retained handles at the cleared origin and marks
removed content as garbage.

Resize can keep row boundaries or reflow wrapped text. Both modes preserve
retained positions and managed cell values; width changes handle wide-cell
spacers and release discarded values. Tracked wide-character pins follow the
character after spacer insertion, not the spacer. Capacity errors remain typed. A failed
resize can leave earlier page changes applied, while keeping page links,
counts and retained ownership valid.

Prompt traversal includes its endpoint row regardless of the endpoint column.
Backward traversal returns the earliest retained continuation when pruning has
removed its primary prompt.
