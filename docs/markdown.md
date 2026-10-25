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

Each inline tokenizer indexes its source's `*` and `_` run endpoints once, and
prepares the raw closing positions its caller's deletion grammar admits. Recursive
emphasis and deletion reuse that preparation, with widths and closers clipped to
their selected window; projected link labels build an index for their own source.
The same source view also prepares where each backtick run can close a code span of
its own width and which bracket pairs a link label reaches, so a code-span or link
opener is answered from that preparation instead of re-reading the rest of the
window. The code-span runs take one pass over the source; the label scan takes one
forward pass over bracket pairs and one reverse pass over the closers they reach, and
records the next unescaped `]` at each offset. A reference opener whose window holds
no such closing bracket therefore skips the caller's reference rules, while an
admitted window still runs the explicit rule and, on an explicit miss, the shortcut or
collapsed rule over its whole window, so admitted openers keep their existing cost and
the tokenizer
as a whole makes no linearity promise.
Masks remain local to each window because clipping can reveal syntax hidden by an
enclosing mask. Label-closer lookup is constant time and code-span lookup is a binary
search over the prepared runs, while an opener whose label can reach a closing pair
still runs the caller's own link rule over its window. Right matching and recursive
masking keep their own traversal costs. Deletion keeps its own raw closing rules
rather than the emphasis mask, so a raw `~~` inside code ticks still closes it.

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
trailing container's own raw source and its ancestor's unconsumed remainder. The
ancestor cuts back at the container's authored start, or at a body it published
itself, so a character is never split. The trailing list item keeps its admission
state and block output while its quote resumes. An item that stopped accepting
lines keeps only what publication and a caller tokenizer's replay read: its
settled block output with the step a replay resumes from, its semantic body, its
task marker state, and whether a blank line ends it.
Admission resumes at the first open block: one whose grammar read the item's
current end, or whose following content sits on the item's unterminated last
line. Open indented code continues from the line it reached. A caller block
tokenizer reads the whole remaining item, so when one is present every
resumption admits the item again. Item text answers the setext underline
question from one classification per line, and from the resume offset when a
caller token stopped inside a line, instead of re-reading the rest of the item,
so a long item costs its own length. Semantic text can change spelling or be
empty without erasing raw consumption. Code continuations reuse one
indentation-removal operation; definitions continuing paragraph or item text
stay literal instead of entering the reference index.

Quote and list nesting runs on retained frames, not on native recursion: one frame
stack drives quote chunks, list items and their block admission, so every deeper
level holds heap frames instead of stack. Each nesting level is built from the same
block structures a shallow document uses, with no container depth limit and no
nesting error. Inline spans are the exception: nested emphasis, links and labels
still interpret recursively on the native stack.

Finished containers publish their items, and replaced subtrees and dropped documents
release their children. The walks that follow a document - inline-scope indexing,
terminal render rows, browser markup and the documentation checks - use explicit
worklists.

Depth costs work: each level reads its own operand again, so the cost grows with
the square of the nesting depth. A single line of 1,000 quotes parses in about a
second in a development build, 2,000 in about four, and 4,000 in about fifteen.

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
