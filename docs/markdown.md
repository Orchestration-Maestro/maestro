# Owned Markdown structure

`maestro_markdown::parse(source: String)` returns a `Document` owning the supplied
source and all native nodes, metadata and authored ranges. The same operations
are exposed through `maestro_app::markdown` for frontend consumers.

`parse_with_references(source, &document)` parses a fragment with the complete
document's resolved reference destinations and titles. Contextual renderers use
this operation to preserve links and images alongside formulas.

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
