# Spreadsheet values and workbooks

The application library exposes `maestro_app::browser::ssf` for scalar spreadsheet
formatting, in-memory XLSX/XLS reading and HTML table projection. These operations
prepare owned data and markup; they do not mount a native preview or write files.

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
fraction, date and custom conditional formats.

`Ssf::format` accepts `NumberFormat::Code` (including empty codes) or
`NumberFormat::Id`, a borrowed `CellValue` and `FormatOptions`. Supplied tables
replace direct active-table lookup; mapped fallback can still use the active
table. Nonempty `date_nf` replaces exact `m/d/yy` or ID14. Owned UTC
`CellValue::Date` inputs reuse the real-date serial conversion; General dispatch
uses ID14 with the same table, date override and epoch options.

`Ssf::load` overwrites an explicit ID or deduplicates before choosing the first
hole in 0..392; a full range replaces391. `load_table` copies slots through391,
including empty codes, and leaves the same-owner table unchanged.
`Ssf::get_table` returns a shared mutable format table. Reading XLSX replaces
the current table slot without changing earlier handles; XLS retains that slot.
Both readers repair the producer date code `d.m` to `d\.m` before loading it;
direct `Ssf::load` calls preserve the supplied code.
`WorkBook::ssf` is a detached snapshot. `WorkBook` preserves sheet order and the date epoch; each
`WorkSheet` retains ordered populated cells, its range and merged regions.
`WorkBook::sheets` stores a `Result<WorkSheet, WorkbookError>` for each named sheet.
An XLSX sheet decoding failure retains its typed error and discards partial cells,
while readable siblings remain available. A successfully decoded blank sheet counts
as readable. If all declared sheets fail, reading returns the first sheet's error;
a zero-sheet workbook remains empty. Binary XLS stream failures still fail the
whole workbook. Archive, workbook metadata, shared strings/styles and malformed
relationship XML failures remain workbook-level errors.

Workbook relationships are indexed once by exact, case-sensitive identity; the
last duplicate wins. A missing target fails the whole workbook, even when unused.
A selected empty target is a sheet error. Missing, empty or unresolved sheet
identities select positional paths for the whole workbook, using `sheet.xml`,
`sheet1.xml`, … when the zero-based first part exists. Document and hyperlink
relationship resolution is unchanged.

Spreadsheet attachment extraction keeps each sheet's name, order and indexed
wrapper. A failed sheet in a mixed XLSX workbook contributes an empty CSV body,
not a diagnostic; valid siblings contribute their formatted CSV. All-corrupt
workbooks remain extraction errors.

In XLSX, blank sheets retain declared rectangular ranges; empty singleton defaults
are omitted. Retained XLSX error cells load implicit formats into the live table and
workbook snapshot. Cells retain raw typed values and optional formatted text.
Raw string-cell escapes and format-code attributes decode once after XML entity
decoding; shared and inline strings are not decoded again. XLSX text normalizes
CRLF to LF; XLS preserves raw text.

