# Styled terminal text

`maestro-tui` measures, wraps, truncates and slices text that carries terminal
escapes. A grapheme cluster with an escape written inside it is one grapheme in
every operation.

## Escapes

A recognized escape is a CSI sequence (`ESC [` up to and including the first `m`,
`G`, `K`, `H` or `J`) or an OSC or APC string (`ESC ]` or `ESC _` up to and
including the first BEL or `ESC \`). Any other text, including an escape with no
terminator, is visible text. A recognized escape takes no cells.

- A CSI sequence that ends in `m` is a style escape when only digits and semicolons
  lie between its last `ESC [` and that `m`; they are its parameters.
- An OSC 8 string (`ESC ] 8 ; params ; uri`) opens a hyperlink, or closes the open
  one when `uri` is empty.
- Every other recognized escape is metadata, such as a prompt marker, the cursor
  marker or an erase sequence.

A style escape applies its parameters one by one, left to right. `0`, and an empty
list, reset attributes and colours but leave the hyperlink open. `1` to `5`, `7`, `8`
and `9` turn on bold, dim, italic, underline, blink, inverse, hidden and
strikethrough. `21` turns off bold, `22` bold and dim, and `23` to `25` and `27` to
`29` turn off italic, underline, blink, inverse, hidden and strikethrough. `30` to
`37` and `90` to `97` set the foreground, `40` to `47` and `100` to `107` the
background, and `39` and `49` clear them. `38` or `48` followed by `5;n` or
`2;r;g;b` sets an indexed or true colour. Any other parameter changes nothing.

Where an operation re-establishes the style in effect, which is at the start of a
line that follows another line from `wrap_text_with_ansi` and of the part after an
overlay from `extract_segments`, it writes one `ESC [ … m` with the active
attributes in the order bold, dim, italic, underline, blink, inverse, hidden,
strikethrough, then the foreground, then the background. Indexed and true colours
keep their parameters as written and the other colours are written as plain
decimals. The open hyperlink follows, with the parameters and terminator of its
opener. A style with nothing active writes nothing.

## Measuring

`visible_width(text)` returns the terminal columns of `text`. Recognized escapes
take none and a tab takes three. A grapheme cluster is measured as a whole: wide
and fullwidth characters, complete emoji sequences and regional indicators, alone
or in flags, take two columns, and a cluster made only of marks, ignorable characters
and control characters other than the tab takes none.

`get_segmenter(text)` yields each grapheme cluster with its UTF-8 byte position.
`extract_ansi_code(text, pos)` returns the recognized escape that starts at byte
`pos` with its byte length, or `None`. `normalize_terminal_output(text)` rewrites
the Thai and Lao AM vowels (`U+0E33`, `U+0EB3`) as the pairs `U+0E4D U+0E32` and
`U+0ECD U+0EB2`, and returns text without them borrowed.

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

`wrap_text_with_ansi(text, width)` returns the lines of `text` without padding.

- Lines break at ASCII spaces and fill greedily. A word wider than `width` breaks
  between whole graphemes, and a non-whitespace grapheme wider than `width` is kept
  whole.
- Text is split at literal newlines before escapes are read, so no escape reaches
  across a line feed. Empty text, and an empty literal line, return an empty line.
- A literal line that measures at most `width` is neither broken nor trimmed. A
  wider one is broken, every line it breaks into loses its trailing whitespace, and
  a line left empty by that is dropped.
- A literal line left with no graphemes returns one line holding only its metadata
  escapes; style escapes alone give an empty line.
- A line that has text starts with the style in effect at its first grapheme, also
  after a literal newline.
- A line that has text closes the hyperlink still open at its end, with the
  terminator its opener used. It closes underline only when underline is active and
  another line of the same literal line follows. It closes no other attribute or
  colour.

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

`truncate_to_width(text, max_width, options)` returns `""` when `max_width` is
zero. The ellipsis defaults to `...`.

- Text that measures at most `max_width` is returned without the ellipsis, even
  when the ellipsis is wider than `max_width`. Its only changes are that a
  hyperlink still open at its end is closed and that a text with no visible
  characters keeps only its metadata escapes.
- Longer text is cut. When the ellipsis is narrower than `max_width`, the result is
  a prefix of whole graphemes followed by the ellipsis. The prefix takes graphemes
  from the start while their cells total at most `max_width` minus the cells of the
  ellipsis, and stops at the first one that does not fit. A full reset follows the
  prefix and another follows the ellipsis; a hyperlink open at the end of either is
  closed first. A result with no prefix and no ellipsis is empty.
- The result is measured as emitted, because the prefix and the ellipsis can join
  into a grapheme of another width, such as a flag from two halves or a base with a
  variation selector. When joining makes the result wider than `max_width`, the
  prefix loses graphemes until it fits.
- When the text is cut and the ellipsis alone is as wide as `max_width` or wider,
  only the ellipsis is returned, framed by resets and cut to the whole graphemes
  that fit in `max_width`; it is empty when its first grapheme does not fit.
- With `pad`, spaces are added until the finished result measures `max_width`.

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

`slice_with_width(line, start_col, length, strict)` returns the graphemes that
start in the columns `start_col..start_col + length` and the cells they occupy;
`slice_by_column` returns only their text. A grapheme that starts left of
`start_col` is left out, and one that crosses the right edge is kept, or left out
when `strict` is set. A non-empty selection starts with the escapes written before
its first grapheme, in their original order, and closes a hyperlink still open at
its end. A tab takes no columns here, unlike in `visible_width`. A length of zero,
or a range that starts beyond the end of the line, selects nothing.

`extract_segments(line, before_end, after_start, after_len, strict_after)` returns
the graphemes that start before `before_end` and those that start in
`after_start..after_start + after_len`, each part with its cells. Where the ranges
overlap, the part before keeps the graphemes. The part after starts with the style
in effect at its first grapheme, and `strict_after` leaves out a grapheme that
crosses its right edge. Metadata escapes stay in place in either part: between its
graphemes, in front of its first grapheme and at its end when the line ends inside
it. Those under the overlay are dropped.

`apply_background_to_line(line, width, bg_fn)` pads `line` with spaces up to
`width` columns, never truncates it, and returns the result of calling `bg_fn` once
on the padded text.

```rust
use maestro_tui::{apply_background_to_line, extract_segments};

let parts = extract_segments("\x1b[31mab\x1b[1;44mcd\x1b[39mef", 1, 4, 2, false);
assert_eq!((parts.before.as_str(), parts.before_width), ("\x1b[31ma", 1));
assert_eq!((parts.after.as_str(), parts.after_width), ("\x1b[1;44mef", 2));

let framed = apply_background_to_line("hi", 5, |padded| format!("<{padded}>"));
assert_eq!(framed, "<hi   >");
```

## Classifying characters

`is_whitespace_char(text)` and `is_punctuation_char(text)` are true when at least
one scalar of `text` matches and false for the empty string. Whitespace is the set
of the ECMAScript definition, which includes the byte-order mark and the no-break
space and excludes U+0085 and U+200B. Punctuation is any of
``(){}[]<>.,;:'"!?+-=*/\|&%^$#@~` ``.
