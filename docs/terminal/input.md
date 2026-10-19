# Buffered terminal input

`StdinBuffer` turns the text or bytes a terminal sends into complete, ordered events.
A terminal delivers input in arbitrary chunks: one chunk can hold several keys, and an
escape sequence can arrive in pieces. The buffer groups what it receives into one
`Data` event per character or complete escape sequence, and one `Paste` event per
bracketed paste. It reads no device and owns no clock or timer. The caller passes each
chunk with the instant it was observed, asks `deadline` when to look again and calls
`expire` at that instant.

```rust
use std::time::Instant;

use maestro_tui::{StdinBuffer, StdinBufferEventMap, StdinBufferInput, StdinBufferOptions};

let mut buffer = StdinBuffer::new(StdinBufferOptions::default());
let now = Instant::now();
let data = |text: &str| StdinBufferEventMap::Data(text.to_owned());

// A mouse report that arrives in two chunks stays whole.
let events = buffer.process(StdinBufferInput::Text("ab\x1b[<35"), now);
assert_eq!(events, [data("a"), data("b")]);
assert_eq!(buffer.get_buffer(), "\x1b[<35");
let events = buffer.process(StdinBufferInput::Text(";20;5m"), now);
assert_eq!(events, [data("\x1b[<35;20;5m")]);
assert_eq!(buffer.deadline(), None);

// A lone escape waits for its deadline, then is released as a key.
buffer.process(StdinBufferInput::Text("\x1b"), now);
let deadline = buffer.deadline().expect("an incomplete sequence arms the deadline");
assert!(buffer.expire(now).is_empty());
assert_eq!(buffer.expire(deadline), [data("\x1b")]);

// Pasted text is one event, with input before and after it delivered separately.
let events = buffer.process(StdinBufferInput::Text("x\x1b[200~one\ntwo\x1b[201~y"), now);
assert_eq!(
    events,
    [data("x"), StdinBufferEventMap::Paste("one\ntwo".to_owned()), data("y")]
);
```

## Events

`process` and `expire` return the events they produce, in the order their input
arrived. `Data` holds one character or one complete escape sequence, except for a
fragment released by `expire` or immediately before a paste starts, and the empty
text an empty input produces outside a paste. `Paste` holds the text between the
paste markers, without them. A caller that passes input on as raw text puts the markers `ESC [ 200 ~` and `ESC [ 201 ~` back around a pasted
text. Nothing is trimmed or normalized: whitespace, control characters and U+FEFF are
`Data` like any other character. A character is never split, so an emoji is one
event.

## Sequences

A character other than `ESC` is a unit of its own. After `ESC`:

- `[` opens a control sequence ending at the first character from `@` to `~`. After
  `ESC [ <` a mouse report must have three nonempty decimal fields and end at `M` or
  `m`; an unrecognized report stays buffered under the fragment rules. After
  `ESC [ M` the legacy mouse report ends after three more characters;
