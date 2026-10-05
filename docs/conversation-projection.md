# Conversation projection

`Context` keeps the current `system_prompt`, ordered `messages` and current
`tools` separate. `Message` is exactly user, assistant or tool result. A tool
declaration contains only its name, description and JSON Schema parameters;
execution callbacks and policy belong to the caller. Tool results have no usage
or nested-call field.

`Models::stream` projects once after adapter lookup, using its existing clock
sample for synthetic records, then checks cancellation immediately before
adapter dispatch. `complete` drains the same stream. Adapters receive an owned
request view and perform wire conversion, not another shared projection pass.
`project_context` is also available for pure, deterministic inspection. Projection
stores nothing and never edits supplied prompt, tools, history or image strings.
Tool-result `details` remain caller-owned but never enter provider context.

## Preservation and omission

Exact requested provider, protocol and model equality retains text/call replay
metadata, thinking signatures, signed empty thinking and opaque redacted data.
Actual response model/ID do not redefine that equality. Unsigned blank thinking
is omitted. Foreign replay drops text/call metadata and redacted thinking;
nonblank readable thinking becomes unsigned text, without exposing opaque data.
`TextEnd` attaches supplied replay metadata before its owned closing snapshot;
earlier snapshots retain absent metadata.

Only foreign calls invoke the adapter's deterministic, effect-free
`normalize_tool_call_id` rule. Its identity default changes nothing. Overrides
must preserve distinct IDs within a batch. The originating-turn mapping also
rewrites real and synthetic results; a later same-model turn reusing an ID is
not affected by an earlier rewrite. Names and completed arguments stay exact.

Input capability identifiers are supplied data. `image` retains exact base64
and MIME strings, without fetching, decoding, resizing or re-encoding. Otherwise,
adjacent image runs become one text block:

- User: `(image omitted: model does not support images)`.
- Tool result: `(tool image omitted: model does not support images)`.

Interleaved text order survives. Repeated omission does not accumulate adjacent
identical placeholders. At each next user/assistant boundary and transcript end,
unanswered successful calls receive error results in call order: `No result
provided`, no details, and the supplied synthetic timestamp. Existing matching
results prevent duplicates. Error, aborted and partial assistant attempts are
omitted, without synthesizing their results or inventing completed arguments.
Repair is not tool execution, a model retry or a history write.

## One caller, two registered models

This handoff uses a replacement adapter's ID rule without changing the caller.
The fake observes exactly the request delivered to the adapter.

