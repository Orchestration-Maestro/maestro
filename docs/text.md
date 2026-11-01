# Text

`maestro-text` is a core leaf below layer 0 with no workspace dependencies. It holds
shared text operations and a per-stream decoder; each caller keeps its own trimming, any-or-all semantics,
stable sorting and comparator lifetime.

- **Terminal controls.** `terminal::strip_ansi` removes matches of one cached terminal
  escape expression; `terminal::sanitize_binary_output` drops C0 controls except TAB,
  LF and CR, plus U+FFF9 to U+FFFB. Tools and opt-in Markdown content emission share
  these functions. `terminal::ansi_regex` exposes the same compiled grammar for
  streamed tool-output boundary detection.
- **Streaming UTF-8.** `Utf8Stream` decodes complete scalars, replaces invalid prefixes
  and holds an incomplete suffix until `push` or `finish`. It retains BOMs;
  terminal input, tool output and message framing keep their own BOM policy.
- **UTF-8 decoding.** `decode_utf8` removes one initial byte-order mark and uses
  lossy UTF-8 decoding for invalid bytes. Attachments and HTTP response readers
  share this decoder.
- **Owned UTF-8 decoding.** `lossy_string` takes owned bytes and keeps their buffer
  when they are valid; otherwise it decodes them lossily. It keeps a byte-order mark.
  Process output and Office package entries share it.
- **Environment flags.** `is_truthy_env_flag` accepts `1`, ASCII-case-insensitive
  `true` or `yes` without trimming; absent values are false. Tools and packages share it.
  `is_offline_selected` returns a caller's offline selection, or reads `MAESTRO_OFFLINE`
  through that rule when the selection is absent.
- **Whitespace.** `WHITESPACE` lists the 25 scalars once: Unicode white space except
  U+0085, plus U+FEFF. `is_whitespace_scalar` tests membership. HTTP edge
  whitespace and JSON token whitespace are different rules and stay with their owners.
  The model overflow pattern builds its character class from `WHITESPACE`.
- **Newlines.** `normalize_to_lf` converts every CRLF pair and lone CR to LF and touches
  nothing else. Frontmatter parsing, the editor's stored text, Markdown parsing and
  edit matching all call it; each keeps its own tab policy.
- **UTF-16 prefixes.** `truncate_utf16(text, limit)` returns the longest whole-scalar
  prefix within the unit budget. Models and tree previews share this helper, also
  re-exported from `maestro_app`.
- **Lexical order.** `utf16_order` compares UTF-16 code units, so supplementary
  scalars sort before U+E000.
- **Markup.** `markup::escape` and `markup::escape_into` replace `&`, `<`, `>` and `"`
  in one pass and never rescan emitted entities. `EscapeMode` picks the apostrophe
  spelling: `Xml` writes `&apos;`, `HtmlDecimal` and `HtmlHex` write numeric references; `HtmlDecimalPadded`
  writes `&#039;` for literal HTML export. `XmlAttribute` retains XML attribute
  tabs/newlines as character references. `SpreadsheetHtml` uses named apostrophes,
  LF break tags and four-digit lowercase hexadecimal references for other C0 controls.
- **CSS strings.** `css::string` quotes text, escapes quotation marks and backslashes,
  replaces NUL with U+FFFD and hex-escapes C0 controls, DEL and `<`. Brand font
  stacks and invalid document font names share it. `css::escape_control` supplies
  the same control escaping for brand property identifiers.
- **Collation** (feature `collation`). `collation::Collator::new` reads the first
  present of `LC_ALL`, `LC_MESSAGES` and `LANG` through the supplied reader, empty
  values included, and none means `en_US`. Encoding and modifier suffixes are
  dropped, `C` and `POSIX` mean `en-US`, an empty name means `und`, and text that is
  not a locale selects the library default. The collator keeps the chosen locale,
  never the reader. `maestro_theme::Collator` re-exports it outside browser targets.

## Numbers

The optional `number-format` feature exports `number_format::LocaleNumberFormat`.
Its constructor uses the same locale selection as the collator above, retaining
neither the reader nor later environment changes. `format(f64)` returns locale
grouping and digits, zero to three fractional digits, half-expand rounding and
signed zero. It accepts finite values and returns an I/O error for conversion
failures. The application re-exports this formatter for transcript summaries.

```rust
use maestro_text::{markup::{EscapeMode, escape}, is_whitespace_scalar, utf16_order};
use std::cmp::Ordering;

assert!(is_whitespace_scalar('\u{feff}') && !is_whitespace_scalar('\u{85}'));
assert_eq!(utf16_order("\u{1f600}", "\u{e000}"), Ordering::Less);
assert_eq!(escape("'", EscapeMode::HtmlDecimal).len(), 5);
```

## Local dates and times

The optional `date-time-format` feature exports
`date_time_format::LocaleDateTimeFormat`. It captures the same locale observations
as number formatting and accepts Gregorian local `[year, month, day]` and
`[hour, minute, second]` fields. The caller supplies the clock and time zone.
Native callers can use the re-export `maestro_theme::LocaleDateTimeFormat`.
The extension helper keeps its existing input and output; browser surfaces
continue to use browser formatting.

The formatter uses one numeric date-plus-time profile with Gregorian, Buddhist,
or Persian default calendars. It preserves local digits, two-digit widths,
day-period text, punctuation and directional marks. It rejects invalid fields.
It does not accept arbitrary patterns, calendar overrides or time-zone options.

An exact locale row wins. Otherwise the locked locale fallback engine walks
language and script parents while retaining the original territory's hour cycle
and calendar through representative rows. Unsupported languages use a root
recipe, not en-US. A missing environment, `C`, or `POSIX` still selects en-US;
an empty selected value selects `und`. For example, `fr-CA` uses
`2026-10-27 09 h 25 min 07 s`, while an unknown language in Iran uses a Persian
root date with a twenty-four-hour clock.

Committed data derives from CLDR JSON 48.2.0, under Unicode-3.0. The
[asset notes](../crates/maestro-text/data/date-time/README.md) document the schema,
pinned archive hashes, recorded pattern choices, licence, and the Rust
regeneration/check commands. The generator verifies the recorded JSON inputs
before writing any asset. Builds and data generation need neither Node nor
network access. Output follows that pinned data version; other platform releases can
choose different defaults. Arbitrary locale extensions and combinations outside
the measured regional closure are not a claim of full platform equivalence.

Dependencies are listed in [text libraries](libraries/text.md).
