# Content-generation conversion

`maestro_models::providers::content::google_shared` converts a conversation, its tools
and finish reasons into the serializable content turns of the Gemini-style protocols.
It sends nothing; requests and streams are built on top of it. `GoogleThinkingLevel`
is also exported at the crate root.

`convert_messages(model, context)` first runs the [conversation projection](https://github.com/Orchestration-Maestro/maestro/blob/main/docs/conversation-projection.md)
for `model`, which decides replay identity, image placeholders, failed turns and
missing tool results, then emits user and model turns. Blank assistant text and
thinking is omitted (script whitespace, including the byte-order mark, counts as blank); an empty user block
list or an assistant message left without parts produces no turn. A thought
signature survives only on content the projection keeps from the same provider, API and
model, and only when it is nonempty, a multiple of four long, made of base64 characters
and ends in at most two `=`; it is not decoded. Call identifiers are emitted only for `claude-` and `gpt-oss-`
models, where a replay across models also rewrites them.

Tool results become `functionResponse` parts under `output`, or under `error` when the
result failed. Text blocks are joined with newlines; a result whose retained content is only
images answers `(see attached image)` (images the model cannot accept are replaced by the
projection first). A result joins the previous user turn only when that turn is the last
emitted turn and already holds function responses. Retained images nest in the response
from Gemini 3 on and for every other family; older Gemini models receive them in a
following user turn that starts with `Tool result image:`, which the next result does not join.

`convert_tools(tools, use_parameters)` returns `None` for no tools. With `false` each
schema is sent whole as `parametersJsonSchema`; with `true` it is sent as `parameters`
with `$schema`, `$id`, `$anchor`, `$dynamicAnchor`, `$vocabulary`, `$comment`, `$defs`
and `definitions` removed from every schema node, including schemas inside `properties`
maps and `items`, `anyOf`, `oneOf`, `allOf` and the other schema-holding keywords. Property
names and literal values (`default`, `const`, `enum`, `examples`) stay as written, as do
`$ref` and all other members.

`map_tool_choice` answers `AUTO`, `NONE` or `ANY`, and `AUTO` for any other string.
`map_stop_reason` accepts the 17 declared finish strings (`STOP` is `stop`,
`MAX_TOKENS` is `length`, the rest `error`) and fails on any other string with
`Unhandled stop reason: <reason>`; `map_stop_reason_string` never fails and answers `error`
for every string except those two. `is_thinking_part` looks only at the thought flag, and
`retain_thought_signature` replaces the held signature with a nonempty incoming one and
otherwise keeps the held one, even when it is empty.

```rust
use maestro_models::Tool;
use maestro_models::providers::content::google_shared::convert_tools;
use serde_json::json;

let tools = vec![Tool {
    name: "inspect".to_owned(),
    description: "Use inspect".to_owned(),
    parameters: json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$id": "urn:a",
        "$anchor": "a",
        "$dynamicAnchor": "dyn",
        "$vocabulary": {"vocab": true},
        "$comment": "note",
        "$defs": {"a": {"type": "string"}},
        "definitions": {"b": {"type": "number"}},
        "type": "object",
        "properties": {"value": {
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "$id": "urn:a",
            "$anchor": "a",
            "$dynamicAnchor": "dyn",
            "$vocabulary": {"vocab": true},
            "$comment": "note",
            "$defs": {"a": {"type": "string"}},
            "definitions": {"b": {"type": "number"}},
            "type": "string",
            "$ref": "#/$defs/a",
            "$dynamicRef": "#a",
            "pattern": "^keep$"
        }},
        "required": ["value"],
        "additionalProperties": false,
        "default": null
    }),
}];
let declarations = convert_tools(&tools, true).expect("one tool is declared");
assert_eq!(
    serde_json::to_value(declarations).expect("declarations serialize"),
    json!([{"functionDeclarations": [{
        "name": "inspect",
        "description": "Use inspect",
        "parameters": {
            "type": "object",
            "properties": {"value": {
                "type": "string",
                "$ref": "#/$defs/a",
                "$dynamicRef": "#a",
                "pattern": "^keep$"
            }},
            "required": ["value"],
            "additionalProperties": false,
            "default": null
        },
    }]}]),
);
```