```rust
use maestro_models::*;
use serde_json::json;
use std::sync::Arc;

struct WireIds(Arc<ScriptedProvider>);
impl Provider for WireIds {
    fn supports(&self, operation: &str) -> bool { self.0.supports(operation) }
    fn normalize_tool_call_id(&self, id: &str, _: &Model, _: &AssistantMessage) -> String {
        format!("wire-{id}")
    }
    fn stream(&self, model: Model, context: Context, options: ProviderOptions)
        -> Result<Box<dyn ProviderStream>, Failure> {
        self.0.stream(model, context, options)
    }
}

async fn caller(models: &Models, target: Model, source: &Context) -> AssistantMessage {
    models.complete(target, source.clone(), StreamOptions {
        auth: Some(RequestAuth::ConfiguredWithoutSecret { source: None }),
        ..Default::default()
    }).await
}

async fn handoff() -> Result<(), Box<dyn std::error::Error>> {
    let first = Model {
        identity: ModelIdentity { provider: "first".into(), model: "vision".into(), operation: "chat".into() },
        protocol: "chat".into(), input: vec!["text".into(), "image".into()],
        rates: None,
        headers: Default::default(),
        capabilities: Default::default(),
    };
    let second = Model {
        identity: ModelIdentity { provider: "second".into(), model: "text".into(), operation: "chat".into() },
        protocol: "chat".into(), input: vec!["text".into()],
        rates: None,
        headers: Default::default(),
        capabilities: Default::default(),
    };
    let call = ToolCall::new("call".into(), "lookup".into(),
        json!({"count":"2"}).as_object().unwrap().clone(), Some("opaque".into()));
    let source = Context {
        system_prompt: Some("Current instructions".into()),
        tools: vec![ToolDeclaration { name: "lookup".into(), description: "Read data".into(),
            parameters: json!({"type":"object","properties":{"count":{"type":"integer"}},"required":["count"]}) }],
        messages: vec![
            Message::User(UserMessage { timestamp: 1, content: vec![InputContent::Image(
                ImageContent { data: "AQID".into(), mime_type: "image/png".into() })] }),
            Message::Assistant(AssistantMessage {
                provider: first.identity.provider.clone(), protocol: first.protocol.clone(),
                model: first.identity.model.clone(), timestamp: 2,
                content: vec![AssistantContent::ToolCall(call.clone())], usage: Usage::default(),
                stop_reason: Some(StopReason::ToolUse), failure: None,
                response_model: None, response_id: None,
            }),
            Message::ToolResult(ToolResultMessage {
                tool_call_id: "call".into(), tool_name: "lookup".into(), timestamp: 3,
                content: vec![InputContent::Text(TextContent { text: "found".into(), replay_metadata: None })],
                details: Some(json!({"application":"private"})), is_error: false,
            }),
        ],
    };
    let before = source.clone();
    let script = || Script::Steps(vec![ScriptStep::Update(ProviderUpdate::Done { reason: StopReason::Stop })]);
    let first_fake = Arc::new(ScriptedProvider::new(vec![script()]));
    let second_fake = Arc::new(ScriptedProvider::new(vec![script()]));
    let mut models = Models::new(Arc::new(|| 73));
    models.register(first.clone(), first_fake.clone())?;
    models.register(second.clone(), Arc::new(WireIds(second_fake.clone())))?;
    for target in [first, second] {
        assert_eq!(caller(&models, target, &source).await.stop_reason, Some(StopReason::Stop));
    }
    assert_eq!(source, before);
    let outgoing = &second_fake.calls()[0].context;
    let Message::User(user) = &outgoing.messages[0] else { panic!() };
    assert_eq!(user.content, vec![InputContent::Text(TextContent {
        text: "(image omitted: model does not support images)".into(), replay_metadata: None })]);
    let Message::Assistant(assistant) = &outgoing.messages[1] else { panic!() };
    let AssistantContent::ToolCall(projected_call) = &assistant.content[0] else { panic!() };
    let Message::ToolResult(result) = &outgoing.messages[2] else { panic!() };
    assert_eq!(projected_call.id, "wire-call");
    assert_eq!(result.tool_call_id, projected_call.id);
    assert_eq!(result.details, None);
    let Message::User(user) = &first_fake.calls()[0].context.messages[0] else { panic!() };
    assert!(matches!(&user.content[0], InputContent::Image(image) if image.data == "AQID"));

    // The caller decides authorization and execution separately from validation.
    let arguments = validate_tool_call(&source.tools, &call)?;
    assert_eq!(arguments, json!({"count":2}).as_object().unwrap().clone());
    let execute = |arguments: serde_json::Map<String, serde_json::Value>| arguments["count"].clone();
    assert_eq!(execute(arguments), json!(2));
    assert_eq!(source, before);
    Ok(())
}
```

## Pure argument validation

`ToolCall::new` constructs completed object arguments; it neither validates nor
authorizes them. Streamed calls expose no arguments until their valid object end.
`validate_tool_call` selects the first matching declaration, requires completed
arguments, clones them, coerces supported values and validates the whole object.
Validation executes nothing and never mutates declarations or calls. Dispatch
does not automatically validate arguments or invoke execution policy.

The offline checker detects the declared JSON Schema dialect automatically;
format checking follows its documented dialect-dependent behavior. Invalid or
unresolvable schemas, including unsupported dialects, fail with `InvalidSchema`.
In-document references resolve locally. External HTTP/file references cannot
trigger retrieval: dependency retrieval features are disabled and construction
is explicitly offline. The checker has documented limitations for some newer
dialect keywords; this interface does not claim universal keyword coverage.
`UnknownTool`, `IncompleteArguments` and `InvalidArguments` distinguish the other
failures. Error text never echoes raw names, schemas or arguments.

Supported coercions are deliberately narrower than full schema validation:

- Number/integer: finite nonblank numeric strings (decimal, exponent, hexadecimal,
  octal, binary), booleans to 1/0, null to 0. Fractional values are not rounded to
  integers; nonfinite values are rejected.
- Boolean: exact `true`/`false` strings, numeric 1/0, null to false. String:
  number/boolean scalar text, null to empty. Null: empty string, zero or false.
- Type arrays preserve an already matching type, otherwise trying declaration
  order. No object/string or singleton-array conversions, default insertion or
  extra-property removal occurs.
- Declared object properties, schema-valued additional properties and declared
  homogeneous/tuple array items recurse. Tuple schemas must declare a supporting
  dialect. `allOf` traverses in order; `anyOf`/`oneOf` try independent candidate
  clones in schema order, validating branches with their enclosing dialect and
  local reference scope intact. Whole-object validation still enforces exclusive
  `oneOf`, required fields, bounds, enums and additional-property rules.
