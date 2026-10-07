# Tool arguments

`parse_streaming_json` provides a best-effort value for display. It tries complete parsing with string repair, partial parsing, and partial parsing after string repair, in that order. If none succeeds it returns an empty object. Parsing neither validates arguments nor executes a tool.

`parse_json_with_repair` first tries the supplied text. It retries only when string repair changes that text, and reports the error from the attempt that failed. Parsed roots may be objects, arrays or primitives. Lone UTF-16 surrogate escapes become U+FFFD; valid pairs become their character. Out-of-range numbers are unreadable. The separate `sanitize_surrogates` text helper removes unpaired UTF-16 units.

`validate_tool_call` selects the first exact tool name; `validate_tool_arguments` validates against an explicitly supplied declaration. Both clone arguments before conversion. Metadata-bearing schemas and plain serialized schemas have distinct conversion behavior. A shared `TSchema` keeps its identity and compiled checker across replacement, while JSON serialization omits hidden schema metadata.

Validation errors include the call's tool name, corrective property or root paths, and the original received arguments as two-space-indented JSON. Neither successful nor failed validation mutates the supplied call. When primitive-root conversion changes the value but the converted value fails its schema, validation returns the original root without an error; callers must not infer authorization from that result.

Schema checking is offline: it retrieves no HTTP or file references. Union alternatives are checked independently, and an unbuildable alternative is skipped. Reader and checker diagnostics use the established wording, order and locations. Partial display values are never an instruction to execute a tool.

```rust
use maestro_models::{Tool, ToolCall, parse_streaming_json, validate_tool_call};
use serde_json::json;

let partial = parse_streaming_json(Some(r#"{"count":"4"#));
assert_eq!(partial, json!({"count":"4"}));
let tools = [Tool {
    name: "echo".into(),
    description: "Echo tool".into(),
    parameters: json!({
        "type":"object", "properties":{"count":{"type":"number"}},
        "required":["count"]
    }).into(),
}];
let call = ToolCall {
    id: "tool-1".into(), name: "echo".into(),
    arguments: json!({"count":"42"}), thought_signature: None,
};
let converted = validate_tool_call(&tools, &call).unwrap();
assert_eq!(converted, json!({"count":42}));
assert_eq!(call.arguments, json!({"count":"42"}));
```
