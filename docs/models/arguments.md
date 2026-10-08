# Tool arguments

## Streamed arguments

`maestro-models` provides four shared parsing and sanitizing operations for
model-supplied JSON.

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

## Invocation checking

`validate_tool_call` selects the first declaration with the invocation's exact
name. `validate_tool_arguments` checks one supplied declaration independently of
its name. Both return an owned argument object and leave the invocation and schema
unchanged.

Checking converts declared primitive values before evaluating the original schema.
Objects and arrays are traversed only when their type is explicitly declared.
Combinator alternatives use independent candidates; the first valid converted
alternative wins. Type unions preserve values that already match a member.
Missing values are not defaulted and unknown fields are not removed.

The checker supports object, array, string, number and composition constraints,
offline schema resources, anchors, recursive and dynamic references, and evaluated
property/item tracking. Both tuple forms can occur in one schema. References are
never fetched; unknown resources and non-progressing cycles reject normally.
Every string `$id`, including a fragment-only identifier, becomes the reference
base for its subtree. Array pointer segments use exact unsigned index names:
`0` and `1` resolve, while `+0`, `+1` and `01` do not.
Known string formats are asserted, unknown format names remain annotations, and
string lengths count extended grapheme clusters.

Failures report ordered field diagnostics followed by the original arguments as
two-space-indented JSON. Numeric-index keys precede ordinary insertion-ordered
keys. Returned diagnostics have an error name and message, without a manufactured
stack or code.

```rust
use maestro_models::{JsonObject, Tool, ToolCall, validate_tool_arguments};
use serde_json::json;

# fn main() -> Result<(), Box<dyn std::error::Error>> {
let tool = Tool {
    name: "count".into(),
    description: "Count supplied items".into(),
    parameters: json!({
        "type": "object",
        "properties": {"count": {"type": "integer", "minimum": 1}},
        "required": ["count"]
    }),
};
let mut call = ToolCall {
    id: "call-1".into(),
    name: "count".into(),
    arguments: JsonObject::from_iter([("count".into(), json!("42"))]),
    thought_signature: None,
};
let checked = validate_tool_arguments(&tool, &call)?;
assert_eq!(checked["count"].as_f64(), Some(42.0));
assert_eq!(call.arguments["count"], "42");

call.arguments.remove("count");
let error = validate_tool_arguments(&tool, &call).unwrap_err();
assert_eq!(error.message,
    "Validation failed for tool \"count\":\n  - count: must have required properties count\n\nReceived arguments:\n{}");
# Ok(())
# }
```
