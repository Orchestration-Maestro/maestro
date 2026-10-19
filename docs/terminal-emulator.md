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

## Screens

`Screen` owns a page list, a tracked cursor, saved cursor state, character-set
state and per-screen protocol state. `screen::Options` requires positive dimensions
and defaults to 80 columns by 24 rows. A zero `max_scrollback` disables history;
a positive byte budget retains the page list's active-area minimum.

The cursor starts at the origin with a block shape, default text attributes,
output semantics and no hyperlink or pending wrap. G0 through G3 initially use
Unicode, GL selects G0, GR selects G2 and no single shift is active. Designations,
GL, GR and single shift are independent. `reset` clears content and protocol
state while retaining dimensions, history policy and the cursor's tracked handle.

Cursor movement keeps active coordinates and the tracked cell aligned. Cross-page
moves migrate cursor-owned styles and hyperlinks into the destination page;
absolute moves mark the old and new rows dirty. Horizontal moves do not wrap.
Cell and row accessors borrow the actual page storage, including the preceding
row's final cell when adjacent pages have different stored widths. `cursor_copy`
copies writing state into another screen and can omit the hyperlink.

`set_attribute` applies decoded rendition attributes. Reasserting an unchanged
attribute retains its existing style reference. `manual_style_update` reconciles
a directly edited style with the cursor page, growing or splitting storage when
necessary. Capacity failures remain typed; `set_attribute` attempts to restore
the old style before falling back to default attributes.

Hyperlinks retain owned URI and identity bytes independently of caller buffers.
Implicit identities wrap and regenerate on page crossings; explicit identities
retain their bytes. `cursor_set_hyperlink` gives the current cell its own link
reference, and `end_hyperlink` releases only the cursor's reference. Reusing a full
live hyperlink identity needs no additional string storage. Page replacement
restores cursor references without changing the references held by copied cells.
`append_grapheme(column, scalar)` appends a suffix to a textual cell on the cursor
row. If suffix storage fills, it grows the page through the same retained cursor
owner and retries at the original column. Capacity failures remain typed; cursor
style and hyperlink ownership survive replacement.

Clearing releases page-owned styles, hyperlinks and grapheme suffixes before
applying background-only blanks. `clear_rows` uses inclusive endpoints and each
page's stored width; selective clearing retains protected cells and row metadata.
`cursor_reset_wrap` clears pending wrap. When the current row wraps, it also
clears the next row's continuation and a trailing spacer head. `split_cell_boundary` removes disconnected wide-cell halves without
changing wrap metadata. Clearing a trailing spacer marks its row dirty, including
when the spacer is on the preceding page.

`cursor_down_scroll` scrolls at the active bottom. With history disabled it rotates
active rows, or clears a single-row screen. Otherwise it grows retained storage,
keeps the cursor at the active bottom and migrates managed references on page
crossings. Viewport scrolling delegates to the page list; `scroll_clear` retains
nonempty active rows as history. History and active erasure reload the tracked
cursor. Managed row-copy failures retain their typed errors and partial changes.

`cursor_scroll_above` inserts a blank at the cursor while moving the rows below
it down. It grows once and rotates only the affected page suffix, leaving older
history alone. `cursor_scroll_region_up` discards the top row of a full-width
region ending at the cursor, keeps content outside that region and leaves the
cursor on the new background-filled bottom row.

`screen::Resize` selects positive dimensions, soft-wrap reflow and prompt redraw.
Resizing preserves cursor attributes and owned hyperlinks, updates a saved cursor
through its tracked cell and resets saved coordinates that leave the active area.
A saved pending wrap clears when its cell is no longer at the right edge.
Prompt redraw is explicit: `False` preserves content, `Last` clears the cursor
line, and `True` clears from the prompt's beginning through the remaining rows.
Output cursors do not trigger prompt clearing. A failed resize retains its typed
storage error and earlier changes; reset the screen before reusing its content.

