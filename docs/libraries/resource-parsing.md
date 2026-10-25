# Resource parsing

The resources crate owns its frontmatter YAML reader. A single lexer owns token
boundaries. An iterative parser builds a private syntax tree; composition validates
its properties and resolves ordered metadata. There is no bundled parser or public
YAML SDK.
See [frontmatter](../resources/frontmatter.md) for scalar and escape behavior.
Source-derived grammar attribution is in the [third-party notices](../../THIRD-PARTY-NOTICES.md#frontmatter-grammar).

Numeric scalar-key spelling uses `ryu-js =1.0.3` with defaults, under Apache-2.0.
Radix scalar resolution uses `num-bigint =0.4.8` and `num-traits =0.2.19` with
defaults (MIT OR Apache-2.0) for arbitrary-width integers and correctly rounded
floating-point conversion. Shorthand tag suffixes use the already-shared
`percent-encoding =2.3.2` with defaults (MIT OR Apache-2.0) after validating
percent triplets; decoded suffixes must be valid UTF-8. Verbatim and local tag
names are not percent-decoded.

Discovery uses the shared `maestro-pattern` ignore rules; its dependencies are
documented in [file-pattern libraries](package-pattern.md). Authored path operations
use `maestro-path`, and text normalization uses `maestro-text`. Internal resource
dependencies remain the shared request-record, path, text and file-pattern owners.
