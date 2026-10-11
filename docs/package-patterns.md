# Package file patterns

`maestro_packages::minimatch(path, pattern)` evaluates a file pattern without
filesystem access. Package discovery does not yet use this operation.

Matching is case-sensitive outside native Windows drive identity. `*` matches
zero or more scalars within a nonempty component; `?` matches one Unicode scalar.
Wildcard components exclude leading dots unless the pattern explicitly starts
with a dot, and never match `.` or `..`. Literal `.` and `..` remain matchable.
A whole-component `**` spans permitted components, excluding hidden and traversal
components. A terminal `/**` requires a remaining component, including an empty
component supplied by a trailing separator.

Leading `#` makes the original pattern a comment. Each leading `!` toggles
negation. `/` always separates components; within a component, a backslash escapes
the next scalar and a final backslash remains literal.
Brace alternatives and sequences use the
[brace-expansion library](https://docs.rs/brace-expansion/0.1.1/brace_expansion/).
Repeated separators collapse; eligible pattern `component/..` pairs reduce,
without normalizing the input path. Windows input backslashes are separators;
pattern backslashes remain escapes. Windows drive letters compare without case,
including extended drive prefixes; other components remain case-sensitive.

Patterns longer than 65,536 Unicode scalars return `PatternError::TooLong`, whose
message is `pattern is too long`. Extended groups are not yet implemented.

## Bracket classes

A bracket class consumes one Unicode scalar. `[abc]` selects members and `[a-z]`
selects an inclusive range. A first unescaped `!` or `^` complements the complete
member union; escaped or later markers are literal. A closing bracket in the first
member position is a member. Backslashes escape scalars, not regular-expression
syntax. Leading/trailing hyphens can be literal.

Descending ranges discard their own members. A class with no surviving members,
or a range ending at a recognized named class, cannot match that alternative.
An unclosed opening bracket is literal unless a recognized named range endpoint
has already made the alternative impossible; otherwise following syntax still
parses normally.
A lone surviving ordinary member without negation becomes a literal, including
`[.]` and `[a-a]`; authored duplicates such as `[..]` remain magic. Magic classes
use the component guards described above.

Named members use forms such as `[[:alpha:]]`, and combine with ordinary members
and outer negation. These names select the following Unicode categories:

| Name | Membership |
|---|---|
| `alnum` | Letters, letter numbers, decimal digits (`L`, `Nl`, `Nd`) |
| `alpha` | Letters and letter numbers (`L`, `Nl`) |
| `ascii` | U+0000 through U+007F |
| `blank` | Space separators (`Zs`) and tab |
| `cntrl` | Controls (`Cc`) |
| `digit` | Decimal digits (`Nd`) |
| `graph` | Neither separators (`Z`) nor Other (`C`) |
| `lower` | Lowercase letters (`Ll`) |
| `print` | Characters outside Other (`C`) |
| `punct` | Punctuation (`P`) |
| `space` | Separators (`Z`) and tab, CR, LF, VT, FF |
| `upper` | Uppercase letters (`Lu`) |
| `word` | Letters, letter numbers, decimal digits, connector punctuation (`L`, `Nl`, `Nd`, `Pc`) |
| `xdigit` | ASCII A–F, a–f, 0–9 |

Named space excludes U+0085 and U+FEFF. Printable membership excludes controls,
formats, unassigned and private-use scalars. Unknown or differently cased named
syntax follows ordinary bracket parsing.