`ScreenSet` initializes the primary screen immediately and creates an alternate
screen only through `get_init`. `get` and `get_mut` borrow initialized screens
without allocating. `active` and `active_mut` borrow the selected resident
screen; `active_key()` reports its identity. `switch_to` returns a typed error
for an uninitialized alternate screen. Each screen retains
its own content and protocol state. Remove alternate storage while the primary
is active; removal advances only the alternate generation, and removing an absent
alternate is a no-op. Recreating it keeps that generation. The primary cannot be
removed. Removing the active alternate screen also returns a typed error;
select the primary first.

`kitty::key::FlagStack` retains eight keyboard-protocol flag sets. Push evicts the
oldest entry when full; pop clears removed entries, and a count of eight or more
resets the history. Set replaces, enables or disables the five supported bits
without changing the stack position.


## Terminal operations

`Terminal::new(cols, rows, scrollback_limit)` constructs a blank terminal and
rejects either zero dimension with `SizeError`. The history limit is in bytes.
Pixel dimensions start at zero. The palette and clear dirty flags start clear.
Grapheme clustering is enabled in both the current and reset-default modes;
the saved value remains the catalogue default until explicitly saved.

`modes::ModeState` retains 41 modes, current values, saved values and reset
defaults. `Mode::from_number` distinguishes ANSI and private numbers and returns
`None` for an unsupported number. `get_report` and `Report::encode` produce
mode reports, including unknown-number responses.

`horizontal_tab` stops at the next tab destination or the right margin.
`horizontal_tab_back` stops at the preceding destination, bounded by the left
margin in origin mode or column zero otherwise. `tab_set` sets the current
column; `tab_clear` clears that column or every stop. Clearing an absent stop
never creates one. `tab_reset` restores interval-eight stops, excluding the
final column. The private storage retains positions across width changes;
growth does not add interval stops.

Screen identity, retained row count, keyboard flags and viewport scrolling use
the selected screen. `scroll_viewport` accepts top, bottom, signed delta or
absolute row destinations. `scrollbar` reports the selected screen's visible
interval. `cols`, `rows` and `max_scrollback` return the configured dimensions
and history byte limit.

`set_default_palette` preserves explicit index overrides and marks the palette
dirty. `default_palette` reads defaults without those overrides. The three
dynamic-colour default setters also retain overrides and mark the palette dirty.
`effective_foreground_color` and `effective_cursor_color` independently select
the override before the default, returning `None` when neither is set. An unset
cursor colour does not fall back to the foreground.

`print`, `print_string`, `print_slice` and `print_repeat` share scalar printing,
charset translation, pending wrap, insert mode and cluster-width effects.
`print_string` treats newline as carriage return followed by linefeed. Selecting
`StatusDisplay::StatusLine` suppresses scalar text, but not that newline handling.
Rows containing image placeholders are marked for renderer lookup. Widening a
cluster retains the original cell's protection on both halves and any wrap spacer,
even if cursor protection changed between scalars.

Cursor moves clamp to the current margins or screen edge. `set_cursor_pos`
uses one-based coordinates and applies scrolling offsets in origin mode.
`save_cursor` and `restore_cursor` retain position, style, protection, wrap,
origin and charset state independently for each screen. `set_attribute`
delegates rendition changes to the active screen.

`carriage_return`, `linefeed`, `backspace`, `index` and `reverse_index` provide
line control. `scroll_up`, `scroll_down`, `insert_lines`, `delete_lines` and
`insert_blanks` respect the scrolling region and preserve the current background
in cleared cells. Managed transfers and printing return typed capacity errors;
changes applied before an error remain visible.

`set_top_and_bottom_margin` takes one-based bounds; zero selects the corresponding
screen edge. `set_left_and_right_margin` applies the same rules when mode 69 is
enabled. Both ignore empty or inverted ranges and home the cursor after an
accepted change, using origin-relative coordinates when origin mode is enabled.

`delete_chars` shifts cells left within the horizontal margins and fills the
vacated cells with the cursor background. Zero counts and cursors outside the
margins do nothing. An admitted deletion clears soft-wrap state and marks its
row dirty. `set_protected_mode` changes cursor protection; turning protection off
retains the most recently selected ISO or DEC protocol.

`set_pwd` and `set_title` copy raw bytes without decoding or length limits.
Their getters borrow the retained bytes and return `None` for an empty value.
OSC ingress validation is separate from these direct setters.

