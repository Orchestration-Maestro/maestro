# Content-generation conversion

`maestro_models::providers::content::google_shared` converts a conversation, its tools
and finish reasons into the serializable content turns of the Gemini-style protocols.
It sends nothing; requests and streams are built on top of it, and
`process_google_stream` reduces the responses they receive. `GoogleThinkingLevel`
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

## Response reduction

`process_google_stream(chunks, output, events, model, counter)` reads framed response
items, one JSON text each, into the shared assistant message `output` and publishes
text, thinking and tool-call events on `events`. It publishes no start, done or error
event and does not end the stream: the caller owns the initial message, those lifecycle
events, cancellation and the stream's end. It adds no HTTP, framing, retry or timeout.

Only the first candidate of an item contributes. The first nonempty response identifier
stays; an empty or absent one is replaced by a later item's. A part whose `thought` flag is
`true` is thinking and every other part with text is text; a signature never decides.
Consecutive parts of one kind extend the open block, a part of the other kind or a function
call closes it first, and a part with both text and a function call applies the text first.
A nonempty signature replaces the block's held one. Image, code and signature-only parts
change nothing.

A function call becomes one complete tool call: a start, one delta holding its arguments as
[compact JSON](arguments.md#streamed-arguments), and an end. Missing or null arguments are
an empty object. The service's identifier is kept when it is nonempty and no call in the
message has it; otherwise the call gets `<name>_<epoch milliseconds>_<n>`, where `<n>`
is the next value of the caller's `counter` (a missing name reads `undefined`). A call
signature is kept when nonempty.

A nonempty finish reason is mapped with `map_stop_reason`, and any call in the message
then makes the outcome `toolUse`; a reason that maps to `error` is recorded, not returned.
A usage report replaces the previous one: input is the prompt count minus the cached count
(without clamping), output is the candidate plus thought counts, cache read is the cached
count, cache write is zero, and the total is the reported total, never a sum. Cost is
recalculated from the model's rates. Absent and null counts are zero, and an absent or null
report keeps the previous one.

A clean end of `chunks` closes the open text or thinking block. A failure from the
source (returned unchanged and never polled past), malformed JSON, a failed typed read of
a selected member, call arguments beyond the JSON conversion's nesting bound, or an
undeclared finish reason returns without closing it, leaving earlier updates in `output`.
A call closes the open block and takes its identifier before its arguments are converted,
so an argument failure keeps those earlier effects. A part's text, thought flag and
signature are read together; unknown members and later candidates are never decoded.

## Request preparation

`maestro_models::providers::content::google` prepares the parameters of a content-generation
request. It sends nothing, calls no hook and never reads a retained timeout, retry or transport
setting; `GoogleOptions` is also exported at the crate root. Two operations compose:

- `build_simple_options(model, options)` selects raw options from simple ones.
- `build_params(model, context, options)` builds `GenerateContentParameters` from the
  [converted](#content-generation-conversion) context and raw options.

```rust
use maestro_models::providers::content::google::{build_params, build_simple_options};
use maestro_models::{Context, Model, SimpleStreamOptions, StreamOptions, ThinkingLevel};
use serde_json::json;

let model: Model = serde_json::from_value(json!({
    "id": "gemini-2.5-flash", "name": "Gemini", "api": "google-generative-ai",
    "provider": "google", "baseUrl": "https://example.invalid/v1beta", "reasoning": true,
    "input": ["text"], "cost": {"input": 0, "output": 0, "cacheRead": 0, "cacheWrite": 0},
    "contextWindow": 1_000_000, "maxTokens": 8192
})).expect("model");
let context: Context = serde_json::from_value(json!({
    "messages": [{"role": "user", "content": "hello", "timestamp": 1}]
})).expect("context");
let simple = SimpleStreamOptions {
    common: StreamOptions { api_key: Some("fixture-key".to_owned()), ..StreamOptions::default() },
    reasoning: Some(ThinkingLevel::Low),
    ..SimpleStreamOptions::default()
};
let options = build_simple_options(&model, Some(&simple)).expect("a key is supplied");
let params = build_params(&model, &context, Some(&options)).expect("not aborted");
let wire = serde_json::to_value(&params).expect("serializes");
assert_eq!(
    maestro_models::compact_json(&wire["config"]["thinkingConfig"]).expect("compact"),
    r#"{"includeThoughts":true,"thinkingBudget":2048}"#,
);
```

On success `build_params` returns the model identifier, the converted turns and a configuration.
Temperature and output limit are kept as supplied, zero and negative values included; the
system prompt is omitted only when empty and is otherwise kept untrimmed. Tools are
declared whole (`parametersJsonSchema`), and a tool mode is emitted only when the context
declares at least one tool. A signal supplied already aborted fails with `Request aborted`;
an active one is retained and serializes as `{}`, so aborting the original later is seen
through the prepared value.

Raw thinking is never inferred. Without a thinking request, or for a model that does not
reason, no thinking configuration is emitted. An enabled request on a reasoning model
includes thoughts and carries its level, or its budget when no level is given; budgets are
not clamped. A disabled request ignores any level and budget and selects the family's
lowest setting without thoughts: `LOW` for third-generation Pro, `MINIMAL` for
third-generation Flash and fourth-generation Gemma (recognized case-insensitively
anywhere in the model identifier, with ASCII digits only), and a zero budget otherwise.

`build_simple_options` fails with `No API key for provider: <provider>` unless a nonempty
supplied key (kept untrimmed) or the provider's environment key exists; it does so before
anything else and does not inspect the context or the signal, which `build_params` checks
later. Common settings come from [`build_base_options`](../model-options.md), which also decides the
output limit; no thinking tokens are added;
the simple tool choice is not forwarded. Missing reasoning selects disabled thinking. A
requested effort is first clamped to the model's supported levels, and off and extra-high
both select high. Level-based families (Pro and Flash of the third generation, Gemma 4)
map the effort to a level and ignore custom budgets; other models use the selected
`thinking_budgets` slot, an explicit zero included, or the family default: Pro
`128/2048/8192/32768`, Flash-Lite `512/2048/8192/24576`, Flash `128/2048/8192/24576`
(minimal to high; `2.5` identifiers, matched case-sensitively) and `-1` (dynamic) otherwise.

## Request projection

`maestro_models::providers::content::google_shared::project_content_request` takes the JSON a
payload hook returned and selects the model path and wire body of the public endpoint. It calls
no hook, sends nothing and builds no URL or header: the endpoint, API version, HTTP options and
cancellation stay with the transport caller, and a client-only member is never a body member.

```rust
use maestro_models::compact_json;
use maestro_models::providers::content::google_shared::project_content_request;
use serde_json::{Value, json};

let projected = project_content_request(json!({
    "model": "gemini-fixture",
    "contents": "hello",
    "config": {"temperature": 0, "serviceTier": "flex"},
})).expect("supported members");
assert_eq!(projected.model, "models/gemini-fixture");
assert_eq!(
    compact_json(&Value::Object(projected.body)).expect("compact"),
    r#"{"contents":[{"parts":[{"text":"hello"}],"role":"user"}],"serviceTier":"flex","generationConfig":{"temperature":0}}"#,
);
```

The result holds the model resource path (`models/` is added unless the name starts with
`models/` or `tunedModels/`) and the body, whose key order and number spelling are written by
the [JSON owner](../json.md). A model that is missing, empty or not text, or that contains
`..`, `?` or `&`, is rejected before contents and configuration are read; the first rejection
in the order model, contents, configuration wins.

- Contents accept a content, a text, an admitted part, or a non-empty homogeneous list of
  contents or admitted parts. Empty lists, mixed content/part lists, and bare function-call
  or function-response parts (single or in a list) are rejected.
- A part may hold several members. File data, inline data and function calls select only
  the members the endpoint knows; other supported part members are copied whole.
- Selected record members omit missing or null values, retain zero and false, and apply
  field-specific transforms to empty text, lists and objects. Unsupported record members
  are rejected on presence, even when null or false.
- Sampling and output settings go into `generationConfig`; the system instruction, safety
  settings, tools, tool configuration, cached content and service tier go to the body.
- A schema is converted node by node: type names are uppercased (unknown names become
  `TYPE_UNSPECIFIED`), type arrays and `anyOf` lists drop their null type into `nullable`,
  `items`, `anyOf` and `properties` are converted recursively, and `additionalProperties`
  is dropped at converted nodes. A union collapses to its other arm only when it is a pure
  nullable wrapper; a parent that holds other non-null members is retained for ordinary
  schema conversion. At the responseSchema and function parameters/response fields only,
  a schema carrying `$schema` is moved to its JSON-schema alternative when that destination
  is absent, null, false, zero or empty text, and is not converted in that alternative.
