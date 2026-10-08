# Styled terminal text

`maestro-tui` measures, wraps, truncates and slices text that carries terminal
escapes. Every operation reads the text once into its visible characters and the
escapes written between them, so they agree on which graphemes exist and which
style applies to each.

## Supported escapes

Only these sequences are recognized; anything else is ordinary text:

- CSI sequences up to the first `m`, `G`, `K`, `H` or `J`. Those ending in `m`
  that carry only digits and semicolons change colours and attributes.
- OSC and APC strings ended by BEL or by `ESC \`. OSC 8 opens and closes hyperlinks;
  other strings, such as prompt markers and the cursor marker, are metadata.

An escape with no terminator stays in the text as visible characters. Escapes keep
their exact spelling, including padded parameters and the BEL or `ESC \` ending of
a hyperlink. This is not a terminal emulator: nothing is sanitized or reordered.

## Measuring

`visible_width` counts terminal columns. Escapes take none, a tab takes three, and
a grapheme cluster is measured as a whole: wide and fullwidth characters, complete
emoji sequences and regional indicators take two, combining marks and other
ignorable scalars take none. A lone regional indicator takes two so that a flag
that is still streaming does not shift the line when its second half arrives.
Widths of printable ASCII bypass segmentation, and other strings are cached by
their exact text, 512 at a time, evicting the oldest.

`get_segmenter` yields grapheme clusters with their UTF-8 byte positions.
`extract_ansi_code` returns the escape that starts at a byte position, if any.
`normalize_terminal_output` rewrites the Thai and Lao AM vowels as the pairs that
terminals repaint reliably; width is unchanged and other text is borrowed as is.

```rust
use maestro_tui::{extract_ansi_code, get_segmenter, visible_width};

assert_eq!(visible_width("\x1b[31mHello\x1b[0m"), 5);
assert_eq!(visible_width("\t\x1b[31m\u{754c}\x1b[0m"), 5);
assert_eq!(visible_width("\u{1f1e8}"), 2);

let segments: Vec<_> = get_segmenter("a\u{1f642}e\u{301}").collect();
assert_eq!(segments, [(0, "a"), (1, "\u{1f642}"), (5, "e\u{301}")]);

let found = extract_ansi_code("x\x1b[1;4m", 1).expect("a style escape starts at byte 1");
assert_eq!((found.code, found.length), ("\x1b[1;4m", 6));
```

## Wrapping

`wrap_text_with_ansi(text, width)` breaks at ASCII spaces, greedily, and returns
lines without padding. Whitespace that fits stays; whitespace that would overflow
is dropped, and trailing whitespace of a wrapped line is trimmed. Literal newlines
keep their structure, and an intentionally empty line stays empty. A word wider
than the width breaks between whole graphemes. A single grapheme wider than the
width, such as a double-width character at width one, is kept alone on its line
rather than clipped or dropped.

Each continuation line starts with the style in effect at its first character and
each wrapped line closes underline and any hyperlink, but not other attributes, so
backgrounds survive across lines. A full reset does not close a hyperlink. The
close uses the terminator the link was opened with. The last line of a text ends
without a reset, except for a hyperlink that is still open, which is closed too.
Style escapes attached to whitespace that wrapping drops are folded into the style
the next line starts with; metadata escapes such as markers are never dropped.

```rust
use maestro_tui::wrap_text_with_ansi;

let lines = wrap_text_with_ansi("This is a long line that needs wrapping", 20);
assert_eq!(lines, ["This is a long line", "that needs wrapping"]);

let red = wrap_text_with_ansi("\x1b[31mhello world\x1b[0m", 7);
assert_eq!(red, ["\x1b[31mhello", "\x1b[31mworld\x1b[0m"]);

let link = wrap_text_with_ansi("\x1b]8;;https://example.com\x07click here\x1b]8;;\x07", 6);
assert_eq!(
    link,
    [
        "\x1b]8;;https://example.com\x07click\x1b]8;;\x07",
        "\x1b]8;;https://example.com\x07here\x1b]8;;\x07",
    ]
);
```

## Truncating

`truncate_to_width(text, max_width, options)` keeps the longest prefix of whole
graphemes that fits beside the ellipsis, which defaults to `...`. When something
was cut, the prefix, the ellipsis and the end of the result are separated by full
resets, and a hyperlink in the prefix or the ellipsis is closed before its reset.
Text that already fits is returned unchanged, even if the ellipsis would be wider
than the limit. An ellipsis wider than the limit is clipped between graphemes. A
result with no kept text and no ellipsis is empty, never a lone reset. With
`pad`, the result is filled with spaces to the limit.

```rust
use maestro_tui::{truncate_to_width, TruncateOptions};

let cut = truncate_to_width("Hello World", 8, TruncateOptions::default());
assert_eq!(cut, "Hello\x1b[0m...\x1b[0m");

let bare = TruncateOptions { ellipsis: "", pad: false };
assert_eq!(truncate_to_width("Hello World", 8, bare), "Hello Wo\x1b[0m");

let padded = TruncateOptions { ellipsis: "\u{2026}", pad: true };
let wide = "\u{754c}\u{754c}\u{754c}\u{754c}";
assert_eq!(truncate_to_width(wide, 6, padded), "\u{754c}\u{754c}\x1b[0m\u{2026}\x1b[0m ");
assert_eq!(truncate_to_width("anything", 0, TruncateOptions::default()), "");
```

## Selecting columns

`slice_by_column` and `slice_with_width` return the graphemes that start inside a
column range, with the width they actually use. A grapheme that crosses the left
edge is omitted; one that crosses the right edge is kept, or omitted in strict
mode. Escapes that precede the range are carried in front of the first selected
grapheme in their original order, and an open hyperlink is closed at the end of
the selection. Columns count graphemes only: a tab here occupies no columns,
unlike in `visible_width`, and offsets beyond the text select nothing.

`extract_segments` takes the text before an overlay and the text after it in one
pass; the part before wins where they overlap. The part after begins with the
style that was in effect at its first grapheme. `apply_background_to_line` pads a
line to a width and hands it to a caller-supplied function exactly once; it never
truncates.

```rust
use maestro_tui::{apply_background_to_line, extract_segments, slice_by_column};

assert_eq!(slice_by_column("a\u{754c}b", 0, 2, false), "a\u{754c}");
assert_eq!(slice_by_column("a\u{754c}b", 0, 2, true), "a");

let parts = extract_segments("\x1b[31mab\x1b[1;44mcd\x1b[39mef", 1, 4, 2, false);
assert_eq!((parts.before.as_str(), parts.before_width), ("\x1b[31ma", 1));
assert_eq!((parts.after.as_str(), parts.after_width), ("\x1b[1;44mef", 2));

let framed = apply_background_to_line("hi", 5, |padded| format!("<{padded}>"));
assert_eq!(framed, "<hi   >");
```

## Classifying characters

`is_whitespace_char` and `is_punctuation_char` answer for any string: they are
true when at least one scalar matches, and false for the empty string.
Whitespace follows the ECMAScript definition, which includes the byte-order mark
and the no-break space and excludes U+0085 and U+200B.
