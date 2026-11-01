# Frontmatter

The resources crate uses one owned YAML reader for parsing and stripping headers.
Skill and prompt loaders consume its ordered, owned values through the same seam.
The reader supports block/flow collections, scalar tags, completed anchors and
plain, quoted, literal and folded scalars; it rejects duplicate typed keys and
unresolved aliases before a loader selects its fields. Comment-only lines between
collection entries do not end the collection. Collections nested beyond 1,000
levels are rejected with an error. Alias expansion is
limited: a document whose aliases would expand past 100 weighted uses of one
anchor is rejected, so a hostile skill file cannot grow memory exponentially.

Double-quoted strings in extracted metadata accept an adjacent four-digit
high-surrogate/low-surrogate escape pair as one Unicode scalar. For example,
`description: "\uD83C\uDF89"` produces `🎉`.

Other escaped surrogate halves remain invalid, including in metadata the loader
does not select. Lone, reversed and separated halves are rejected; malformed
metadata returns a parsing error with source coordinates where available.

Single-quoted, plain and block scalars do not interpret these backslash escapes;
ordinary valid eight-digit scalar escapes remain supported in double-quoted
strings.

## Raw YAML documents

`maestro_resources::frontmatter::parse_yaml` parses one complete YAML document.
It returns `FrontmatterValue` directly: empty documents and null roots stay null,
and scalar, sequence and mapping roots retain their kinds. Mappings keep authored
key order. Numbers retain signed zero, NaN and infinities.

The reader accepts document start/end markers and separator text inside block
scalars. It rejects multiple documents and reports parser causes with source
coordinates. Newlines use the same normalization as headers.

`parse_frontmatter` extracts only a leading header, converts an empty or
null header to an empty mapping, and returns its parsed body.
