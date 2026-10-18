# Terminal emulator

This specification defines `maestro-terminal`, the pure-Rust terminal emulator that runs
inside every workspace pane. The foundation changes are listed in
[Amendment to the foundation specification](#amendment-to-the-foundation-specification),
and the terms in [Glossary](#glossary) are also defined in [the glossary](../../CONTEXT.md).

## Purpose and authority

A pane terminal is a PTY child plus an emulator that turns the child's output into
screens, scrollback and replies, and turns the user's keys and mouse into the bytes the
child expects. The workspace reference runs a native terminal engine for this through a
foreign-function binding. Maestro ports that engine to Rust instead:

- every crate keeps `unsafe_code = "forbid"`, with no exception;
- no foreign code, no build script and no second toolchain enter the build;
- the engine is ported like any reference: each reference file maps to a Rust module with
  the reference's exported names in Rust casing, the inside is idiomatic Rust, the
  reference's own tests are ported red first under Maestro names, and reference defects are
  fixed and listed once.

The port covers what the workspace reference reaches: the 80 engine functions its source
calls and everything they reach, plus every capability a pane user or the daemon gets
through them. What no workspace path reaches is listed under [Out of scope](#out-of-scope)
with its reason.

## Problem statement

Workspace panes need an emulator with the reference's exact screen behaviour: programs in
panes (shells, editors, coding agents) draw full screens, ask the terminal questions and
send images, and agent detection reads the resulting screen. A different emulator would
change what those programs see and what detection reads. The available pure-Rust emulators
lack the image protocol panes use, so pane graphics would fall below the reference. The
native engine needs foreign code, which Maestro forbids.

## Solution

`maestro-terminal` is a concrete deep module with one job, emulating terminals. Its public
interface is the reference engine's library exports, restricted to the in-scope subset:
`Terminal`, `RenderState`, the formatters, `Selection`, the key, mouse and focus encoders,
the codepoint width function and the image-protocol types. The binding layer the
reference exposes for foreign callers, and the workspace reference's wrapper over it,
cancel out: each behaviour they add becomes a method on the type that owns it. There is no
port trait: one implementation needs no seam.

## User stories

1. As a pane user, I want programs in my pane to draw exactly as they do in the reference, so that shells, editors and agents look and behave the same.
2. As a pane user, I want wide characters, combining marks, emoji sequences, flags and skin tones to occupy the cells the reference gives them, so that columns line up.
3. As a pane user, I want scrollback bounded by a byte limit that retains at least as much history as the reference on the recorded controls, with identical visible content.
4. As a pane user, I want resizing to rewrap soft-wrapped lines and keep wide characters and links, so that history stays readable after a resize.
5. As a pane user, I want alternate screens, cursor styles and modes to follow what programs request, so that full-screen programs restore my shell when they exit.
6. As a pane user, I want links, titles, working-directory reports, clipboard writes and colour queries to work through the escape sequences programs use, so that pane programs integrate with the workspace.
7. As a pane user, I want keys, modifiers, the mouse and focus changes encoded exactly as the program asked (legacy, extended keyboard protocol, every mouse format), so that key bindings and mouse support work.
8. As a pane user, I want images sent with the terminal image protocol to appear in my pane, including placeholder-based placements, so that image-capable programs work.
9. As the workspace daemon, I want an incremental render snapshot with per-row dirty flags, so that I redraw only rows that changed.
10. As the workspace daemon, I want plain and styled text over the viewport or the whole screen, hyperlink lookups and tracked rows, so that copy mode, history reads and agent detection work.
11. As the workspace daemon, I want replies, bells, directory changes and clipboard writes handed to me after each write, so that I forward them to the PTY and the user.
12. As a maintainer, I want the emulator free of `unsafe` and foreign code, so that the safety rule has no exception.

## Capabilities kept

Each row exists in Maestro; none falls below the reference.

| # | Capability |
|---|---|
| 1 | A full xterm-class screen: printing, cursor movement, erase, insert and delete, margins, scroll regions, tabs, character sets, protected areas, status-line suppression and the alignment test |
| 2 | Unicode cells: wide characters, combining marks, grapheme clustering on by default and kept across full reset, variation selectors, flags, joiner sequences and skin tones |
| 3 | Scrollback bounded in bytes (zero disables it), total and history row counts, a scrollbar, and viewport scrolling to the bottom, by a delta or to a row, saturating at both ends |
| 4 | Resize with reflow of soft-wrapped rows (wide characters, styles, graphemes, hyperlinks and prompt marks kept), pixel size, the in-band size report, and synchronized output cleared on resize |
| 5 | Primary and alternate screens (all three alternate-screen modes), cursor visibility, blinking and style, and the active-screen query |
| 6 | A retained render snapshot: whole-screen and per-row dirtiness with rows clearable one by one; cursor position, wide-tail flag and style; resolved colours, palette and defaults; per cell width, hyperlink flag, style and graphemes |
| 7 | Text extraction: plain or styled, over the viewport or the screen, by rectangle or stream, with unwrapping and trimming; screen rows with wrap flags; the hyperlink at a cell; cell graphemes; keyboard-protocol state as replayable text |
| 8 | Replies to programs: device status and cursor position, primary, secondary and tertiary device attributes, mode reports, keyboard-protocol query, terminal version, enquiry, size reports in cells and pixels, the title report, colour queries and the colour-scheme report, image-protocol replies |
| 9 | Effects for the caller: bell count, working-directory changes from all three report forms, clipboard writes from both write forms with the workspace's text-only size policy |
| 10 | Modes by number (including focus events, bracketed paste, synchronized output, grapheme clustering, the in-band size report and the colour-scheme report), mouse tracking, the extended modifier-key mode and keyboard-protocol flags |
| 11 | Key encoding (legacy, application cursor and keypad, alt prefix, backarrow, numlock keypad, extended modifiers, every keyboard-protocol flag), mouse encoding (all event classes and formats, motion de-duplication, pixel geometry with padding) and focus encoding |
| 12 | The terminal image protocol: direct, file, temporary-file and shared-memory transmission, RGB, RGBA and PNG, zlib, display with cursor movement, z-order, offsets, source rectangles and cell sizes, every delete form, queries, quiet levels, a storage limit with eviction, placeholder placements, change generations and per-placement render geometry |
| 13 | The codepoint width the multiplexer uses for layout |
| 14 | The default 256-colour palette, palette replacement for theme sync, and the effective foreground and cursor colours |
| 15 | Semantic prompts: clearing at a prompt moves it into history, and the prompt is redrawn on resize only when the shell asks |
| 16 | Hyperlinks with explicit and implicit identifiers |
| 17 | The window title stored for the title report |
| 18 | A tracked grid row that follows edits |

## Design

### Placement

`maestro-terminal` sits at layer 0. Its only internal dependency is `maestro-text`, whose
`Utf8Stream` is the PTY byte decoder, reused unchanged. It inherits the workspace lints,
including `unsafe_code = "forbid"`, and has no build script. The workspace daemon depends on
it directly. Frontends reach the items they need (the codepoint width, the host cell size
and the regular-file image command) through re-exports in `maestro-app`, added by the first
frontend delivery that uses one; no frontend depends on the emulator directly.

### Module map

Every internal edge points down. Where the reference is cyclic, the `impl` block lives in
the dependent module.

| Module | Owns |
|---|---|
| `size`, `point` | cell-count types, page-size bounds, point tags and coordinates |
| `unicode` | codepoint width, grapheme break state, the cluster width effect the printer uses |
| `page` | packed cells and rows, the page's grapheme and hyperlink stores, the byte layout model |
| `style`, `hyperlink` | de-duplicated, reference-counted styles and hyperlink entries |
| `page_list` | the page arena, growth and byte-bounded pruning, pins and tracked pins, iteration, viewport scrolling, erase, resize, reflow, prompt rows |
| `screen`, `screen_set` | cursor, scrolling, clearing, SGR state, hyperlink state, resize, the primary and alternate screens |
| `charsets`, `tabstops`, `modes`, `color`, `x11_color`, `mouse` | character sets, tab stops, the 41 modes with defaults, colour parsing and palettes, terminal mouse modes |
| `terminal` | `Terminal`: printing, line control, cursor, margins, erase, screen switching, resize, reset, prompts, working directory and title, the terminal dirty flags, and the workspace's reads |
| `parser`, `sgr`, `stream`, `osc`, `apc` | the VT state machine, SGR parsing, typed dispatch, OSC commands, APC routing |
| `stream_terminal`, `effects` | the stream handler that applies actions to a terminal, replies and effects |
| `device_attributes`, `device_status`, `size_report` | reply encodings |
| `selection`, `formatter`, `render` | ranges, plain and styled text, the render snapshot |
| `kitty::key`, `kitty::color`, `kitty::graphics`, `kitty::host` | keyboard-protocol flags, the colour protocol, the image protocol, the host-side graphics cache |
| `input`, `focus` | key events, modifiers, key and mouse encoders, mouse pixel geometry, focus encoding |

There is one trait, `stream::Handler`, because the reference itself drives the stream with
two kinds of handler: the terminal's and recording handlers in tests.

### Data model

1. **Page list.** An index arena with generational node identifiers and previous/next
   links: the reference's intrusive list on indices. Prune, split and insert stay
   constant-time, pins keep node identity, and a stale identifier is a checked miss.
2. **Byte budget.** Scrollback is limited in page bytes: the list prunes its oldest page
   when the next page would exceed the limit, never below the active area. The page byte
   size uses the page layout arithmetic (record sizes, set and map layouts, the chunk
   allocator and the host page size) as pure functions over safe storage. This is logical
   page accounting, not process allocator RSS. Charged history stays within the configured
   budget, retains at least the reference's recorded rows, and preserves visible cells,
   styles and links; exact row counts depend on the storage runtime's memory layout.
3. **Cells and rows** are packed `Copy` newtypes with typed accessors.
4. **Page tables.** Styles and links share one deduplicated, reference-counted owner.
   Identifiers, probe geometry, interior reuse and growth thresholds are runtime details;
   admitted operations and value lifetimes are preserved. Grapheme and string stores keep
   first-fit chunk allocation, because it decides when a page runs out of room and grows.
5. **Pins.** A pin names a cell by node, row and column. Tracked pins live in a slab the
   page list updates on every edit and releases explicitly.
6. **Screens.** The primary screen plus an alternate screen created on first use, each
   with its own page list, cursors, character-set state, keyboard-protocol flag stack and
   image storage, and a generation per screen.
7. **Terminal.** The reference terminal plus its stream state and effects. `write` takes
   the stream state out, feeds it through a handler borrowing the terminal, and puts it
   back, so there is no shared mutable cell. `Terminal` is `Send` without any marker.

### Interfaces

1. **Effects.** The caller drains, after each write: replies for the PTY, the bell count
   (saturating), working-directory changes and clipboard writes. The caller sets the colour
   scheme for the colour-scheme report (none means no reply). Enquiry answers empty, the
   terminal-version reply is the reference engine's identity string byte for byte (programs
   in panes detect features from it), and device attributes use the reference defaults.
2. **Clipboard writes.** Read requests are never forwarded; an empty payload is a clear and
   is not captured; the payload uses strict padded standard base64, and a decode error
   captures nothing; only the standard clipboard, non-empty and at most 192 KiB, is kept.
3. **Size.** `Terminal::new` and `Terminal::resize` reject zero columns or rows. A resize
   records the caller's unclamped size for the size reports (any zero means no reply) and
   resizes the screen with cell sizes clamped to at least one pixel, which the pixel size
   and the in-band report use.
4. **Render state** is retained and incremental: `update` reads the terminal, rows expose
   their dirty flag, can clear it and iterate their cells, and cells resolve colours the
   way the reference does.
5. **Encoders.** `KeyEncoder::from_terminal` and `MouseEncoder::from_terminal` read the
   terminal's modes; the mouse encoder keeps its last-cell de-duplication state and pixel
   geometry. A key event owns its text.
6. **Errors.** Allocation failure is not modelled. Unknown mode numbers are `None`; reads
   are infallible.
7. **Image protocol.** Change generations come from one process-wide counter, so they never
   recur across screens or resets. PNG decoding is on exactly for terminals with image
   support enabled. Shared-memory names are a trust boundary: they follow the platform's
   naming rules, are read up to the expected size and are then unlinked.

### Libraries

All are already locked. `icu_properties` 2.3.0 (compiled data) supplies Unicode 17
properties; `base64` 0.23.1 decodes clipboard payloads strictly and image payloads with
indifferent padding; `flate2` 1.1.10 (zlib-rs) decompresses images; `png` 0.17.16 decodes
PNG; `rustix` 1.1.5 with `shm` opens and unlinks shared-memory objects on Unix. Image
fingerprints use the standard library's hasher. `num-traits` 0.2.19 supplies checked
floating-point intensity conversion.

## Implementation decisions

- **Numeric presence.** Saturated CSI/DCS parameters remain present regardless of
  digit count; a wrapping digit counter must not discard them.
- **Special colour reset.** OSC105 reaches the special-colour reset decoder through
  both standalone capture and VT dispatch.

- **One decoding path.** The reference's vector fast paths (UTF-8 runs, escape scanning,
  APC payloads, the print loop) must equal its scalar paths, so Rust keeps the scalar
  behaviour only. The escape split is a slice search.
- **Unicode data.** Width, zero-width-in-cluster, grapheme-break and emoji-variation
  properties are derived from `icu_properties` with the reference's derivation rules and
  checked against the reference's generated table for every code point. The codepoint width
  takes a `char`; the reference's out-of-range input cannot occur.
- **Dirty marking** is the reference's, never coarser or finer: the daemon's redraw trusts
  it. Only the terminal dirty flags with producers in the reference's library build are
  ported (palette and clear); screen-level dirty state has no producer there.
- **Prompt redraw** is off until the shell asks for it, as the reference library defaults.
- **Grapheme clustering** is a default mode, kept across full reset.
- **Configuration** is fixed to what the workspace reference ships: image protocol on with
  its storage limit and media switches as the caller sets them, no regex link matching, no
  terminal-multiplexer control mode, no glyph protocol.
- **Test capacity.** The reference builds its tests with a smaller grapheme capacity; Rust
  uses the production capacity and adapts the affected tests once.

## Testing decisions

- Every in-scope reference test is ported red first, named in Maestro's words, at its
  owning interface: public types through `tests/integration.rs`, crate-private storage
  through the owning module's `tests.rs`. 1,533 reference tests are in scope.
- A test lands with the later of its owner's delivery and the delivery that provides its
  harness: terminal-constructing tests with `Terminal::new`, stream tests with
  `Terminal::write`, image tests that write with image execution.
- The workspace reference's 30 engine tests (one reduced to its content assertion) and its
  25 host-graphics tests are ported as witnesses in the delivery that makes them pass.
- Where a test needs a value only the reference can produce (retained rows per byte limit,
  the Unicode property table, a failing witness), the delivery probes the unmodified
  reference and writes the observed value into the test.
- At least 80% line coverage from behaviour tests; no tautological or implementation-detail
  tests.

## Corrected reference defects

1. A key event kept a borrowed text after the caller's borrow ended, so a later change to
   the caller's string changed the encoded key. The Rust event owns its text.
2. PNG decoding was installed process-wide by the first terminal that enabled images, so
   later terminals decoded PNG whether or not they enabled images. Decoding follows each
   terminal's own setting.
3. The placeholder diacritics table was read by parsing a source file at run time. It is
   constant data.

## Runtime mechanics not copied

- SIMD dispatch and vector loops; results equal the reference's scalar paths.
- Page-aligned single allocations, offset pointers, memory mapping, memory pools and the
  forked hash map; only the size arithmetic stays. Internal identifier reuse, probe
  geometry, growth thresholds and exact byte-budget history-row equality are not copied.
- The foreign-function layer: result codes, sized structs, opaque handles, getter and setter
  enums, multi-get, "query size then fill" buffers, callback trampolines and send markers.
- Allocation-failure error unions and fault injection.
- Compile-time build options; one configuration is fixed.
- Logging and once-only warning statics.
- Debug integrity walkers; they become debug assertions or test helpers.
- Global hooks and global mutable decoder state (the generation counter stays process-wide
  because non-recurrence is reference behaviour).

## Out of scope

Each item is unreachable from every workspace path or replaced by a Maestro owner.

- Terminal-multiplexer control mode, search, page compression, regex links and the
  glyph protocol: compiled out of, or switched off in, the configuration the workspace
  reference ships.
- DCS commands: the library handler ignores them (the parser still consumes the strings);
  the workspace answers capability queries itself.
- Selection gestures and the selection API, render highlights and link cells, prompt
  jumps, screen and page-list cloning, compaction, memory statistics and diagrams: no
  workspace call reaches them; the workspace keeps its own selection and link finding and
  formats ranges with the formatter.
- HTML formatting and the formatter extras other than the keyboard ones.
- OSC commands the stream drops (notifications, progress, pointer shape, text sizing and
  the other unread forms); the title stack; the title, enquiry, version and attribute
  callbacks the workspace never installs (their defaults stay as behaviour).
- Engine functions the workspace never calls (palette generation, contrast, paste
  encoding, report encoders, build information, logging, allocator access and the other
  unused foreign-caller entries), and the key-binding, keymap, modifier-remapping and paste
  layers.
- Data structures used only by fonts, rendering, search or the application, the symbol
  tables, renderer padding balancing, and the image animation semantics (the reference
  itself replies that animation is unimplemented, and so does Maestro).
- Configurations without a callback: the workspace captures every effect from the first
  write.

## Delivery order

| # | Delivery | Blocked by |
|---|---|---|
| 1 | Create the crate, amend the foundation, codepoint width and grapheme rules | — |
| 2 | Page storage | 1 |
| 3 | Page list and byte-bounded scrollback | 2 |
| 4 | VT parser, SGR, colours, character sets and standalone OSC decoding | 1 |
| 5 | Screens | 3, 4 |
| 6 | Terminal printing, cursor, tabs and modes | 5 |
| 7 | Terminal editing, screens, resize and reset | 6 |
| 8 | Byte input through the stream | 7 |
| 9 | OSC effects, APC routing and replies | 8 |
| 10 | Selection, formatters and the render snapshot | 9 |
| 11 | Image-protocol parsing and media loading | 1 |
| 12 | Image protocol in the screen | 10, 11 |
| 13 | Key, mouse and focus encoding | 8 |
| 14 | Host graphics cache and wire encoding | 12 |

## Glossary

- **Terminal emulator:** The pure-Rust engine in `maestro-terminal` that parses a pane
  terminal's output, keeps its screens and byte-bounded scrollback, and encodes its keyboard,
  mouse and focus input.
- **Scrollback limit:** The bytes of page storage a pane terminal keeps beyond its active
  screen; zero disables history.
- **Terminal effects:** What a write hands back to the caller: PTY replies, the bell count,
  working-directory changes and clipboard writes.
- **Render state:** The emulator's retained viewport snapshot with whole-screen and per-row
  dirty flags.
- **Image placement:** An image's position on a pane terminal's grid, given directly or
  through placeholder cells.
- **Host graphics cache:** The multiplexer-side record of images uploaded to the host
  terminal and the placements shown there.

## Amendment to the foundation specification

Delivery 1 applies these edits as its first commit.

In `docs/specs/maestro-port.md`:

1. Common contract, item 7: replace "forbid unsafe code except the terminal engine bindings
   described under [Workspace daemon](#workspace-daemon)" with "forbid unsafe code".
2. Crate table: `maestro-terminal`'s job becomes "emulate terminals"; delete the
   `maestro-terminal-ghostty` row; drop it from the `maestro` row.
3. Workspace daemon: the first paragraph says the executable always wires the daemon and
   the multiplexer and requires direct application and multiplexer dependencies; replace the
   unsafe-exception paragraph with: "`maestro-terminal` is a pure-Rust terminal emulator,
   specified in [terminal emulator](terminal-emulator.md): no foreign code, no build script
   and no unsafe code. The existing guest-authoring generated-binding lint exception is
   unchanged."
4. Crate order, layer 1: drop `maestro-terminal-ghostty`.

Elsewhere: `workspace-crates.json` drops the adapter; `AGENTS.md` and
`docs/architecture.md` drop the unsafe exception and the adapter's required root edge; the
conventions checker drops the adapter's lint exception, policy row and required edge, with
their tests.