`erase_chars` erases at least one cell, extending through a wide character's
other half where needed. It reaches the screen edge, independently of horizontal
margins. ISO protection remains effective after protection is turned off;
most-recent DEC protection does not make this operation selective. Protected
wide pairs, links and cluster suffixes survive selective boundary cleanup.

`erase_line` accepts right, left, complete and conditional-right modes. Right
erasure resets soft wrapping; left and complete erasure retain it. Supported
modes clear pending wrap and mark the row dirty. Conditional-right erasure does
nothing while wrap is pending; unknown modes always do nothing. ISO or an explicit
selective request preserves protected cells. Selective clearing releases managed
values in one pass and refreshes row metadata once, not once per unprotected run.

`erase_display` clears below, above, the active area or retained history, or
moves active content into history. Complete clearing on the primary screen first
preserves a final nonempty prompt row in history, even with trailing empty rows.
A later nonempty output row or an alternate screen does not trigger that rule.
The selector shares scroll-clear's cell-emptiness rules. Complete clearing marks
the terminal clear flag; partial erasure marks only affected rows.

`decaln` fills the grid with unprotected E cells, releases old managed contents,
resets margins and origin mode, and returns the cursor home. It keeps foreground
and background colours while clearing other rendition attributes. Cursor
protection remains selected but does not protect cells from alignment.

`switch_screen` lazily creates alternate storage without history, ends the old
screen's active hyperlink and carries charset state. Switching to the already
selected screen changes nothing. It returns the old screen identity on a real
switch. `switch_screen_mode` applies the protocol ordering: 47 preserves content;
1047 clears alternate content on exit; 1049 saves on every enable, clears on
entry and restores the primary cursor on disable. Real switches copy cursor
state without copying hyperlinks where the selected mode requires it.

`resize(cols, rows, cell_width_px, cell_height_px)` rejects zero grid dimensions
before mutation. Same-grid calls update effective pixels and end synchronized
output without resetting margins, tabs or clear flags. Changed columns rebuild
interval-eight tab stops; changed rows alone retain custom stops. Primary content
reflows only with wraparound enabled; alternate content never reflows. Cell pixel
sizes are clamped independently to at least one, and multiplication saturates at
`u32::MAX`. Managed-storage refusals may leave earlier screen changes applied;
reset before using content after that refusal.

`deccolm` selects 80 or 132 columns only when mode 40 permits it, clears the active
display and homes the cursor. Without that permission it only clears mode 3.
`full_reset` selects primary before removing alternate storage, resets screen and
keyboard state, restores mode defaults, clears metadata and resets tabs and
margins. Grid size, pixel dimensions and terminal colours survive. Reset marks
the terminal clear flag.

`semantic_prompt` applies shell prompt, input and output phases. Prompt and
continuation marks follow line transitions through the existing line owner.
Explicit input persists until another shell phase; end-of-line input terminates
on a line transition. Output beginning at column zero clears the current row's
prompt mark. `cursor_is_at_prompt` recognizes primary prompt/input state and row
marks, but always returns false on alternate storage.

Prompt redraw defaults to off, including after full reset. An explicit shell
option selects False, True or Last: resize preserves prompts, clears the full
prompt region or clears its last row respectively. Alternate resize never uses
this primary prompt-redraw policy.

## Byte input

`Terminal::write(&[u8])` updates the screen synchronously. It retains incomplete
UTF-8, escape sequences and cursor-default selections between calls, so a single
write and byte-at-a-time input produce the same screen. Invalid UTF-8 subparts
become replacement characters; ESC terminates an incomplete scalar once. Empty
writes do not flush pending input.

The `stream::Stream` decoder also works with a recording handler. Its one
`stream::Handler` trait receives typed `Action` values synchronously; borrowed
parameters remain valid only during the callback. OSC commands keep their
existing owned payloads. Terminal writes temporarily take the retained stream
out of the terminal, lend the terminal to `stream_terminal::Handler`, and restore
the stream without cloning it.

