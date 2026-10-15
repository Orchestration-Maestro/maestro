# Spreadsheet values and workbooks

The application library exposes `maestro_app::browser::ssf` for scalar spreadsheet
formatting and in-memory XLSX reading. These operations do not render a workbook
preview or write files.

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
  unclosed quoted literals, unclosed bracket blocks, excess sections and unrecognized characters;
  callers need not parse its display text.
- `is_date` reports whether a pattern contains calendar or clock fields. It skips
  quoted/escaped literals and numeric runs, recognizes General case-insensitively,
  and detects calendar markers, meridiem and elapsed fields. This is tolerant
  recognition, not validation of syntax or field widths.

The [numeric/calendar library inventory](../libraries/spreadsheet-formats.md)
documents the underlying primitives. `read(bytes, name, &mut ssf)` uses the same
selected-section evaluator for workbook number, currency, percentage, scientific,
fraction, date and custom conditional formats. Full standalone formatting options,
date-valued SDK inputs and legacy XLS reading are not yet exposed.

`Ssf::get_table` returns a shared mutable format table. Reading a workbook replaces
the current table slot without changing earlier handles; `WorkBook::ssf` is a
detached snapshot. `WorkBook` preserves sheet order and the date epoch; each
`WorkSheet` retains ordered populated cells, its range and merged regions.
Blank sheets retain declared rectangular ranges; empty singleton defaults are omitted.
Retained error cells load implicit formats into the live table and workbook snapshot.
Cells retain raw typed values and optional formatted text. Raw string-cell escapes
are decoded once; shared and inline strings are not decoded again. Decoded text
normalizes CRLF to LF at the common cell boundary. `format_cell` respects
present caches, including empty text, and otherwise uses the active default
format with unrounded raw-scalar fallback on formatting failure, leaving the cache
absent. Unknown error labels survive
native conversion, and no-cache formula cells are omitted without evaluation.
Malformed archives, required missing parts, XML, reader and address failures have
distinct `WorkbookError` variants. Missing style declarations are optional, while
missing declared style parts and malformed style XML fail with workbook/part
context. ISO dates and timezone-free datetimes use UTC serial conversion with the
real March 1900 boundary. Time-only cells attach to the workbook epoch date and
use the same millisecond serial conversion; noon is 0.5 in either epoch. Numeric
serial 60 retains the spreadsheet's synthetic leap day.
`encode_row`, `encode_col`, `encode_cell` and `encode_range` render zero-based
positions as A1 addresses; decoders accept absolute `$` markers.

The internal evaluator The internal alternate projection uses
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

## Owned Word element tree

`maestro_app::browser::docx_preview::{parse_body_elements, parse_paragraph,
parse_run}` parse an `OwnedXmlElement` selection into an `OpenXmlElementTree`.
Ordered arena positions preserve separate runs and their paragraph or simple-field
parents, including hidden paragraphs used by content controls and change wrappers.
Text, breaks, tabs, symbols, note references and field metadata remain typed;
paragraph-level simple fields retain their result runs without evaluating fields.
Field locks read `fldLock`, falling back to `lock` only when `fldLock` is absent;
invalid or missing flag values default to false independently of `dirty`.
Symbol font-family values preserve valid CSS spellings and quote invalid names
through the [shared CSS string serializer](../text.md), including escaping `<`.

Raw property elements and recognized foreign branches retain shared complete XML
source with selected byte ranges, preserving ancestor namespaces. These slots do
not yet interpret formatting, tables, links, bookmarks, drawings, math or comments.
The tree is a partial semantic parser, not a rendered preview or attachment-text
extractor. Package loading remains separate from element parsing.

## Word background and borders

`maestro_app::browser::docx_preview::{parse_background, parse_border_properties}`
consume borrowed XML selections. Background parsing returns a new ordered style
map. Border parsing updates the supplied map from immediate top, bottom, left and
right children; start/end alias left/right. Replacing a side retains its insertion
position and leaves unrelated entries untouched.

Both operations share color selection: nonempty literal colors precede theme
fallbacks, auto is black, document color names use their defined RGB values and
none is transparent. Border styles default to solid; nil/none produces only none.
Absent width/color components are omitted, allowing CSS defaults. Widths use the
existing length conversion. These operations do not wire formatting into the
owned element tree or render a document.

## Word default properties and conditional table styles

`maestro_app::browser::docx_preview::parse_default_properties` updates supplied
ordered CSS maps from immediate property children. It handles alignment, colors,
font size and families, underline, indentation, paragraph spacing, cell margins,
row height and table layout. The optional child map receives table borders and
cell margins; paragraph and cell borders stay in the main map. Replacements keep
their original key positions, including nullable values.

`DefaultPropertiesOptions` defaults to no width suppression and no interceptor.
Its `ignore_width` suppresses cell widths only. The borrowed synchronous handler
sees and may mutate the main map before each child's dispatch; returning true
consumes that child. Font parsing selects ASCII, ASCII theme and East Asian
families, using the existing font enclosure. Position writes the valid
`vertical-align` CSS key.

`parse_table_style` prepares owned `TableStyle` selector/modifier/value records
for first/last rows and columns and horizontal/vertical banding. Property-child
order, duplicates and empty maps are retained. Unsupported regions return no
records. These parsers do not integrate formatting into the owned element tree
or render a preview.
