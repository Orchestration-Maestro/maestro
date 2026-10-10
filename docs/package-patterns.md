# Basic package file patterns

`maestro_packages::minimatch(path, pattern)` evaluates a basic file pattern without
filesystem access. Package discovery does not yet use this operation.

Matching is case-sensitive outside native Windows drive identity. `*` matches
zero or more scalars within a nonempty component; `?` matches one Unicode scalar.
Wildcard components exclude leading dots unless the pattern explicitly starts
with a dot, and never match `.` or `..`. Literal `.` and `..` remain matchable.
A whole-component `**` spans permitted components, excluding hidden and traversal
components. A terminal `/**` requires a remaining component, including an empty
component supplied by a trailing separator.

Leading `#` makes the original pattern a comment. Each leading `!` toggles
negation. Backslashes escape the next scalar; a final backslash remains literal.
Brace alternatives and sequences use the
[brace-expansion library](https://docs.rs/brace-expansion/0.1.1/brace_expansion/).
Repeated separators collapse; eligible pattern `component/..` pairs reduce,
without normalizing the input path. Windows input backslashes are separators;
pattern backslashes remain escapes. Windows drive letters compare without case,
including extended drive prefixes; other components remain case-sensitive.

Patterns longer than 65,536 Unicode scalars return `PatternError::TooLong`, whose
message is `pattern is too long`. Closed character classes and extended groups
are not yet implemented.
