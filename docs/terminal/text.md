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

An escape with no terminator stays in the text as visible characters. Escapes are
written out with their exact spelling, including padded parameters and the BEL or
`ESC \` ending of a hyperlink. Only the style that a continuation line or the part
after an overlay starts from is rebuilt: attributes in a fixed order, then the
foreground and the background, with indexed and true-colour parameters kept
verbatim. This is not a terminal emulator: nothing is sanitized.

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

let segments: Vec<_> = get_segmenter("a\u{1f642}e\u{301}").collect();
assert_eq!(segments, [(0, "a"), (1, "\u{1f642}"), (5, "e\u{301}")]);

let found = extract_ansi_code("x\x1b[1;4m", 1).expect("a style escape starts at byte 1");
assert_eq!((found.code, found.length), ("\x1b[1;4m", 6));
```

## Wrapping

`wrap_text_with_ansi(text, width)` breaks at ASCII spaces, greedily, and returns
lines without padding. A literal line that fits the width is returned as it is,
whitespace included. One that overflows loses the whitespace that would overflow
and the trailing whitespace of each wrapped line, and a line left empty by that
is dropped. Literal newlines keep their structure, and an intentionally empty
line stays empty. A word wider than the width breaks between whole graphemes. A
single grapheme wider than the width, such as a double-width character at width
one, is kept alone on its line rather than clipped or dropped.

Each continuation line starts with the style in effect at its first character.
A line that another line of the same literal line follows closes underline; the
last line of a literal line leaves underline as the text left it. Every line closes a hyperlink that is still open at its end, with
the terminator the link was opened with, and a full reset does not close a
hyperlink. Colours and other attributes are not closed at a line end and each
continuation restores them, so backgrounds survive across lines. Style escapes
attached to whitespace that wrapping drops are folded into the style the next line
starts with. Wrapping never drops metadata escapes such as markers: metadata of
whitespace dropped at a break moves to the end of the previous line, and metadata of
whitespace dropped before the first line's content moves to the start of that line.

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
Text that already fits is kept whole, even if the ellipsis would be wider than the
limit; the only changes are that a hyperlink still open at its end is closed and
that text with no visible characters keeps just its metadata escapes, so style
escapes alone give an empty result. An ellipsis as wide as the limit or wider
replaces the text and is clipped between graphemes; a clipped ellipsis whose
graphemes take no cells, such as a combining mark, is still kept and framed. A
result with no kept text and no ellipsis is empty, never a lone reset. With `pad`,
spaces fill what the result measures short of the limit. A prefix and an ellipsis
can join into one grapheme, such as the two halves of a flag or a base followed by
a variation selector. The result is measured as emitted: when joining makes it
wider than the limit, the prefix loses graphemes until it fits.

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
style that was in effect at its first grapheme. Metadata escapes such as the cursor
marker stay in either part: between its graphemes, in front of the first grapheme
after, and at the end of either part when the line ends inside it; metadata under
the overlay is dropped. `apply_background_to_line` pads a line to a width and
hands it to a caller-supplied function exactly once; it never truncates.

```rust
use maestro_tui::{apply_background_to_line, extract_segments};

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