`format_cell(cell, value, options, ssf)` respects present caches, including empty
text, before the optional non-null value override. It tries `z` before the XF
format ID and caches only successful formatting. Date-classified cells can
prepare `z` from `CellFormatOptions::date_nf`; standalone table/epoch options
are not accepted by this helper. Raw values remain unchanged. If both formats
fail, fallback uses unrounded scalar text without caching: booleans use `true` or
`false`, null uses `null`, and a missing value uses `undefined`. UTC Dates use
readable RFC3339 text with milliseconds, not host-dependent date spelling; HTML
raw-value attributes reuse this spelling. Standard error
codes use their labels; an unknown numeric error stays numeric. Unknown XLSX error labels
are kept, and formulas are not evaluated.
Cells retain owned `f` formula text and optional `array_formula` range metadata.
XLSX formula text decodes spreadsheet escapes once, normalizes CRLF to LF and removes
function prefixes. Shared formulas expand within their own sheet using the
[operand rules](#shared-formula-operands). Uncached XLSX numeric formulas retain missing raw values
rather than fabricated zeroes; explicit string cells without a value retain empty strings.
Malformed archives, required missing parts, XML, reader and address failures have
distinct `WorkbookError` variants. Missing style declarations are optional, while
missing declared style parts and malformed style XML fail with workbook/part
context. ISO dates and timezone-free datetimes use UTC serial conversion with the
real March 1900 boundary. Time-only cells attach to the workbook epoch date and
use the same millisecond serial conversion; noon is 0.5 in either epoch. Numeric
serial 60 retains the spreadsheet's synthetic leap day.
XLS selection uses the complete eight-byte compound-file header, not the supplied
filename. The legacy reader preserves sheet order, record-ordered cells, merges,
epoch and raw scalar values. Each cell's resolved number format selects display
text through the shared formatter; General bypasses active format0. Missing XFs
remain uncached except NUMBER's default-format branch. Failed formats remain
uncached for public fallback. FORMAT records load in encounter order into the live
table, including duplicate, unused and high identifiers; later records win.
Declared exclusive dimensions include blank rows and columns without materializing
blank cells, and populated cells expand those bounds. Without a declaration,
populated bounds start at A1; empty sheets have no range. For empty sheets, singleton declarations
retain their range, while zero-sized declarations have no range.
The owned container reader accepts version-3 512-byte and version-4 4096-byte
sectors, allocation extension sectors and fragmented regular/mini streams. It
selects only root-level streams, preferring `Workbook` to `Book`, with
ASCII case-insensitive names. Invalid headers, missing streams, encrypted files
and truncated records return a contextual `WorkbookError::Reader`.
The record reader joins continuation pieces, retaining string-width transitions
and surrogate pairs while skipping rich-run payloads. Shared-string termination
checks do not rescan the remaining continuation pieces. Declared code pages,
including producer aliases, decode older strings; empty byte strings need no
Unicode width flag. Embedded chart substreams do
not replace worksheet cells; chart sheets shift repeated NUMBER addresses one
column right. Sheets follow ascending stream positions. Boolean records and
formula caches are true only for byte 1. A pending formula string consumes its
following STRING record; a STRING without a pending formula fails.

XLSX values come from the same ZIP/XML pass as metadata. Numeric cells use decimal
prefix parsing, including incomplete exponents, signed infinity and signed zero;
no numeric prefix produces NaN. Missing or empty numeric/shared values stay absent.
Boolean tokens `1` and `true` alone are true; absent values are false. Inline
strings without text retain an empty string. Plain shared and inline strings
select direct text before direct rich runs, skip phonetic runs and decode workbook
escapes once. Invalid shared-string indices fail with workbook and sheet context.

Binary expressions retain their token order, independent absolute axes and
version-specific reference layouts. Relative rows use the version's negative-offset
wrap rule; displaced areas retain whole-row and whole-column spelling. Expression
composition retains operand handles and qualifier splits until final text emission.
Qualifier comparison uses composable case-fold summaries as a rejection filter,
with byte-for-byte verification of matches.
Counted older external-sheet records inside BIFF8 do not prevent ordinary cells
from reading, but provide no usable sheet selection.
Shared expressions expand per consuming cell;
array anchors retain their expression and range while followers retain only the
range. Template ownership survives a following STRING cache, including a shared
anchor that is the first populated cell. Number, Boolean, error and text caches
remain unchanged; a writer-omitted binary cache is empty text, not a missing value.
Binary strings retain CRLF and literal escape-looking text. Rich strings retain
decoded plain text without generated run markup. Hyperlink records retain their
selected target on existing cells in record order; unused tooltip and run metadata
are not stored. The existing HTML writer owns escaping and link safety. Each
sheet's observed bounds are independent when no dimension is declared. Unsupported
binary formula grammar returns a contextual reader failure; formulas are not evaluated.

`encode_row`, `encode_col`, `encode_cell` and `encode_range` render zero-based
positions as A1 addresses; decoders accept absolute `$` markers.

The internal alternate projection uses
a tabular Thursday-epoch calendar, preserving the standard serial sentinels.
Midnight carry retains both the effective epoch and the first captured calendar.
Meridiem labels use the rounded clock. General tokens resolve in source order;
spaces before literal digits join a fraction only after its denominator slash.
Numeric fields use the magnitude for negative values in multi-section formats;
a single-section format retains its sign, moving it into a leading literal when
needed. Scientific and engineering fields use the same selected value.

## Shared formula operands

The reader advances bounded A1 coordinates without evaluating formulas. Relative
axes advance by follower minus template position; `$` binds its own axis. Fixed
coordinates and zero applicable displacement retain their authored spelling.
Changed coordinates retain column case. Whole-column/whole-row pairs remain
unchanged, as do out-of-grid names such as `XFE1` and `A1048577`.

Coordinate matching classifies maximal runs between operator characters,
whitespace and operand punctuation. All other characters belong to a run, including
combining marks, `#`, `~`, `\`, `?`, `.`, `_` and `$`. Quotes with doubled escapes
and nested brackets with apostrophe escapes are opaque; an unterminated quote or
bracket protects the remainder. Runs starting with `#`, sheet qualifiers ending in
`!` and function names immediately followed by `(` are opaque, in that order.
An exact bounded coordinate advances, as does a coordinate with one trailing spill
`#`; other runs remain unchanged. A space before `(` remains an intersection.

The operand-blind displacement defect is corrected at this single reader owner:
opaque spans and the grammar's identifier class prevent coordinate-shaped text
inside names from being rewritten. Lowercase references retain case when advanced.
A colon qualifier `run:run!` is opaque unless immediately preceded by `!`:
`Q1:Q3!A1` protects the sheet range while `Sheet1!A1:Sheet1!B2` advances both cells.
The ambiguous `A1:Sheet2!B2` is read as a 3-D qualifier and protects `A1:Sheet2!`.
Input coordinate bounds do not constrain displaced output: `A1048576` moved down
becomes `A1048577`, and `XFD1` moved right becomes `XFE1`. Every advanced coordinate
uses the shared address owner. Formula
decoding, missing-cache retention and CSV quoting stay separate. R1C1 conversion
belongs to SpreadsheetML/SYLK readers, not this XLSX path; identifier boundaries
and brackets protect XLSX `RC`, `R1C1` and bracketed spellings.

## Default CSV

`sheet_to_csv(&mut sheet, &ssf)` traverses the declared inclusive rectangle with
comma fields and LF rows. Missing ranges produce empty text. Blank rows, columns
and declared trailing extent remain; hidden rows/columns and merges do not change
traversal. There is no unconditional final LF.

Present raw values use `format_cell`, which may populate text caches; caches
without raw values do not manufacture CSV data. Missing/null values instead emit
`=` plus a retained scalar formula, unless a nonempty array range suppresses it.
Cached zero wins over formulas. Fields containing comma, quote or LF are quoted,
with embedded quotes doubled; lone CR and surrounding whitespace are retained.
The initial cell's formatted `ID` is quoted. No custom CSV options are exposed.

## Word package reader

`maestro_app::browser::docx_preview::WordDocument::load` opens supplied archive
bytes. It checks recognized XML parts before returning, including property and
comment parts; missing parts and unknown relationship types are tolerated. It
also prepares [metadata and comments](#word-metadata-and-comments) eagerly. This
is not complete document parsing, text extraction or rendering.

Parts retain decoded XML, ordered relationships and one optional typed `PartData`. Relationship lookup borrows
stored parts. Missing part relationships fall back to root relationships while
retaining the supplied part folder; an empty list does not fall back. External
relationship paths retain their resolved absolute URLs. Resource lookup reads
bytes on demand without network requests or filesystem writes. XML entry decoding uses lossy UTF-8. DOCTYPE is rejected.
Before inflation, the reader returns an archive error if an entry declares more
than 512 MiB expanded. The archive library rejects data that exceeds its declared
size. Word, presentation and spreadsheet attachment builders also reject
extracted output above 512 MiB with a document processing error. The reader has no
cumulative read quota. These bounds do not cover parsed document structures,
archive metadata or lossy UTF-8 expansion.

`WordDocument::document_body` reparses the retained main-part XML and returns the
[owned body tree](#owned-word-element-tree), or `None` when the package has no
main part or the main part has no `w:body` element. Loading parses the main part
once while it admits the package parts, and an extraction parses it again to
build the tree rather than rereading the archive.

`parse_xml_string` provides borrowed element/attribute queries and descendant
text. Element selection uses immediate children and local names; attribute
selection uses the first ordinary attribute with that local name. Namespace
URIs are available separately. `Options::trim_xml_declaration` defaults to true;
standalone XML parsing takes this policy explicitly.

The same namespace exposes boolean and length conversion for property parsers.
See the [document-format library inventory](../libraries/document-formats.md)
for the native archive, XML and URL implementations.

## Word metadata and comments

`CorePropsPart`, `ExtendedPropsPart` and `CustomPropsPart` prepare owned property
records through `parse_xml`. Core properties select title, description, subject,
creator, keywords, language, last modifier and revision. Extended properties
select template, application, company, application version and page, word,
character, line and paragraph counts. Text stays raw; selected numbers use
integer-prefix conversion, including NaN and negative zero. An empty core
revision does not erase an earlier value; an empty extended count becomes NaN.

Custom properties preserve source order, nullable names/format IDs, authored
qualified value-element names and descendant text without scalar conversion.
Preparation selects the first element, ignoring preceding whitespace, comments,
processing instructions and CDATA. A property with no value element fails.
Selection and decoding follow the [XML reader](#word-package-reader).

`CommentsPart` reuses the [owned body parser](#owned-word-element-tree) and its
width policy for each comment's content, retaining raw author, initials, date
and identifier.
`CommentsExtendedPart` retains paragraph/parent links and nullable exact-token
completion flags. Both expose source-order lists and last-key-wins maps sharing
`Arc<RwLock<_>>` records. Missing IDs use the key `null`; numeric keys enumerate
first in ascending order. Editing a record does not update its snapshot lookup
key, and external handles survive removal or document release.

`WordDocument::load` prepares these records even without rendering; comment
bodies keep their cell widths. It admits relationships first, XML second and
semantic data third, before traversing that part's descendants. Semantic failures return
`DocxError::Xml` with the part path; no partial document is returned. The four
named property/comment accessors borrow the latest designated data. Custom
properties are available through `parts()` and `PartData::CustomProps`;
unsupported semantic kinds retain `data: None`.

## Owned Word element tree

`maestro_app::browser::docx_preview::{parse_body_elements, parse_paragraph,
parse_run}` parse an `OwnedXmlElement` selection into an `OpenXmlElementTree`.
Their `ignore_width` flag suppresses cell widths in every property group; table
widths remain.
Ordered arena positions preserve separate runs and their parents: a paragraph or
simple field, or a hyperlink or smart tag inside one. Hidden paragraphs used by
content controls and change wrappers remain.
Text, breaks, tabs, symbols, note references and field metadata remain typed;
paragraph-level simple fields retain their result runs without evaluating fields.
Field locks read `fldLock`, falling back to `lock` only when `fldLock` is absent;
invalid or missing flag values default to false independently of `dirty`.
Symbol font-family values preserve valid CSS spellings and quote invalid names
through the [shared CSS string serializer](../text.md), including escaping `<`.

Paragraph and run payloads carry owned properties, including style references,
numbering, tabs, line spacing, alignment, keep/break flags and run tag alignment.
Common paragraph metadata retains raw colors and converted font sizes. Each
property group replaces its ordered CSS map without clearing unrelated metadata.
Paragraph spacing and alignment update both metadata and CSS through the shared
property dispatcher. Conditional classes use a present bit-string before Boolean
flags, including an authoritative empty mask. Parsed tabs are retained independently
of experimental positioning.

Paragraphs also retain owned nested section properties: page dimensions and margins,
open section/orientation strings, columns, ordered header/footer references,
title-page flags, page numbering and raw physical-side borders. Border frame/shadow
and column separator flags remain nullable. Repeated singleton groups replace the
whole earlier group; later paragraph property groups without a section retain the
existing section. Package-level section composition, pagination, inheritance and
rendering remain separate work.

Tables keep their rows, the last grid's column widths and separate table and
cell style maps from their last property group. Style references, look classes
and band sizes survive later groups. Centered and right-aligned tables use
automatic margins, floating positions add their text distances to any existing
margin, and hidden tables are not displayed. Rows keep conditional classes,
nullable header flags and grid offsets. Cells keep nested paragraphs and tables,
spans, merge markers (a missing value continues the merge), conditional classes
and text directions, which apply after the cell's other properties.

Hyperlinks keep nullable relationship identifiers and anchors; smart tags keep
nonempty URIs and element names. Both own their immediate runs, and those runs
point back to them. Comment references and range markers retain typed nullable
identifiers in inline order; comment rendering remains separate work.

Bookmarks retain nullable identifiers, names and independent column numbers in
source order. Drawings retain inline/anchored wrapper geometry, wrapping and image
relationships, crop sides and rotation. VML shapes retain ordered SVG attributes,
raw style text, image metadata and nested textbox content in the same arena.
Office math retains typed equation trees, optional property groups and formatted
runs. Alternate content selects the first fallback's first element; unavailable
fallbacks are skipped without losing surrounding run content. Missing required
picture structure returns an XML error through the enclosing parse operation;
no partial tree is returned. Drawings append to existing run content.
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
existing length conversion. These operations also supply the shared property
dispatcher; they do not render a document.

## Word default properties and conditional table styles

`maestro_app::browser::docx_preview::parse_default_properties` updates supplied
ordered CSS maps from immediate property children. It handles alignment, colors,
font size and families, underline, indentation, paragraph spacing, cell margins,
row height and table layout. The optional child map receives table borders and
cell margins; paragraph and cell borders stay in the main map. Replacements keep
their original key positions, including nullable values.

Table layout selects `type` before the legacy `val` fallback; only exact `fixed`
selects fixed layout, and a present empty type does not fall back. Table indentation
retains paragraph fields, then a present `w` replaces only `margin-inline-start`:
`nil` writes `0.00pt`, `pct`/`auto` leave the paragraph result unchanged, and other
or absent types use the shared dxa margin conversion. Missing `w` makes no extra
assignment. These table properties stay in the main map regardless of cell-width
suppression.

`DefaultPropertiesOptions` defaults to no width suppression and no interceptor.
Its `ignore_width` suppresses cell widths only. The borrowed synchronous handler
sees and may mutate the main map before each child's dispatch; returning true
consumes that child. Font parsing selects ASCII, ASCII theme and East Asian
families, using the existing font enclosure. Position writes the valid
`vertical-align` CSS key.

`parse_table_style` prepares owned `TableStyle` selector/modifier/value records
for first/last rows and columns and horizontal/vertical banding. Property-child
order, duplicates and empty maps are retained. Unsupported regions return no
records. Table-style preparation remains separate from the owned element tree;
neither operation renders a preview.

## Word styles and auxiliary parts

`maestro_app::browser::docx_preview` exposes `StylesPart`, `ThemePart`,
`HeaderPart`, `FooterPart`, `FootnotesPart`, `EndnotesPart` and `SettingsPart`.
Their `parse_xml` factories prepare owned records separately from package
loading; headers, footers and notes reuse the ordinary body parser with the
caller's width policy.

Styles retain ordered named/default definitions, repeated identifiers,
relationships and CSS declarations without resolving inheritance. Each style
property group appends CSS while replacing its corresponding raw metadata;
this differs from paragraph-node metadata merging. The supplied `ignore_width`
policy reaches both ordinary and conditional declarations and suppresses only
cell widths. An aliases child without a value fails with an XML-part error.

Themes keep raw color values in insertion order and independent major/minor font
groups. Direct sRGB children take precedence over system-color fallbacks. Missing
font children remain distinct from children with missing typeface attributes.
The required `themeElements` child must be present. Settings keep encountered tab
sizes, nullable hyphenation flags and separate note-default groups with ordered,
nullable IDs. Repeated schemes and note-default groups replace earlier groups.

Header/footer roots have static Rust kinds and initially empty CSS maps. Note
records retain raw nullable IDs/types and repeated IDs in source order. Note
parts select their own immediate note kind using one admitted source per call.
Package-to-part composition and preview rendering remain separate from these
factories; no inherited-style resolution or document rendering occurs in the
application factories.

## Word numbering and embedded fonts

`maestro_app::browser::docx_preview::lists::NumberingPart::parse_xml` returns
ordered raw definitions, concrete references, overrides and nullable picture
slots, plus an owned concrete level sequence. Each matching concrete instance
keeps its identifier; ordered level/start overrides affect that instance only.
Picture references resolve even when their declarations follow the definition.
Raw records remain separate from the prepared levels. Reference binding uses the
first immediate abstract-reference child, while raw scalar metadata keeps the last
write. Numbering property groups reuse the [raw metadata](#owned-word-element-tree)
and [CSS dispatcher](#word-default-properties-and-conditional-table-styles): raw
groups replace, CSS accumulates, and `ignore_width` reaches both style maps.

`font_table::FontTablePart::parse_xml` retains nullable font metadata and ordered
embedded-face references. `WordDocument::load_font` takes the retained font-table
`Part`, relationship ID and optional key through the existing
[resource lookup](#word-package-reader). Missing resources return `None` before
key validation; an existing resource, even an empty one, requires a key or returns
`DocxError::FontKey` (`Embedded font key is missing`). It returns owned bytes,
XOR-decoding at most the first 32 bytes with the reversed hexadecimal key and
leaving the tail unchanged. The archive is not rewritten. These operations do
not create browser URLs, load fonts into a browser or render a preview.

## Word page rendering

`maestro_web::docx_preview::parse_async` opens a Word package with the readers
above and prepares its body, styles, theme, numbering, fonts, notes, headers and
footers. A malformed part fails the load with `DocxError`, so rendering itself
cannot fail. `WordPreview::render` returns the styles, the pages and the resource
styles one rendering composes; `render_sections` returns the pages alone, with the
object URLs their nodes name.
`docx_preview::Options` defaults to the `docx` class namespace, XML declaration
trimming, page-break detection, ignored last-rendered page breaks, kept page
sizes, rendered headers, footers and note lists, the page wrapper, loaded fonts,
object URLs and rendered alternate chunks. The reading options decide the URL
kind; a later rendering cannot change it.

A paragraph's section properties end a section after that paragraph, and the
body's own section properties apply to the sections after the last such
paragraph. A page break inside a paragraph splits the paragraph, and the run
holding the break, across two pages; a break that ends the paragraph only ends the
page.
A paragraph style with page-break-before ends the current section before the
paragraph, and the ended section uses the properties of the group after it. While
last-rendered page breaks are ignored, each section starts a new page; otherwise
pages break at page breaks, last-rendered page breaks and changes of orientation
or page size.

Each page carries the margins, width and minimum height of its first section,
unless widths or heights are ignored, and the document background. Each
section's content carries its column count, gap and rule. The first page of a
section with a title page uses the first-page header and footer, even pages use
the even ones, and other pages the default ones. Their offsets from the page
margins are written as `calc()` expressions, and a reference to a missing part
renders nothing. Footnotes are numbered per page and listed at the end of their
page; endnotes are numbered through the document and listed at the end of the
last page. A reference keeps its number when its note is missing, and repeated
note identifiers use the last note. Tables keep their column widths, cell spans
and placeholder cells for grid offsets. In a vertical merge the first cell spans
the merged rows and the others are hidden. Body, header and footer tables hand
their cell borders and margins to cells that declare none.

Insertions flatten into ordered children; deleted content, field runs and
non-wrapping breaks are omitted. Tabs use an em space, and nonbreaking hyphens
remain visible without introducing a break. Symbols decode numeric references
without reading authored suffixes as markup. Paragraph and run classes keep the
caller's namespace; a local numbering group, even an empty one, suppresses the
style's numbering class.

A link reads the relationships of the part that holds it: the main document for
the body, the header or footer part for its content, and the notes part for
notes. Its target is the first of those relationships, in retained order, whose
identifier equals the link's nonempty identifier and whose target mode is
exactly `External`; the relationship type is not checked. A matched
relationship without a target gives an empty target and ends the search, and an
absent, empty or unmatched identifier gives an empty target too. A nonempty
anchor appends `#` and the anchor, so the target becomes `target#anchor`, or
`#anchor` alone. The value keeps its authored spelling, with no percent-encoding
or trimming. It then passes the same
[`executable`](../../crates/maestro-app/src/browser/links.rs) check the
worksheet tables use. A refused link keeps its element and text but has no
`href`, so following it does nothing.

Style lookup selects the last occurrence of an exact nonempty identifier. A
style's numbering and page-break-before come from the nearest style along its
base chain that sets them, with the numbering identifier and level resolved
separately, so a style's own values, false flags included, win over its bases.
Each chain reads unchanged definitions and stops at its first repeated style.
This corrects base values overwriting a style's own, styles without paragraph
properties inheriting nothing, and styles defined before their base losing the
rest of the chain.

Each style value stays inside its own declaration: a value whose strings or
brackets do not close, or that holds a separator, block, comment, priority or
escape outside a string, is dropped, as an invalid style property is, so a
document cannot add declarations. Values the renderer composes itself, such as
resource URLs, counters and level text, are written as composed.

## Word preview composition

One rendering carries the library stylesheet, the page wrapper, theme variables,
document and list styles, embedded font faces, the pages, the images and shapes
they name, and the alternate format chunks. `WordPreview::render` returns them as
a `Rendering`: `styles` holds the style elements in the order the library mounts
them, `body` holds the page wrapper or the pages themselves, and `resource_styles`
holds the picture-bullet variables and font faces that follow the body. A mount
writes `styles` and then `resource_styles` into the style container, which is the
body container unless one is given, and `body` into the body container, so a
shared container shows the picture bullets and font faces after the pages.

The style container holds the library stylesheet, the theme, the document styles,
the list styles, the picture-bullet variables and the font faces, in that order.
The library stylesheet declares the page frame, the page box, the page content,
the tables and the inline defaults under the class namespace; the two wrapper
rules sit inside `@media not print` when the wrapper is hidden on print. The theme
declares `--docx-majorHAnsi-font`, `--docx-minorHAnsi-font` and one
`--docx-<slot>-color` per colour slot, writing `null` for a missing typeface or
colour value and `undefined` for an absent font entry. Document styles emit their
own resolved substyles, then their linked style's, with the last default
definition of a target also matching that target anywhere in the document. List styles emit a marker, a counter set per level and a
`counter-reset` on the page wrapper, or on the document root when the pages are
unwrapped. Font faces name their embedded bytes once de-obfuscated and add
`font-weight` and `font-style` by face kind.

Images, picture bullets, fonts and alternate chunks load against the part whose
relationships name them: the document part by default, and each header, footer
and note part for its own content. Their bytes become a data URL when the package
was read with base64 URLs, and an object URL otherwise; the rendering keeps that
object URL until it is dropped. A drawing becomes an inline block holding its content, a
picture carries its crop and rotation as a transform and a clip path, shapes
become SVG, equations become MathML, and a chunk becomes a frame whose `srcdoc`
holds the chunk's text and which runs no script.

`render_document` clears the containers and mounts a rendering as nodes built with
`createElementNS` in each element's namespace, assigning attributes in order. One
frame later it sizes each shape image to its shape's bounding box.
`render_async` parses and mounts in one call, returning the parsed preview beside
the mounted document. The `RenderedDocument` it returns cancels pending shape
sizing and revokes its object URLs when it is dropped.
The attachment mount belongs to
[the attachment DOM switch](https://github.com/Orchestration-Maestro/maestro/issues/1761).

These corrections to the library are kept: an alternate chunk's frame carries
`sandbox=""` so its scripts never run; a value that would leave its own
declaration, and a style, counter or variable token that is not a plain
identifier, are dropped; a font name CSS would reject is enclosed as a string; a
missing resource leaves its reference unset instead of writing `null`; an image in
a note reads the notes part's own relationships; a bottom bar underlines its base;
a malformed equation takes the documented default instead of failing the document;
and list level text keeps its literal `%`, quotes and backslashes.

## Worksheet tables

`sheet_to_html(&mut worksheet, id, &ssf)` returns a table fragment over the declared
range. It preserves absolute cell addresses, first-match merge order, spans and
formatted caches. Missing/null scalars have empty display content. Generated `h`
markup precedes escaped formatted text; fallback formatting uses the existing
formatter without caching error labels. Nonfinite numeric cells use temporary
error projections without changing the stored cell.

Modern reading retains generated rich/plain string markup in `CellObject::h` and
literal hyperlink targets in `CellObject::l`. `Hyperlink::target` preserves source
spelling, including embedded tabs/newlines; URL parsing at the table writer rejects
executable schemes while keeping ordinary links unchanged. Rich literal text and
identifier/format attributes are escaped at their writing boundary. Explicit XML
off-values leave rich flags disabled.

`CellObject::z` stores an optional authored number-format code or numeric selector
for explicitly supplied cells; HTML metadata retains either spelling. Default
workbook reading leaves it absent. General typed format options remain separate
from this projection.
