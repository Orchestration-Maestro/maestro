# Owned Markdown structure

`maestro_markdown::parse(source: String)` returns a `Document` owning the supplied
source and all native nodes, metadata and authored ranges. The same operations
are exposed through `maestro_app::markdown` for frontend consumers.
`Alignment` and `LinkType` belong to the shared Markdown owner. They describe
column alignment and link provenance through owned Rust types. All reference
classifications remain constructible; wiki classifications do not enable wiki syntax.

After mapped-link selection, `InlineContext::new(&document)` indexes original
container and link-label scopes. `scope(range, label)` exposes authored bytes,
resolved operands and opaque ranges without parsing a fragment for admission.
`InlineScope::interpret` applies native inline grammar locally with caller spans
and resolved operands as typed atoms. The caller supplies ordered, disjoint spans
within the selected UTF-8 source range. Link hrefs, titles and reference identities
remain those of the original document; label interpretation uses inline-only grammar
and does not resolve them again. Subrange lookup selects source-ordered operands by
start offset rather than scanning the entire scope. Captured table-pipe provenance
also keeps mapped URLs separate from their neighboring inline source. Link-label
scopes retain authored escapes for caller admission; export interpretation unescapes
brackets before applying inline grammar, with ranges still pointing to the original source.

Parsing preserves native structure before extended links are selected. Call
`autolinks(&mut document)` separately to map bare URL, www and email spans.
Native code, authored links and images remain opaque to that pass. Identity text
units retain exact selected source subranges; decoded transformations stay atomic.

The parser retains link titles, destinations, reference identities and inline
labels; image destinations, titles and alt children; table alignments and authored
cell spans; task markers; and full code information and bodies. Container-local
source removes quote/list continuation prefixes and expands list continuation tabs
to four spaces. A node's range still names the original document source, not its
container-local spelling.

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
