# Package file patterns

`maestro_packages::minimatch(path, pattern)` evaluates a file pattern without
filesystem access. Package discovery does not yet use this operation.

Matching is case-sensitive outside native Windows drive identity. `*` matches
zero or more scalars within a nonempty component; `?` matches one Unicode scalar.
Basic wildcard components exclude leading dots unless the pattern explicitly
starts with a dot, and never match `.` or `..`. Literal `.` and `..` remain matchable.
Groups apply the branch-specific rules below.
A whole-component `**` spans permitted components, excluding hidden and traversal
components. A terminal `/**` requires a remaining component, including an empty
component supplied by a trailing separator.

Leading `#` makes the original pattern a comment. Each leading `!` toggles
negation. `/` always separates components; within a component, a backslash escapes
the next scalar and a final backslash remains literal.
Brace alternatives and sequences use the
[brace-expansion library](https://docs.rs/brace-expansion/0.1.1/brace_expansion/).
Repeated separators collapse except for the extra leading empty component in Windows
UNC roots; eligible pattern `component/..` pairs reduce,
without normalizing the input path. Windows input backslashes are separators;
pattern backslashes remain escapes. Windows drive letters compare without case,
including extended drive prefixes; other components remain case-sensitive.

Patterns longer than 65,536 Unicode scalars return `PatternError::TooLong`, whose
message is `pattern is too long`.

## Groups

`@(a|b)` selects one alternative, `?(a|b)` selects zero or one, `*(a|b)`
selects zero or more, and `+(a|b)` requires at least one. Alternatives can contain
nested groups, including nullable branches. Matching uses finite position states,
without a nesting cutoff. Escaped bars remain literal; bars inside bracket regions,
including recognized named members and their outer bracket, do not separate
alternatives. Unclosed outer groups use basic token parsing of
their remaining authored text, without recognizing nested groups in that fallback.

`!(a|b)` excludes alternatives together with the following component context,
including enclosing-sequence tails, but not another enclosing repetition.
Whole-pattern leading `!` still toggles negation before group parsing.
An actually empty negative group requires a scalar; a trailing empty alternative
does not erase other exclusions.

Directly empty alternatives are omitted for a positive group evaluated with both
component-start and component-end context. Start context passes through preceding
negative groups; end context passes through terminal nesting. A positive group
with no remaining alternatives then matches its authored syntax literally.
Otherwise, empty alternatives remain nullable. Nested nullable groups are not
treated as directly empty alternatives.

Dot guards depend on the selected branch and its repetition phase:
`@(.|..)` accepts literal traversal names, `*(a).x` can skip to the literal dot,
and later repetitions can consume dots after an initially permitted branch.
Magic class guards remain part of this branch selection.

For pattern size P and input-position count L (scalar count plus one), group
matching uses O(P² L³) work before ordered-set factors and O(P L² + P²) retained
state. Endpoint sets are cached before repetition closure; deduplicating an
expansion state does not discard its accepted endpoint or its later-dot phase.

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
use the basic component guards or, inside groups, the branch guards described above.

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
