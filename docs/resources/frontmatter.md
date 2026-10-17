# Frontmatter

The resources crate uses one owned YAML reader for parsing and stripping headers.
Skill and prompt loaders consume its ordered, owned values through the same seam.
The reader supports block/flow collections, scalar tags, completed anchors and
plain, quoted, literal and folded scalars; it rejects duplicate typed keys and
unresolved aliases before a loader selects its fields.

Double-quoted strings in extracted metadata accept an adjacent four-digit
high-surrogate/low-surrogate escape pair as one Unicode scalar. For example,
`description: "\uD83C\uDF89"` produces `🎉`.

Other escaped surrogate halves remain invalid, including in metadata the loader
does not select. Lone, reversed and separated halves are rejected; malformed
metadata keeps the parser's native errors.

Single-quoted, plain and block scalars do not interpret these backslash escapes;
ordinary valid eight-digit scalar escapes remain supported in double-quoted
strings.
