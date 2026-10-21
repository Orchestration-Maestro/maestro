# Owned Markdown structure

`maestro_markdown::parse(source: String)` returns a `Document` owning the supplied
source and all native nodes, metadata and authored ranges. The same operations
are exposed through `maestro_app::markdown` for frontend consumers.
`Alignment` and `LinkType` belong to the shared Markdown owner. They describe
column alignment and link handling through owned Rust types. Links distinguish
authored labels (including resolved references) from automatic URI and email
destinations, whether angle-enclosed or bare.

After parsing, `InlineContext::new(&document)` indexes original
container and link-label scopes. `scope(range, label)` exposes authored bytes,
resolved operands and opaque ranges without parsing a fragment for admission.
`InlineScope::interpret` applies native inline grammar locally with caller spans
and resolved operands as typed atoms. The caller supplies ordered, disjoint spans
within the selected UTF-8 source range. Link hrefs and titles remain those of the
original document; label interpretation uses inline-only grammar and does not
resolve them again. Subrange lookup selects source-ordered operands by
start offset rather than scanning the entire scope. Captured table-pipe provenance
also keeps mapped URLs separate from their neighboring inline source. Link-label
scopes retain authored escapes. Export and cursor interpretation unescape label
brackets before applying inline grammar, with ranges still pointing to the original source.

`InlineScope::interpret_with_tokenizer` tries an `InlineTokenizer` at each reached
inline cursor, before built-in grammar. Its `InlineToken` selects a nonempty UTF-8
prefix and a caller identity; invalid prefixes and hint boundaries are ignored.
Unlike static caller spans, cursor tokens leave their contents visible to emphasis
matching. Recursive spans and labels use their selected child windows. Generated
nodes own their source, while returned runs retain original document operands,
not the temporary tokenizer. Code, image and autolink ranges remain barriers.
An earlier caller token may consume an authored HTML tag; otherwise the tag remains
one retained operand, so tokens never start inside its attributes.

Each inline tokenizer indexes its source's `*` and `_` run endpoints once.
Recursive emphasis reuses that index with widths clipped to its selected window;
projected link labels build an index for their own source. Masks remain local
to each window because clipping can reveal syntax hidden by an enclosing mask.
Run-width lookup is constant time; right matching and recursive masking still
have their own traversal costs.

Parsing emits bare URL, www and email links at reached inline cursors outside
link labels and raw anchors in the terminal, browser and export grammars.
Raw-anchor context follows the document's queued inline bodies, including bodies
split by caller block extensions; completing an authored link or image label
clears it. Scoped interpretation starts with the context at the
selected range and applies the same transitions to retained operands.
Documentation grammar still applies emphasis inside such text.
Bare-link labels and destinations retain their authored spelling, while surrounding
text is decoded. Source ranges point to the original document, including through
container framing and escaped table pipes.
Web prefixes require an ASCII letter, digit or hyphen immediately afterward, not
a valid domain name; later bytes may include underscores or Unicode. Terminal
and export grammars admit mixed-case prefixes only at reached text cursors,
including after consumed escaped punctuation. Browser protocol matching is
case-insensitive, while its `www.` prefix is lowercase.
The recognizer skips failed email-local runs without retrying each suffix;
this does not bound the cost of parsing emphasis or other inline grammar.

The parser retains link titles, destinations and inline labels; image destinations,
titles and alt children; table alignments, cell children and original source ranges;
task markers; and full code information and bodies. Export renders literal image alt text,
not its inline children; those children remain available in the shared document.
Container-local source removes quote/list continuation prefixes. Aligned list continuations expand
tabs to four spaces; lazy continuations retain authored tabs. A node's range still
names the original document source, not its container-local spelling.

Documents, list items and quote chunks share one block accumulator. Quotes
suspend and resume through retained source handles. Quote reentry retains the
trailing container's own raw source and its ancestor's unconsumed remainder.
A list item keeps its admission state and block output while its quote
resumes. Only blocks whose grammar read the item's current end are admitted
again, and open indented code continues from its last line. A caller block
tokenizer reads the whole remaining item, so when one is present every
resumption admits the item again. Semantic text can change spelling or be empty
without erasing raw consumption. Code continuations reuse one indentation-removal
operation; definitions continuing paragraph or item text stay literal instead
of entering the reference index.

`Node.authored` and quote bodies use `MappedText`, which owns shared UTF-8 text.
Cloning a value keeps it alive after its document is dropped. Concatenation and
selection share a balanced sequence, so retained ownership depth stays logarithmic
in the number of nonempty pieces, including when snapshots remain live. `len()` returns
its logical byte length in constant time; `is_empty()` tests that length.
`chunks()` yields the exact bytes as borrowed strings in order, without
flattening. Chunk boundaries are not part of the text format.
`to_owned_string()` explicitly allocates the complete logical value.
`MappedText::from(String)` accepts caller-owned text; the default value is empty.
The type supports `Clone` and `Debug`, but no implicit `Deref<str>` or flattened
cache. `Document.source` remains a `String`; source ranges and inline interfaces
are unchanged. Reading all overlapping quote bodies materializes their combined
logical lengths, rather than measuring retained parser storage.

Callers own normalization and presentation. The terminal adapter normalizes line
endings, expands tabs and replaces NUL before parsing; it removes one final code
newline only when formatting terminal code.
`parse_with_options(source, MarkedOptions)` selects terminal, browser, export or
documentation grammar. Browser selection includes whether authored HTML is
shown as text. Each document retains that selection and its first-wins reference
index for later inline interpretation; writers keep presentation policy.

`InlineScope::text()` retains authored container padding. `content_range()`
selects the semantic child extent in that source, so renderers can re-enter the
inline grammar without adding padding to table content.

`parse_with_tokenizer` accepts one caller-owned block tokenizer with a start hint
and token admission. Custom blocks retain the caller's kind and text; writers
render them. Callbacks receive every contiguous operand in order, including
repeated admissions during container reentry. Their operand materialization is
counted separately as caller-requested output; contiguous source slices are
borrowed. Invalid source prefixes or UTF-8 hint boundaries return
`InvalidBlockToken`. The ordinary parse helpers use the same path without a
custom tokenizer. A start hint clips paragraph admission before inline parsing,
including inside later code, links or HTML. Even an unmatched display opener can
split a paragraph's queued inline source, so bare links on either side are selected
independently. Native inline masking does not suppress block hints.
