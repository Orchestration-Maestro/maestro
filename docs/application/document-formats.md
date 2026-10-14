# Spreadsheet value primitives

The application library exposes `maestro_app::browser::ssf` for scalar spreadsheet
formatting. These operations have no I/O and do not render a workbook preview.

- `init_table` overwrites built-in format IDs in the supplied table, retains other
  entries and returns that table. `default_map` and `default_str` independently
  look up defined fallback IDs and currency/accounting patterns; holes return `None`.
- `general` accepts borrowed text, booleans, numbers and empty values. Text passes
  through unchanged; booleans become `TRUE` or `FALSE`; empty values produce empty
  text. Numbers use General's width, significant precision and exponent rules.
  NaN produces `#NUM!`; either infinity produces `#DIV/0!`.
- `frac` returns `[whole, numerator, denominator]` using bounded convergents.
  Format writers supply positive denominator limits. Mixed negative values use
  floor division rather than truncation toward zero.
- `parse_date_code` accepts a numeric serial and the effective 1904-epoch flag.
  Nonfinite values and serials outside 0 through 2,958,465 return `None` before
  normalization or epoch selection. The returned `DateCode` retains the normalized
  serial day before the epoch offset, integral seconds after carry, subsecond
  fraction and calendar components. Midnight carry increments the serial day.
  The standard epoch retains day zero and the synthetic February 29, 1900.
  Calendar conversion uses date-only arithmetic, not the host timezone.

- `write_date` renders one selected `DateToken` from a borrowed `DateCode` and
  caller-selected subsecond precision. It includes English month/weekday names,
  Buddhist years, 12/24-hour clocks and elapsed totals from the serial day.
  It does not reparse the serial, select AM/PM or apply date-wide carry.
- `write_num` renders one selected numeric pattern with a `NumberToken` selecting
  ordinary or accounting-parenthesis handling. Patterns support scale modifiers,
  padding, ASCII grouping, fractions, digit masks and scientific/engineering text.
  Grouped decimal rounding carries into the integer field; engineering rounding
  preserves displacement and mantissa carry. Decimal fields with `0`/`#` integer placeholders retain finite
  integral values without lossy scaling. Zero fixed denominators fail.
  `FormatError` distinguishes unsupported number patterns, invalid date fields,
  unclosed quoted literals, unclosed bracket blocks and unrecognized characters;
  callers need not parse its display text.
- `is_date` reports whether a pattern contains calendar or clock fields. It skips
  quoted/escaped literals and numeric runs, recognizes General case-insensitively,
  and detects calendar markers, meridiem and elapsed fields. This is tolerant
  recognition, not validation of syntax or field widths.

The [numeric/calendar library inventory](../libraries/spreadsheet-formats.md)
documents the underlying primitives. Custom format evaluation, mutable format
loading, date-valued General input and alternate calendars are not exposed by
this scalar interface. The internal selected-section evaluator reuses the scalar
writers for literals, numeric masks, fractions and clock/calendar fields. Its
private implementation is test-gated until the full-format consumer is wired;
it is not a public formatting operation. The internal alternate projection uses
a tabular Thursday-epoch calendar, preserving the standard serial sentinels.
Midnight carry retains both the effective epoch and the first captured calendar.
Meridiem labels use the rounded clock. General tokens resolve in source order;
spaces before literal digits join a fraction only after its denominator slash.
Numeric fields use the magnitude for negative values in multi-section formats;
a single-section format retains its sign, moving it into a leading literal when
needed. Scientific and engineering fields use the same selected value.

## Word package reader

`maestro_app::browser::docx_preview::WordDocument::load` opens supplied archive
bytes. It checks recognized XML parts before returning, including property and
comment parts; missing parts and unknown relationship types are tolerated. This
is package loading, not text extraction, semantic document parsing or rendering.

Parts retain decoded XML and ordered relationships. Relationship lookup borrows
stored parts. Missing part relationships fall back to root relationships while
retaining the supplied part folder; an empty list does not fall back. External
relationship paths retain their resolved absolute URLs. Resource lookup reads
bytes on demand without network requests or filesystem writes. XML entry decoding uses lossy UTF-8. DOCTYPE is rejected.

`parse_xml_string` provides borrowed element/attribute queries and descendant
text. Element selection uses immediate children and local names; attribute
selection uses the first ordinary attribute with that local name. Namespace
URIs are available separately. `Options::trim_xml_declaration` defaults to true;
standalone XML parsing takes this policy explicitly.

The same namespace exposes boolean and length conversion for property parsers.
See the [document-format library inventory](../libraries/document-formats.md)
for the native archive, XML and URL implementations.
