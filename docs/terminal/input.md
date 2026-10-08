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
fragment released by `expire` and the empty text an empty input produces. `Paste`
holds the text between the paste markers, without them. A caller that passes input on
as raw text puts the markers `ESC [ 200 ~` and `ESC [ 201 ~` back around a pasted
text. Nothing is trimmed or normalized: whitespace, control characters and U+FEFF are
`Data` like any other character. A character is never split, so an emoji is one
event.

## Sequences

A character other than `ESC` is a unit of its own. After `ESC`:

- `[` opens a control sequence ending at the first character from `@` to `~`. After
  `ESC [ <` a mouse report must have three nonempty decimal fields and end at `M` or
  `m`; a report of any other shape never ends and is released by its deadline. After
  `ESC [ M` the legacy mouse report ends after three more characters;
- `]` opens an operating system command, ending at the first bell or `ESC \`;
- `P` and `_` open a device control string and an application command, each ending
  at the first `ESC \`;
- `O` takes one more character; any other character completes the sequence at once,
  as in an alt-modified key.

A sequence that cannot end yet is kept as the fragment, which `get_buffer` returns.
The next chunk is appended to it and the whole is framed again.

## Deadlines

A fragment arms a deadline of `StdinBufferOptions::timeout` after the instant given
to `process`, 10 ms by default. The deadline is `None` when that sum is not an
`Instant`. Every `process` call cancels the pending deadline and arms a new one when
a fragment remains, so a fragment expires `timeout` after the last input, even if
that input was empty. `process` never expires a due deadline itself: the caller
decides the order by calling `expire` first.

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
start marker split across chunks is a fragment like any other and expires with its
deadline. The paste ends at the first end marker; a start marker inside it is part of
the text. A fragment kept before the start marker is delivered as `Data` ahead of the
`Paste`.
Empty input while a paste is open produces nothing. `get_buffer` and `flush` never
return the text of a paste in progress, and `clear` discards it.
