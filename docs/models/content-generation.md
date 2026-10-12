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
result failed. Text blocks are joined with newlines; a result with only images answers
`(see attached image)`. Consecutive results share one user turn. Images nest in the
response from Gemini 3 on and for every other family; older Gemini models receive them in
a following user turn that starts with `Tool result image:`.

`convert_tools(tools, use_parameters)` returns `None` for no tools. With `false` each
schema is sent whole as `parametersJsonSchema`; with `true` it is sent as `parameters`
with `$schema`, `$id`, `$anchor`, `$dynamicAnchor`, `$vocabulary`, `$comment`, `$defs`
and `definitions` removed from every object, including objects inside arrays. `$ref` and all other members stay.

`map_tool_choice` answers `AUTO`, `NONE` or `ANY`, and `AUTO` for any other string.
`map_stop_reason` accepts the 17 declared finish strings (`STOP` is `stop`,
`MAX_TOKENS` is `length`, the rest `error`) and fails on any other string with
`Unhandled stop reason: <reason>`; `map_stop_reason_string` never fails and answers `error`
for every string except those two. `is_thinking_part` looks only at the thought flag, and
`retain_thought_signature` keeps the last nonempty signature.

```rust
use maestro_models::Tool;
use maestro_models::providers::content::google_shared::convert_tools;
use serde_json::json;

let tools = vec![Tool {
    name: "inspect".to_owned(),
    description: "Use inspect".to_owned(),
    parameters: json!({"$schema": "draft", "type": "object"}),
}];
let declarations = convert_tools(&tools, true).expect("one tool is declared");
assert_eq!(
    serde_json::to_value(declarations).expect("declarations serialize"),
    json!([{"functionDeclarations": [{
        "name": "inspect",
        "description": "Use inspect",
        "parameters": {"type": "object"},
    }]}]),
);
```
