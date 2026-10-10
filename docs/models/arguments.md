# Tool arguments

## Streamed arguments

`maestro-models` provides four shared parsing and sanitizing operations for
model-supplied JSON.

`repair_json(&str) -> String` escapes raw controls and invalid backslashes only
inside string literals. Valid escapes and text outside strings stay unchanged.
An unfinished Unicode escape remains invalid. `parse_json_with_repair` tries
strict JSON first, then changed literal repair. Native magnitude and lone-surrogate
errors retry raw projection: overwritten lone-surrogate members may be discarded
when the surviving value is representable. If projection fails, the native error
is retained; failure returns the last attempted strict reader's cause through
`DiagnosticErrorInfo`. See the [whole-value conversion boundary](chat-completions.md#whole-value-conversion)
for unrepresentable member names.

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
exponent suffixes retain the numeric prefix. Invalid leading grammar and separators
stop the current member, preserving earlier members.
Numbers round to binary64 doubles: `9007199254740993` becomes `9007199254740992`,
and valid overflow such as `1e400` becomes JSON null, including inside incomplete
containers. Negative zero retains its sign in values; compact JSON writes it as `0`.
Root numbers, including numeric overflow projected to null, require the whole input;
other completed non-null values can precede trailing text. Inner whitespace follows JSON grammar; outer trimming also accepts
Unicode spacing and the byte-order mark, but not U+0085. No prose extraction,
comments, single quotes or unquoted keys are accepted.

Full argument materialization accepts at most 127 nested arrays or objects.
Deeper values fail strict parsing and yield `{}` during streaming, rather than a
truncated container prefix. The bound counts consumed containers, not sibling
width, quoted delimiters or unconsumed trailing text.

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

## Checking without conversion

`maestro_models::arguments::validation::validate_schema` checks an original JSON
value without converting it or changing the schema. It returns diagnostic lines
for invalid values, an empty list for admitted values, or `DiagnosticErrorInfo`
when schema preparation fails. It uses the checker described under
[Invocation checking](#invocation-checking), without the invocation envelope or
received-arguments rendering.

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
Every string `$id`, including a fragment-only identifier, starts a resource that
is resolved once against its enclosing resource. Relative `$ref` and
`$dynamicRef` targets resolve against the resource that contains the reference,
not the document root, however that resource was reached: by identifier,
pointer, anchor or dynamic binding. A `$recursiveRef` searches only its own
resource; when that resource declares `$recursiveAnchor: true` it searches only
the subtree of the first entered schema declaring one and resolves against the
resource containing that schema. Entering any schema, with or without `$id`, binds
its own `$recursiveAnchor: true` and `$dynamicAnchor`; entering a schema with `$id`
also binds the dynamic anchors declared below it, arrays of schemas included,
without crossing a nested `$id`. Following a reference keeps the caller's live
anchors, enters the target's enclosing resources that are not yet entered, then
enters the target. The outermost binding of a name wins, a sibling property
never sees bindings added for another, and a pointer fragment never takes a
dynamic binding. Array pointer segments use exact unsigned index names: `0` and
`1` resolve, while `+0`, `+1` and `01` do not.
Known string formats are asserted, unknown format names remain annotations, and
string lengths count extended grapheme clusters.

Declared conversions run first, skipping a combinator alternative whose own
expressions do not compile. Then, before any assertion is evaluated, the
checker builds once the Unicode expression of every `pattern` and every
`patternProperties` key in the schemas it can reach: the nested schemas of each
keyword and the targets of `$ref`, `$recursiveRef` and `$dynamicRef`, whatever
the arguments are. A target is built in every scope that reaches it, because a
dynamic reference selects its target from the anchors live in that scope.
Unreferenced `$defs` and `definitions`, `then` and `else` without `if`, data such
as `const` and `enum`, and keyword values that are not schemas are never built.
`properties`, `patternProperties`, `dependencies`, `dependentSchemas` and
`dependentRequired` are read only when they hold an object; a list in their place
is ignored. An expression that does not compile is a schema error, not a failed
assertion: the call returns the expression error alone, so no union tolerates it
and no negation inverts it. When several are malformed, the one built first is
reported: object and array keywords with their nested schemas, then the
`pattern` beside them, then reference targets, then conditionals, negation and
combinators.

Evaluated properties and items come only from schemas that succeed. A failing
branch, a negation, a failing `then` or `else`, and the items checked by `contains`
cover nothing, and the properties a nested value evaluated never cover those of
its parent.

Failures report at most eight distinct field diagnostics in evaluation order,
followed by the original arguments as two-space-indented JSON. Repeated identical
messages, including missing-dependency messages, appear only once. Numeric-index
keys precede ordinary insertion-ordered keys. Returned diagnostics have an error name and message, without a manufactured
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
