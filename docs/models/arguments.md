# Tool arguments

`maestro-models` provides four shared operations for model-supplied JSON.

`repair_json(&str) -> String` escapes raw controls and invalid backslashes only
inside string literals. Valid escapes and text outside strings stay unchanged.
An unfinished Unicode escape remains invalid. `parse_json_with_repair` tries
strict JSON first, then changed literal repair; failure returns the last native
parser cause through `DiagnosticErrorInfo`.

`parse_streaming_json(Option<&str>)` returns a JSON value from the accumulated
prefix. It tries strict original, strict repaired, partial original and partial
repaired input, in that order. Missing, blank or unusable input yields `{}`.
Strict scalar values, including null, remain values; partial root null yields
`{}`. Containers preserve completed members and useful nested prefixes without
inventing values for unfinished keys. Quoted delimiters and completed escapes
survive incomplete strings. If escape decoding fails, recovery truncates at the
last backslash and retries the preceding text.
An incomplete string with a raw newline can leave an empty preview until closed.

Truncated booleans and null complete inside containers. Unfinished decimal or
exponent suffixes retain the numeric prefix. Invalid leading grammar, separators
and unrepresentable numbers stop the current member, preserving earlier members.
Root numbers require the whole input; other completed non-null values can precede
trailing text. Inner whitespace follows JSON grammar; outer trimming also accepts
Unicode spacing and the byte-order mark, but not U+0085. No prose extraction,
comments, single quotes or unquoted keys are accepted.

`sanitize_surrogates(&[u16]) -> String` preserves BMP text and valid UTF-16 pairs,
removing unmatched surrogate units. Ordinary Rust strings need no cleanup.

```rust
use maestro_models::{parse_streaming_json, parse_json_with_repair};
use serde_json::json;

let preview = parse_streaming_json(Some(r#"{"count":1,"label":"rea"#));
assert_eq!(preview, json!({"count":1,"label":"rea"}));
let completed = parse_json_with_repair("{\"label\":\"ready\n\"}")?;
assert_eq!(completed, json!({"label":"ready\n"}));
# Ok::<(), Box<dyn std::error::Error>>(())
```