- `]` opens an operating system command, ending at the first bell or `ESC \`;
- `P` and `_` open a device control string and an application command, each ending
  at the first `ESC \`;
- `O` takes one more character; any other character completes the sequence at once,
  as in an alt-modified key.

A sequence that cannot end yet is kept as the fragment, which `get_buffer` returns.
The next chunk is appended to it and the whole is framed again.

## Deadlines

When representable, a fragment's deadline is `StdinBufferOptions::timeout` after
the instant given to `process`, 10 ms by default. The deadline is `None` when that
sum is not an `Instant`. Every `process` call cancels the pending deadline and arms
a new one when a fragment remains and the deadline is representable, even if that
input was empty. `process` never expires a due deadline itself: the caller decides
the order by calling `expire` first.

`expire` does nothing before the deadline. At or after it, it returns the whole
fragment as one `Data` event and clears the deadline, so a fragment is released at most
once. `flush` returns the fragment as text without an
event and cancels the deadline. `clear`, and `destroy` which is the same operation,
drop the fragment, any paste in progress, the deadline and the duplicate memory;
the buffer stays usable.

## Enhanced-key duplicates

A terminal that reports a typed character both as an enhanced key report
`ESC [ code u` and as the raw character would deliver the character twice. After a
report with no modifier field, optionally with colon-separated alternate fields after
the code point, and a code point from 32 up that is a Unicode scalar value, the raw
character that immediately follows, even in the next chunk, is dropped once. The memory covers only the next event: a different event, including the
empty `Data` of an empty input, replaces it. A nonempty `flush`, `clear` and both
paste markers also end it; an empty `flush` leaves it.

## Bytes

`StdinBufferInput::Bytes` carries one chunk read from a device. A chunk of one byte
above 127 is the legacy form of alt with the character 128 lower, so it becomes `ESC`
followed by that character. Every other chunk is decoded as UTF-8 on its own, with
U+FFFD for each invalid part. A chunk that ends inside a multibyte character is
therefore not completed by the next chunk: a reader of a raw device decodes its
stream first and passes `Text`.

## Bracketed paste

Once `ESC [ 200 ~` is complete, the text after it is held until `ESC [ 201 ~` arrives,
however the text and the end marker are split across chunks, and has no deadline. A
start marker split across chunks is a fragment like any other and can be released
by `expire` at its representable deadline. The paste ends at the first end marker;
a start marker inside it is part of the text. A fragment kept before the start marker is delivered as `Data` ahead of the
`Paste`.
Empty input while a paste is open produces nothing. `get_buffer` and `flush` never
return the text of a paste in progress, and `clear` discards it.

## Raw host bytes

`stdin_buffer::RawInputByteFramer` retains original bytes across chunks. It shares
sequence recognition with `StdinBuffer`, but does not use chat byte decoding or
suppress duplicate characters. Split UTF-8 scalars wait for continuation bytes;
invalid lead bytes are dropped at timeout. Default mouse frames consume exactly
six bytes, including non-UTF-8 coordinates. A complete UTF-8 bracketed paste is
returned with its markers as one frame, even when its terminator arrives in pieces.
An incomplete paste is not released by a timeout.

```rust
use maestro_tui::stdin_buffer::RawInputByteFramer;

let mut input = RawInputByteFramer::default();
assert!(input.push(&[0xc3]).is_empty());
assert!(input.flush_timeout().is_empty());
assert_eq!(input.push(&[0xa9]), ["é".as_bytes()]);
```

The framer owns no timer. Host callers use a 10 ms idle flush, or 150 ms only
while `has_pending_incomplete_mouse_sequence` is true. `flush_timeout` releases
a lone Escape, keeps partial UTF-8, and discards incomplete control sequences.
After a mouse timeout it recognizes orphaned SGR tails up to 32 bytes and bounds
continued tail disposal at 128 bytes, preserving following input.

OSC strings end at the first BEL or ST; DCS, APC, PM and SOS strings end only at
ST. An incomplete OSC 10 or 11 colour reply keeps waiting. Other timed-out
strings enter bounded tail disposal rather than becoming Alt keys.

`host_color_query_sent` arms 258 colour replies, and `host_cell_size_query_sent`
arms one cell-size reply. While a query is outstanding, an Escape or expected
CSI prefix gets at most one extra idle flush. Complete replies settle their own
windows. Appearance tracking can rearm colour replies; the focus-query opt-in
arms an appearance reply on focus gain. Neither opt-in delays an unrelated
Escape before the triggering event.

`Default` preserves legacy doubled-Escape Alt input. `for_host_input` preserves
it on macOS and separates the Escapes on other hosts. Tests and supplied host
policies can select either behavior with `with_host_input_policy` without
changing the process environment. No terminal reader or query writer is started.

## Shared mouse fields

`stdin_buffer::scan_sgr_mouse_fields` borrows the button, column, row and any
extra fields from one complete SGR mouse report. `SgrMouseFields::release` records
whether its terminator is `m` rather than `M`. The scanner checks the framing and
field count, leaving numeric admission to each caller.

The daemon's geometry parser requires exactly three nonempty ASCII u32 fields,
permits zero coordinates and rejects extra fields. The scanner itself retains
plus signs, empty fields and extra text without normalizing them.
`parse_host_mouse` applies the host event domain over that scanner: u8 button
values, positive u16 coordinates converted to zero-based cells, and admitted
button classes. It accepts extra fields. Its `HostMouse` result retains the
button byte and release terminator for projection without a wire dependency.
