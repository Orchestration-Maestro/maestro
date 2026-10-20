# Owned Markdown structure

`maestro_markdown::parse(source: String)` returns a `Document` owning the supplied
source and all native nodes, metadata and authored ranges. The same operations
are exposed through `maestro_app::markdown` for frontend consumers.
`Alignment` and `LinkType` belong to the shared Markdown owner. They describe
column alignment and link handling through owned Rust types. Links distinguish
authored labels (including resolved references), angle-enclosed URIs or mapped
bare destinations, and angle-enclosed email addresses.

After mapped-link selection, `InlineContext::new(&document)` indexes original
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
not the temporary tokenizer. Code, image, HTML and autolink ranges remain barriers.

Parsing keeps accepted bare addresses whole outside link labels and raw anchors in
the terminal, browser and export grammars. Raw-anchor context is local to each
inline block; completing an authored link or image label clears it. Scoped
interpretation starts with the context at the selected range and applies the same
transitions to retained operands. Documentation grammar still applies emphasis
inside such text. Call `autolinks(&mut document)` separately to map bare URL, www
and email spans to links.
Native code, authored links and images remain opaque to that pass. Identity text
units retain exact selected source subranges; decoded transformations stay atomic.
The candidate scan consumes failed email-local runs without retrying each suffix;
this does not bound the cost of parsing emphasis or other inline grammar.

The parser retains link titles, destinations and inline labels; image destinations,
titles and alt children; table alignments and authored cell spans; task markers;
and full code information and bodies. Container-local
source removes quote/list continuation prefixes. Aligned list continuations expand
tabs to four spaces; lazy continuations retain authored tabs. A node's range still
names the original document source, not its container-local spelling.

Documents, list items and quote chunks share one recursive block accumulator.
Quote reentry keeps the trailing container's own raw source and the unconsumed
remainder. Semantic text can change spelling or be empty without erasing raw
consumption. Code continuations reuse one indentation-removal operation;
definitions continuing paragraph or item text stay literal instead of entering
the reference index.

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
render them. Invalid source prefixes or UTF-8 hint boundaries return
`InvalidBlockToken`. The ordinary parse helpers use the same path without a
custom tokenizer.