The terminal handler applies cursor movement, edits, erasure, character sets,
tabs, modes, screen switching, keyboard flags, rendition, hyperlinks and shell
prompt phases through their existing owners. Storage errors stop the current
action, not later input. `mouse_tracking_enabled` reads the tracking mode bits;
`modify_other_keys_enabled` reads numeric extended modifier-key mode. Tracking
bits and the selected mouse event or format remain independent.

Query and window requests are observable as typed stream actions. The terminal
handler captures replies and colour effects, routes APC strings and consumes DCS
without changing the screen. Image execution remains disabled. Input decoding
continues after these commands.

## Replies and effects

After `Terminal::write`, drain `take_pty_reply`, `take_bell_count`,
`take_pwd_changes` and `take_clipboard_writes`. Each drain clears only its own
queue. Replies concatenate in input order; bells saturate at `u16::MAX`.

The byte consumer answers operating-status, cursor-position, device-attribute,
version, mode, keyboard-protocol, size and title queries. Cursor reports are relative
to the scrolling origin when origin mode is enabled. Title reports use the stored
title, including an empty title. The default enquiry answer is empty.
`set_color_scheme` selects light or dark status replies; `None` suppresses them.
With mode 2031 enabled, changing between two known schemes queues a report.
Initial discovery, removal and unchanged values do not produce unsolicited reports.

CSI 14, 16 and 18 t report text-area pixels, cell pixels and text-area cells
from the last successful caller resize. No size query replies until both cell
pixel dimensions are nonzero; they start at zero after construction. Mode 2048
queues an in-band size report on every successful resize, including unchanged
grid sizes. It uses cell dimensions clamped to at least one, independently of
the raw query size. Both report paths preserve full pixel products, even when
the screen's stored pixel dimensions saturate at `u32::MAX`. Rejected zero-grid
resizes leave the query size unchanged and queue no report.

Directory changes and accepted clipboard writes are owned byte vectors, queued
in input order. Repeated directory reports each produce a notification, including
reports that clear the directory.

## OSC and APC

OSC 0 and 2 set the title, capped at 1024 bytes. OSC 7, 1337 `CurrentDir=` and
9;9 set the raw working directory, capped at 4096 bytes. The caps apply only at
command ingress; direct `set_title` and `set_pwd` calls remain uncapped. Titles
must be valid UTF-8 before ingress, but truncation can split a scalar. Directory
payloads remain unparsed bytes.

OSC 52 and 1337 `Copy=:` decode clipboard writes with padded standard base64.
Queries, clears, malformed payloads, selection and primary targets capture
nothing. Other targets select the standard clipboard. Only nonempty decoded
writes of at most 192 KiB are queued; no system clipboard access occurs.
The stream ignores control bytes such as tabs and newlines inside OSC strings
before payload decoding. Spaces reach the decoder and are rejected.

OSC 4, 10, 11 and 12 set palette, foreground, background and cursor overrides.
OSC 104 resets selected palette entries or the entire palette; OSC 110, 111 and
112 restore the dynamic defaults. OSC 21 accesses the same retained colours with
named or indexed keys. Unsupported special and dynamic targets change nothing.
Palette mutations mark the palette dirty; dynamic mutations retain their
independent defaults.

Colour queries observe preceding operations in the same command. OSC 4 and
10 through 12 report replicated sixteen-bit RGB channels; an unset cursor falls
back to the foreground. OSC 21 combines query fields in one reply, uses eight-bit
channels and reports supported unset values as empty fields without a cursor
fallback. Unsupported targets produce no query field. Each reply uses the
query's BEL or ST terminator.

APC strings beginning with `G` enter the existing image command parser. Other
protocols are ignored. `apc::Handler` supports byte and slice feeding, drops
malformed or over-budget commands and returns an owned command at `end`.
`Terminal::apc_handler` retains this routing across writes. Its `max_bytes` limits
the encoded image payload, not the control fields; the default is 65 MiB.
Changing the limit or protocol enablement affects future strings, not a parser
already selected by `G`. A new `start` releases an unfinished command.

Image execution is disabled: parsed commands change no screen state and produce
no replies. APC payloads never print as text; normal text resumes after the
string terminator.
