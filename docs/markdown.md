# Owned Markdown structure

`maestro_markdown::parse(source: String)` returns a `Document` owning the supplied
source and all native nodes, metadata and authored ranges. The same operations
are exposed through `maestro_app::markdown` for frontend consumers.

After mapped-link selection, `InlineContext::new(&document)` indexes original
container and link-label scopes. `scope(range, label)` exposes authored bytes,
resolved operands and opaque ranges without parsing a fragment for admission.
`InlineScope::interpret` applies native inline grammar locally with caller spans
and resolved operands as typed atoms. The caller supplies ordered, disjoint spans
within the selected UTF-8 source range. Link hrefs, titles and reference identities
remain those of the original document; label interpretation does not resolve them
again. Captured table-pipe provenance also keeps mapped URLs separate from their
neighboring inline source.

Parsing preserves native structure before extended links are selected. Call
`autolinks(&mut document)` separately to map bare URL, www and email spans.
Native code, authored links and images remain opaque to that pass. Identity text
units retain exact selected source subranges; decoded transformations stay atomic.

The parser retains link titles, destinations, reference identities and inline
labels; image destinations, titles and alt children; table alignments and authored
cell spans; task markers; and full code information and bodies. Container-local
source removes only enclosing native quote/list continuation prefixes. A node's
range still names the original document source, not its container-local spelling.

Callers own normalization and presentation. The terminal adapter normalizes line
endings, expands tabs and replaces NUL before parsing; it removes one final code
newline only when formatting terminal code. Browser text policy does not belong
in this shared owner.
