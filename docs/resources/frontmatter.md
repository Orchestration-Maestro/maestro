# Frontmatter

Double-quoted strings in extracted metadata accept an adjacent four-digit
high-surrogate/low-surrogate escape pair as one Unicode scalar. For example,
`description: "\uD83C\uDF89"` produces `🎉`.

Other escaped surrogate halves remain invalid, including in metadata the loader
does not select. Lone, reversed and separated halves are rejected; malformed
metadata keeps the parser's native errors.

Single-quoted, plain and block scalars do not interpret these backslash escapes;
ordinary valid eight-digit scalar escapes remain supported in double-quoted
strings.
